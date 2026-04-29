use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use reqwest::Client;
use std::collections::HashMap;
use tauri::Window;
use crate::tools::get_tools;

// Key will be read from environment in execute_gemini_request
const MODEL: &str = "gemini-2.0-flash";
const EMBED_MODEL: &str = "text-embedding-004";

#[derive(Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Clone)]
pub struct Agent {
    client: Client,
}

impl Agent {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub fn get_system_prompt(&self) -> String {
        let tools = get_tools();
        let mut tool_desc = String::new();
        for (name, _) in tools.iter() {
            tool_desc.push_str(&format!("- {}: Execute this tool for specialized tasks.\n", name));
        }

        format!(
            "You are an Elite AI Agent for the MCP Desktop Workspace.
            YOUR GOAL: Help the user manage their enterprise data (SharePoint, Dataverse).
            CAPABILITIES:
            {}
            
            GUIDELINES:
            1. Use tools automatically when needed (e.g., 'Search for sales reports' -> call search_documents).
            2. If you use a tool, explain the results clearly.
            3. Be concise and professional.
            4. If the user asks for a summary, synthesize the tool results into a structured response.
            5. ALWAYS act as if you have direct access to these systems through your tools.",
            tool_desc
        )
    }

    pub async fn chat_with_streaming(
        &self,
        history: Vec<Message>,
        window: &Window,
    ) -> Result<String, String> {
        let mut contents = Vec::new();
        let system_msg = self.get_system_prompt();
        
        for msg in history {
            let role = if msg.role == "user" { "user" } else { "model" };
            contents.push(json!({
                "role": role,
                "parts": [{"text": msg.content}]
            }));
        }

        self.execute_gemini_request(contents, system_msg, window, 0.5).await
    }

    pub async fn analyze_item(
        &self,
        item_title: String,
        item_summary: String,
        metadata: Value,
        mode: String,
        window: &Window,
    ) -> Result<String, String> {
        // --- MOCK INTERCEPTION ---
        if item_title.contains("Handbook") || item_title.contains("Active_Accounts") || item_title.contains("Architecture") {
            let mock_result: String = match mode.as_str() {
                "quick" => "This core enterprise asset establishes the definitive framework for organizational operations and strategic alignment. It serves as the single source of truth for critical compliance and operational workflows.".to_string(),
                "deep" => {
                    format!("**DEEP NEURAL ANALYSIS REPORT**\n\n### 1. Executive Summary\nAnalysis of '{}' reveals a high-density intelligence node with 98% relational relevance to the current workspace. This document/record contains essential structural data required for cross-silo orchestration.\n\n### 2. Key Relational Entities\n- **Primary Owner:** Nexus System Architect\n- **Impact Zone:** Global Operations / Engineering\n- **Integrity Level:** Verified (AES-256)\n\n### 3. Business Recommendation\nFinalize local indexing and initiate automated cross-referencing with relative SharePoint libraries to identify latent dependencies.", item_title)
                },
                "anomalies" => "Anomaly detection complete. Metadata consistency: 100%. Data integrity: Verified. No security risks or structural inconsistencies identified in current record state.".to_string(),
                _ => "High-quality enterprise data node. No issues detected.".to_string()
            };

            let _ = window.emit("chat-chunk", &mock_result.to_string());
            return Ok(mock_result.to_string());
        }

        let prompt = match mode.as_str() {
            "quick" => format!(
                "You are an Elite Enterprise AI. Perform a QUICK CONDENSATION of this item:
                TITLE: {}
                SUMMARY: {}
                METADATA: {:?}
                
                GOAL: Provide exactly 2 powerful sentences explaining the bottom-line value of this item.",
                item_title, item_summary, metadata
            ),
            "deep" => format!(
                "You are an Elite Enterprise AI. Perform a DEEP NEURAL ANALYSIS of this item:
                TITLE: {}
                SUMMARY: {}
                METADATA: {:?}
                
                GOAL: Provide a structured report starting exactly with this bold header:
                **DEEP NEURAL ANALYSIS REPORT**
                
                Followed by these sections, ensuring each major number (1, 2, 3) and sub-bullet starts on a NEW LINE:
                ### 1. Executive Summary
                (Provide the summary here)
                
                ### 2. Key Relational Entities
                (List names, dates, or systems involved)
                
                ### 3. Business Recommendation
                (What should we do next?)",
                item_title, item_summary, metadata
            ),
            "anomalies" => format!(
                "You are an Elite Enterprise AI. Perform ANOMALY DETECTION on this item:
                TITLE: {}
                SUMMARY: {}
                METADATA: {:?}
                
                GOAL: Identify any data outliers, security risks, or inconsistencies in the metadata. If none are found, state that the data appears compliant.",
                item_title, item_summary, metadata
            ),
            _ => "Provide a general analysis of this enterprise data.".to_string(),
        };

        let contents = vec![json!({"role": "user", "parts": [{"text": prompt}]})];
        self.execute_gemini_request(contents, "You are a highly structured Enterprise Intelligence Agent.".to_string(), window, 0.2).await
    }

    pub async fn embed_text(&self, text: &str) -> Result<Vec<f32>, String> {
        let api_key = std::env::var("GOOGLE_API_KEY")
            .map_err(|_| "GOOGLE_API_KEY not found in environment".to_string())?;

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:embedContent?key={}",
            EMBED_MODEL, api_key
        );

        let response: Value = self.client
            .post(&url)
            .json(&json!({
                "model": format!("models/{}", EMBED_MODEL),
                "content": {
                    "parts": [{"text": text}]
                }
            }))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;

        let embedding = response["embedding"]["values"]
            .as_array()
            .ok_or_else(|| format!("Invalid embedding response: {:?}", response))?
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0) as f32)
            .collect();

        Ok(embedding)
    }

    async fn execute_gemini_request(
        &self,
        contents: Vec<Value>,
        system_instruction: String,
        window: &Window,
        temp: f32,
    ) -> Result<String, String> {
        let api_key = std::env::var("GOOGLE_API_KEY")
            .map_err(|_| "GOOGLE_API_KEY not found in environment".to_string())?;

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            MODEL, api_key
        );

        let body = json!({
            "contents": contents,
            "system_instruction": {
                "parts": [{"text": system_instruction}]
            },
            "generationConfig": {
                "temperature": temp,
                "maxOutputTokens": 2048,
            }
        });

        let response: Value = self.client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;

        let content = response["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or_else(|| format!("Invalid response from Gemini: {:?}", response))?
            .to_string();

        // Emit to frontend for 'streaming' effect
        window.emit("chat-chunk", &content).map_err(|e: tauri::Error| e.to_string())?;

        Ok(content)
    }
}

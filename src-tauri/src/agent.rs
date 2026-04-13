use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use reqwest::Client;
use std::collections::HashMap;
use tauri::Window;
use crate::tools::get_tools;

// Key will be read from environment in execute_groq_request
const MODEL: &str = "llama-3.3-70b-versatile";

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
        let mut messages = vec![json!({"role": "system", "content": self.get_system_prompt()})];
        
        for msg in history {
            messages.push(json!({"role": msg.role, "content": msg.content}));
        }

        self.execute_groq_request(messages, window, 0.5).await
    }

    pub async fn analyze_item(
        &self,
        item_title: String,
        item_summary: String,
        metadata: Value,
        mode: String,
        window: &Window,
    ) -> Result<String, String> {
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

        let messages = vec![
            json!({"role": "system", "content": "You are a highly structured Enterprise Intelligence Agent."}),
            json!({"role": "user", "content": prompt}),
        ];

        self.execute_groq_request(messages, window, 0.2).await
    }

    async fn execute_groq_request(
        &self,
        messages: Vec<Value>,
        window: &Window,
        temp: f32,
    ) -> Result<String, String> {
        let api_key = std::env::var("GROQ_API_KEY")
            .map_err(|_| "GROQ_API_KEY not found in environment".to_string())?;

        let full_response: Value = self.client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&json!({
                "model": MODEL,
                "messages": messages,
                "temperature": temp,
                "max_tokens": 1024
            }))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;

        let content = full_response["choices"][0]["message"]["content"]
            .as_str()
            .ok_or("Invalid response from Groq")?
            .to_string();

        // Emit to frontend for 'streaming' effect
        window.emit("chat-chunk", &content).map_err(|e| e.to_string())?;

        Ok(content)
    }
}

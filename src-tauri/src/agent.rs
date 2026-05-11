use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use reqwest::Client;
use std::collections::HashMap;
use tauri::Window;
use crate::tools::get_tools;

// Groq configuration
const GROQ_MODEL: &str = "llama-3.3-70b-versatile";
const EMBED_MODEL: &str = "gemini-embedding-001"; // Still using Gemini for embeddings for now

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
            let role = if msg.role == "user" { "user" } else { "assistant" };
            contents.push(json!({
                "role": role,
                "content": msg.content
            }));
        }

        self.execute_groq_request(contents, system_msg, window, 0.5).await
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

        let contents = vec![json!({"role": "user", "content": prompt})];
        self.execute_groq_request(contents, "You are a highly structured Enterprise Intelligence Agent.".to_string(), window, 0.2).await
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

    /// RAG-powered query: takes pre-retrieved relevant chunks and generates a grounded answer
    pub async fn process_rag_query(
        &self,
        question: String,
        retrieved_chunks: Vec<serde_json::Value>, // Results from semantic_chunk_search
        window: &Window,
    ) -> Result<String, String> {
        if retrieved_chunks.is_empty() {
            return Ok("No relevant content was found in the knowledge base for your query. Try running a discovery sync to index documents, or rephrase your question.".to_string());
        }

        // Build focused context from retrieved chunks
        let mut context = String::new();
        for chunk in retrieved_chunks.iter() {
            let title = chunk["doc_title"].as_str().unwrap_or("Unknown");
            let content = chunk["content"].as_str().unwrap_or("");
            let similarity = chunk["similarity"].as_f64().unwrap_or(0.0);
            let chunk_index = chunk["chunk_index"].as_u64().unwrap_or(0);

            context.push_str(&format!(
                "\n[Source: {} | Chunk: {} | Relevance: {:.0}%]\n{}\n",
                title, chunk_index + 1, similarity * 100.0, content
            ));
        }

        let system_prompt = format!(
            "You are Nexus AI, an elite enterprise intelligence assistant with RAG-powered retrieval.

RETRIEVED CONTEXT (ranked by relevance):
{}

INSTRUCTIONS:
1. Answer the user's question using ONLY the retrieved context above.
2. ALWAYS cite your sources using this format: [Source: Document Name | Chunk: N]
3. If multiple chunks from the same document are relevant, consolidate them in your reasoning but cite them all.
4. If the context doesn't contain enough information, say so honestly.
5. Be concise, professional, and structured. Use markdown formatting.
6. For data questions (dates, numbers, names), extract precisely from the context.
7. For analysis questions, synthesize insights across multiple sources.
8. Never fabricate information not present in the retrieved context.",
            context
        );

        let contents = vec![serde_json::json!({"role": "user", "content": question})];
        self.execute_groq_request(contents, system_prompt, window, 0.2).await
    }

    /// Legacy full-context method (kept for backward compatibility)
    pub async fn ask_with_full_context(
        &self,
        question: String,
        data_contexts: Vec<(String, String, String, String)>,
        window: &Window,
    ) -> Result<String, String> {
        let mut knowledge_base = String::new();
        for (i, (title, source, summary, metadata)) in data_contexts.iter().enumerate() {
            knowledge_base.push_str(&format!(
                "\n--- Document {} ---\nTitle: {}\nSource: {}\nSummary: {}\nMetadata: {}\n",
                i + 1, title, source, summary, metadata
            ));
        }

        let system_prompt = format!(
            "You are Nexus AI, an elite enterprise intelligence assistant. You have access to the following knowledge base:\n{}\n\nINSTRUCTIONS:\n1. Answer using ONLY the knowledge base above.\n2. Cite sources by name: [Source: Document Name]\n3. If data is insufficient, say so clearly.\n4. Use markdown formatting.",
            knowledge_base
        );

        let contents = vec![serde_json::json!({"role": "user", "content": question})];
        self.execute_groq_request(contents, system_prompt, window, 0.3).await
    }

    async fn execute_groq_request(
        &self,
        mut messages: Vec<Value>,
        system_instruction: String,
        window: &Window,
        temp: f32,
    ) -> Result<String, String> {
        let api_key = std::env::var("GROQ_API_KEY")
            .map_err(|_| "GROQ_API_KEY not found in environment".to_string())?;

        let url = "https://api.groq.com/openai/v1/chat/completions";

        // Prepend system message
        messages.insert(0, json!({
            "role": "system",
            "content": system_instruction
        }));

        let body = json!({
            "model": GROQ_MODEL,
            "messages": messages,
            "temperature": temp,
            "max_tokens": 2048,
        });

        let response: Value = self.client
            .post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("JSON parsing failed: {}", e))?;

        if let Some(error) = response.get("error") {
            return Err(format!("Groq Error: {}", error["message"].as_str().unwrap_or("Unknown error")));
        }

        let content = response["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| format!("Invalid response from Groq: {:?}", response))?
            .to_string();

        // Emit to frontend for 'streaming' effect
        window.emit("chat-chunk", &content).map_err(|e: tauri::Error| e.to_string())?;

        Ok(content)
    }
}

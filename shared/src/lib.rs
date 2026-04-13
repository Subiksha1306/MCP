use serde::{Deserialize, Serialize};

// Shared types between Rust backend and UI frontend

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SearchResult {
    pub content: String,
    pub source: String,
    pub similarity: f32,
    pub chunk_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SharePointFile {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub last_modified: String,
    pub content_type: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DataverseRecord {
    pub id: String,
    pub logical_name: String,
    pub fields: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerStatus {
    pub running: bool,
    pub port: u16,
    pub uptime_seconds: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuthStatus {
    pub sharepoint_authenticated: bool,
    pub dataverse_authenticated: bool,
    pub claude_authenticated: bool,
}

// Request/Response types for Tauri commands

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SearchRequest {
    pub query: String,
    pub top_k: Option<usize>,
    pub sources: Option<Vec<String>>, // ["sharepoint", "dataverse", "local"]
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatRequest {
    pub message: String,
    pub context: Option<Vec<SearchResult>>,
    pub streaming: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuthRequest {
    pub service: String, // "sharepoint", "dataverse", "claude"
    pub tenant_id: Option<String>,
    pub client_id: Option<String>,
}

// Configuration types

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    pub mcp_server_port: u16,
    pub max_chunk_size: usize,
    pub embedding_model: String,
    pub vector_db_path: String,
    pub sharepoint_site_url: Option<String>,
    pub dataverse_base_url: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            mcp_server_port: 3721,
            max_chunk_size: 1000,
            embedding_model: "mock".to_string(),
            vector_db_path: "data/vectors.db".to_string(),
            sharepoint_site_url: None,
            dataverse_base_url: None,
        }
    }
}
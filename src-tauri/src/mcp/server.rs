use axum::{
    routing::{get, post},
    Router,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::SocketAddr;
use crate::tools::get_tools;

#[derive(Clone)]
pub struct AppState {}

pub fn create_router() -> Router {
    Router::new()
        .route("/tools/list", get(tools_list))
        .route("/tools/call", post(tools_call))
        .route("/search", get(search))
}

async fn tools_list() -> Json<serde_json::Value> {
    Json(json!({
        "tools": ["search", "echo", "search_documents", "fetch_sharepoint_files", "query_dataverse"]
    }))
}

#[derive(Deserialize)]
struct ToolCallRequest {
    name: String,
    arguments: serde_json::Value,
}

async fn tools_call(Json(req): Json<ToolCallRequest>) -> Json<serde_json::Value> {
    let tools = get_tools();

    let result = if let Some(tool_fn) = tools.get(&req.name) {
        tool_fn(&req.arguments)
    } else {
        json!({ "error": "Tool not found" })
    };

    Json(result)
}

#[derive(Serialize)]
struct SearchResult {
    results: Vec<String>,
}

async fn search() -> Json<SearchResult> {
    Json(SearchResult {
        results: vec!["Mock search result".to_string()],
    })
}
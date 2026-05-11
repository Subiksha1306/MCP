use axum::{
    routing::{get, post},
    Router,
    Json,
    extract::Request,
    middleware::{self, Next},
    response::Response,
    http::StatusCode,
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
        .route_layer(middleware::from_fn(auth_audit_middleware))
}

async fn tools_list() -> Json<serde_json::Value> {
    Json(json!({
        "tools": ["search", "echo", "search_documents", "fetch_sharepoint_files", "query_dataverse"]
    }))
}

use crate::permissions::{PermissionLevel, SecurityContext};

async fn auth_audit_middleware(req: Request, next: Next) -> Result<Response, StatusCode> {
    let path = req.uri().path().to_string();
    let security_ctx = SecurityContext::from_env();

    // Basic RBAC check for API access
    if !security_ctx.role.can_query() && path.contains("/tools/call") {
        eprintln!("API Auth Failed: User '{}' has insufficient role ({:?}) for {}", 
            security_ctx.user_id, security_ctx.role, path);
        return Err(StatusCode::FORBIDDEN);
    }

    // Call next handler
    let start = std::time::Instant::now();
    let res = next.run(req).await;
    let elapsed = start.elapsed().as_millis();

    // Log the audit
    let status = if res.status().is_success() { "success" } else { "failed" };
    println!("[Audit] User: {} | Action: {} | Status: {} | Time: {}ms", 
        security_ctx.user_id, path, status, elapsed);

    Ok(res)
}

#[derive(Deserialize)]
struct ToolCallRequest {
    name: String,
    arguments: serde_json::Value,
}

async fn tools_call(Json(req): Json<ToolCallRequest>) -> Json<serde_json::Value> {
    let tools = get_tools();
    let current_permission = PermissionLevel::from_env();

    let result = if let Some(tool) = tools.get(&req.name) {
        if current_permission.can_execute(tool.required_permission) {
            (tool.func)(&req.arguments)
        } else {
            json!({ "error": "Forbidden: Tool requires higher permission level" })
        }
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
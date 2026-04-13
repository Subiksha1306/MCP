use crate::AppState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

// Command payloads
#[derive(Deserialize)]
pub struct SearchDocumentsPayload {
    pub query: String,
    pub top_k: Option<usize>,
}

#[derive(Deserialize)]
pub struct FetchSharePointFilesPayload {
    pub library_name: String,
    pub access_token: Option<String>,
}

#[derive(Deserialize)]
pub struct QueryDataversePayload {
    pub table_name: String,
    pub filter: Option<String>,
    pub access_token: Option<String>,
}

// Response types
#[derive(Serialize)]
pub struct ServerStatus {
    pub running: bool,
    pub port: u16,
}

#[derive(Serialize)]
pub struct SearchResult {
    pub content: String,
    pub source: String,
    pub similarity: f32,
}

#[derive(Serialize)]
pub struct SharePointFile {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub last_modified: String,
}

#[derive(Serialize)]
pub struct DataverseRecord {
    pub id: String,
    pub fields: serde_json::Value,
}

// Tauri commands
#[tauri::command]
pub async fn start_mcp_server(
    state: State<'_, crate::AppState>,
) -> Result<ServerStatus, String> {
    // Start the MCP server in a background task
    let handle = tokio::spawn(async {
        let app = crate::mcp::server::create_router();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:3721")
            .await
            .expect("Failed to bind to port 3721");

        println!("🚀 MCP Server running on http://127.0.0.1:3721");

        axum::serve(listener, app)
            .await
            .expect("Server error");
    });

    // Store the handle in state
    let mut state_guard = state.0.lock().await;
    state_guard.mcp_server_handle = Some(handle);

    Ok(ServerStatus {
        running: true,
        port: 3721,
    })
}

#[tauri::command]
pub async fn stop_mcp_server(
    state: State<'_, crate::AppState>,
) -> Result<(), String> {
    let state_guard = state.0.lock().await;
    if let Some(handle) = &state_guard.mcp_server_handle {
        handle.abort();
    }

    Ok(())
}

#[tauri::command]
pub async fn get_server_status(
    _state: State<'_, crate::AppState>,
) -> Result<ServerStatus, String> {
    // For now, just check if we can connect to the port
    // In a real implementation, we'd check the actual server state
    Ok(ServerStatus {
        running: true, // Assume running for now
        port: 3721,
    })
}

#[tauri::command]
pub async fn search_documents(
    payload: SearchDocumentsPayload,
) -> Result<Vec<SearchResult>, String> {
    // Call the search_documents tool
    let tools = crate::tools::get_tools();
    let args = serde_json::json!({
        "query": payload.query,
        "top_k": payload.top_k.unwrap_or(5)
    });

    match tools.get("search_documents") {
        Some(tool_fn) => {
            let result = tool_fn(&args);
            // Parse the result and return proper SearchResult structs
            if let Some(results) = result.get("results") {
                if let Some(results_array) = results.as_array() {
                    let search_results: Vec<SearchResult> = results_array
                        .iter()
                        .filter_map(|r| {
                            Some(SearchResult {
                                content: r.get("content")?.as_str()?.to_string(),
                                source: r.get("source")?.as_str()?.to_string(),
                                similarity: r.get("similarity")?.as_f64()? as f32,
                            })
                        })
                        .collect();
                    Ok(search_results)
                } else {
                    Err("Invalid results format".to_string())
                }
            } else {
                Err("No results in response".to_string())
            }
        }
        None => Err("Tool not found".to_string()),
    }
}

#[tauri::command]
pub async fn fetch_sharepoint_files(
    payload: FetchSharePointFilesPayload,
) -> Result<Vec<SharePointFile>, String> {
    // Call the fetch_sharepoint_files tool
    let tools = crate::tools::get_tools();
    let args = serde_json::json!({
        "library_name": payload.library_name
    });

    match tools.get("fetch_sharepoint_files") {
        Some(tool_fn) => {
            let result = tool_fn(&args);
            // Parse the result and return proper SharePointFile structs
            if let Some(files) = result.get("files") {
                if let Some(files_array) = files.as_array() {
                    let sharepoint_files: Vec<SharePointFile> = files_array
                        .iter()
                        .filter_map(|f| {
                            Some(SharePointFile {
                                name: f.get("name")?.as_str()?.to_string(),
                                url: f.get("url")?.as_str()?.to_string(),
                                size: f.get("size")?.as_u64()?,
                                last_modified: f.get("last_modified")?.as_str()?.to_string(),
                            })
                        })
                        .collect();
                    Ok(sharepoint_files)
                } else {
                    Err("Invalid files format".to_string())
                }
            } else {
                Err("No files in response".to_string())
            }
        }
        None => Err("Tool not found".to_string()),
    }
}

#[tauri::command]
pub async fn query_dataverse(
    payload: QueryDataversePayload,
) -> Result<Vec<DataverseRecord>, String> {
    // Call the query_dataverse tool
    let tools = crate::tools::get_tools();
    let mut args = serde_json::json!({
        "table_name": payload.table_name
    });

    if let Some(filter) = payload.filter {
        args["filter"] = serde_json::Value::String(filter);
    }

    match tools.get("query_dataverse") {
        Some(tool_fn) => {
            let result = tool_fn(&args);
            // Parse the result and return proper DataverseRecord structs
            if let Some(records) = result.get("records") {
                if let Some(records_array) = records.as_array() {
                    let dataverse_records: Vec<DataverseRecord> = records_array
                        .iter()
                        .filter_map(|r| {
                            Some(DataverseRecord {
                                id: r.get("id")?.as_str()?.to_string(),
                                fields: r.clone(),
                            })
                        })
                        .collect();
                    Ok(dataverse_records)
                } else {
                    Err("Invalid records format".to_string())
                }
            } else {
                Err("No records in response".to_string())
            }
        }
        None => Err("Tool not found".to_string()),
    }
}
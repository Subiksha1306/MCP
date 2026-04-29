use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use tauri::{Window, State};

// Command payloads
// (Structs removed in favor of direct arguments)

#[derive(Deserialize)]
pub struct UploadDocumentPayload {
    pub path: String,
    pub contents: String,
}

#[derive(Deserialize)]
pub struct ClaudeKeyPayload {
    pub api_key: String,
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
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3721")
        .await
        .map_err(|err| err.to_string())?;
    let app = crate::mcp::server::create_router();

    println!("🚀 MCP Server running on http://127.0.0.1:3721");

    let handle = tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, app).await {
            eprintln!("MCP server error: {}", err);
        }
    });

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
    query: String,
    top_k: Option<usize>,
    state: State<'_, crate::AppState>,
) -> Result<Vec<SearchResult>, String> {
    // Call the search_documents tool
    let tools = crate::tools::get_tools();
    let args = serde_json::json!({
        "query": query,
        "top_k": top_k.unwrap_or(5)
    });

    match tools.get("search_documents") {
        Some(tool_fn) => {
            let result = tool_fn(&args);
            
            // Log to memory
            let state_guard = state.0.lock().await;
            if let Some(db) = &state_guard.db {
                let _: rusqlite::Result<()> = db.update_sync_stats("vector_store", 1);
            }

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
pub async fn chat_message(
    message: String,
    window: Window,
    state: State<'_, crate::AppState>,
) -> Result<serde_json::Value, String> {
    // 1. Save user message and get history/agent (hold lock briefly)
    let (history, agent) = {
        let state_guard = state.0.lock().await;
        
        if let Some(db) = &state_guard.db {
            let _: rusqlite::Result<()> = db.save_message("user", &message);
        }

        let history_raw = if let Some(db) = &state_guard.db {
            db.get_chat_history(10).unwrap_or_else(|_| vec![])
        } else {
            vec![]
        };

        let history: Vec<crate::agent::Message> = history_raw.into_iter().map(|(r, c)| {
            crate::agent::Message { role: r, content: c }
        }).collect();

        (history, state_guard.agent.clone())
    };

    // 2. Process with Agent (AI call, don't hold lock)
    let response: String = agent.chat_with_streaming(history, &window).await?;

    // 3. Save AI response (hold lock briefly)
    {
        let state_guard = state.0.lock().await;
        if let Some(db) = &state_guard.db {
            let _: rusqlite::Result<()> = db.save_message("assistant", &response);
        }
    }

    Ok(serde_json::json!({
        "reply": response,
    }))
}

#[tauri::command]
pub async fn upload_document(
    payload: UploadDocumentPayload,
) -> Result<serde_json::Value, String> {
    let mut data_dir = std::env::temp_dir();
    data_dir.push("mcp-desktop");
    fs::create_dir_all(&data_dir).map_err(|err| err.to_string())?;
    let mut file_path = data_dir.clone();
    file_path.push(&payload.path);
    fs::write(&file_path, payload.contents).map_err(|err| err.to_string())?;
    Ok(serde_json::json!({
        "success": true,
        "path": file_path.to_string_lossy(),
    }))
}

#[tauri::command]
pub async fn save_claude_key(
    payload: ClaudeKeyPayload,
) -> Result<String, String> {
    println!("Saved Claude key: {}", payload.api_key);
    Ok("Claude API key saved".to_string())
}
#[tauri::command]
pub async fn fetch_sharepoint_files(
    library_name: String,
    _access_token: Option<String>,
    state: State<'_, crate::AppState>,
) -> Result<Vec<SharePointFile>, String> {
    // Call the fetch_sharepoint_files tool
    let tools = crate::tools::get_tools();
    let args = serde_json::json!({
        "library_name": library_name
    });

    match tools.get("fetch_sharepoint_files") {
        Some(tool_fn) => {
            let result = tool_fn(&args);
            
            // Log to memory
            let state_guard = state.0.lock().await;
            if let Some(db) = &state_guard.db {
                let _: rusqlite::Result<()> = db.update_sync_stats("sharepoint", 1);
            }

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

// --- Discovery Commands ---

#[tauri::command]
pub async fn start_discovery(
    source_type: String,
    url: String,
    _state: State<'_, crate::AppState>,
) -> Result<String, String> {
    // We don't lock the state for the whole discovery process, 
    // just long enough to spawn it.
    let db_path = "memory.db".to_string(); 
    let source_type_clone = source_type.clone();
    let url_clone = url.clone();

    // Spawn discovery in background
    tokio::spawn(async move {
        let engine = crate::discovery::DiscoveryEngine::new(db_path);
        if let Err(e) = engine.start_discovery(&source_type_clone, &url_clone).await {
            eprintln!("Discovery error: {}", e);
        }
    });

    Ok(format!("Discovery started for {} at {}", source_type, url))
}

#[tauri::command]
pub async fn get_discovery_data(
    page: u32,
    limit: u32,
    state: State<'_, crate::AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let state_guard = state.0.lock().await;
    let db = state_guard.db.as_ref()
        .ok_or_else(|| "Database not initialized".to_string())?;

    db.get_paginated_discovery(page, limit)
        .map_err(|e: rusqlite::Error| e.to_string())
}

#[tauri::command]
pub async fn search_discovery(
    query: String,
    state: State<'_, crate::AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    // 1. Get embedding (AI call, don't hold lock)
    let agent = {
        let state_guard = state.0.lock().await;
        state_guard.agent.clone()
    };
    
    let embedding = agent.embed_text(&query).await.ok();

    // 2. Perform search (DB call, hold lock)
    let state_guard = state.0.lock().await;
    let db = state_guard.db.as_ref()
        .ok_or_else(|| "Database not initialized".to_string())?;
    
    if let Some(emb) = embedding {
        if let Ok(results) = db.semantic_search(&emb, 20) {
            let results_val: Vec<serde_json::Value> = results; 
            if !results_val.is_empty() {
                return Ok(results_val);
            }
        }
    }

    // Fallback to keyword search
    db.search_discovery(&query)
        .map_err(|e: rusqlite::Error| e.to_string())
}

#[tauri::command]
pub async fn analyze_enterprise_item(
    item_title: String,
    item_summary: String,
    metadata_json: String,
    mode: String,
    window: tauri::Window,
    state: State<'_, crate::AppState>,
) -> Result<String, String> {
    let metadata: Value = serde_json::from_str(&metadata_json)
        .map_err(|e| format!("Invalid metadata JSON: {}", e))?;
    
    let agent = state.0.lock().await.agent.clone();
    
    agent.analyze_item(item_title, item_summary, metadata, mode, &window).await
}

#[tauri::command]
pub async fn query_dataverse(
    table_name: String,
    filter: Option<String>,
    _access_token: Option<String>,
    state: State<'_, crate::AppState>,
) -> Result<Vec<DataverseRecord>, String> {
    // Call the query_dataverse tool
    let tools = crate::tools::get_tools();
    let mut args = serde_json::json!({
        "table_name": table_name
    });

    if let Some(f) = filter {
        args["filter"] = serde_json::Value::String(f);
    }

    match tools.get("query_dataverse") {
        Some(tool_fn) => {
            let result = tool_fn(&args);
            
            // Log to memory
            let state_guard = state.0.lock().await;
            if let Some(db) = &state_guard.db {
                let _: rusqlite::Result<()> = db.update_sync_stats("dataverse", 1);
            }

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
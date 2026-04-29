#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::sync::Arc;
use tokio::sync::Mutex;

// Import our modules
mod mcp;
mod connectors;
mod tools;
mod commands;
mod db;
mod agent;
mod discovery;

// Shared state for the application
pub struct AppState(pub Arc<Mutex<AppStateInner>>);

pub struct AppStateInner {
    mcp_server_handle: Option<tokio::task::JoinHandle<()>>,
    pub db: Option<db::MemoryDB>,
    pub agent: agent::Agent,
}

fn main() {
    dotenv::dotenv().ok();
    let db_path = "memory.db";
    let memory_db = db::MemoryDB::new(db_path).ok();
    
    if let Some(db) = &memory_db {
        let _ = db.seed_initial_data();
    }

    let agent_instance = agent::Agent::new();

    let state = AppState(Arc::new(Mutex::new(AppStateInner {

        mcp_server_handle: None,
        db: memory_db,
        agent: agent_instance,
    })));

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::start_mcp_server,
            commands::stop_mcp_server,
            commands::search_documents,
            commands::fetch_sharepoint_files,
            commands::query_dataverse,
            commands::chat_message,
            commands::upload_document,
            commands::save_claude_key,
            commands::get_server_status,
            commands::start_discovery,
            commands::get_discovery_data,
            commands::search_discovery,
            commands::analyze_enterprise_item,
            commands::query_dataverse
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
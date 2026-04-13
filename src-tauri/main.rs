#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::State;

// Import our modules
mod mcp;
mod connectors;
mod tools;
mod commands;
mod commands;

// Shared state for the application
#[derive(Default)]
pub struct AppState(pub Arc<Mutex<AppStateInner>>);

#[derive(Default)]
pub struct AppStateInner {
    mcp_server_handle: Option<tokio::task::JoinHandle<()>>,
}

fn main() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::start_mcp_server,
            commands::stop_mcp_server,
            commands::search_documents,
            commands::fetch_sharepoint_files,
            commands::query_dataverse,
            commands::get_server_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
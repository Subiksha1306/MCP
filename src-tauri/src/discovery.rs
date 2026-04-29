use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use crate::connectors::{sharepoint::SharePointConnector, dataverse::DataverseConnector};
use crate::db::MemoryDB;
use anyhow::{Result, Context};

#[derive(Debug, Serialize, Deserialize)]
pub struct DiscoveryItem {
    pub id: String,
    pub title: String,
    pub source_type: String,
    pub category: String,
    pub summary: String,
    pub metadata: Value,
}

use crate::agent::Agent;

pub struct DiscoveryEngine {
    db_path: String,
    agent: Agent,
}

impl DiscoveryEngine {
    pub fn new(db_path: String) -> Self {
        Self { 
            db_path,
            agent: Agent::new(),
        }
    }

    pub async fn start_discovery(&self, source: &str, url: &str) -> Result<()> {
        match source {
            "sharepoint" => self.sync_sharepoint(url).await?,
            "dataverse" => self.sync_dataverse(url).await?,
            _ => return Err(anyhow::anyhow!("Unknown source: {}", source)),
        }
        Ok(())
    }

    async fn sync_sharepoint(&self, url: &str) -> Result<()> {
        let site_url = if url.is_empty() { env::var("SP_SITE_URL").unwrap_or_default() } else { url.to_string() };
        let token = env::var("SP_ACCESS_TOKEN").map_err(|_| anyhow::anyhow!("SP_ACCESS_TOKEN not found in environment"))?;
        
        if site_url.is_empty() {
            return Err(anyhow::anyhow!("SharePoint Site URL is required"));
        }

        let db = MemoryDB::new(&self.db_path).map_err(|e| anyhow::anyhow!("Failed to open DB: {}", e))?;
        let connector = SharePointConnector::new(site_url, token);
        let files = connector.get_files("Documents").await?;

        for file in files {
            let id = format!("sp_file_{}", urlencoding::encode(&file.name));
            let summary = format!("A {} file named {}. Size: {} bytes. Modified: {}", 
                file.content_type, file.name, file.size, file.last_modified);
            
            // Index the metadata and title for semantic search
            let text_to_embed = format!("Title: {}\nSummary: {}", file.name, summary);
            if let Ok(vec) = self.agent.embed_text(&text_to_embed).await {
                db.save_embedding(&id, &vec).ok();
            }

            db.save_discovery_item(
                &id,
                &file.name,
                "SharePoint",
                "File",
                &summary,
                &json!(file).to_string()
            ).context("Failed to save discovery item")?;
        }

        Ok(())
    }

    async fn sync_dataverse(&self, url: &str) -> Result<()> {
        let base_url = if url.is_empty() { env::var("DV_BASE_URL").unwrap_or_default() } else { url.to_string() };
        let token = env::var("DV_ACCESS_TOKEN").map_err(|_| anyhow::anyhow!("DV_ACCESS_TOKEN not found in environment"))?;

        if base_url.is_empty() {
            return Err(anyhow::anyhow!("Dataverse Base URL is required"));
        }

        let db = MemoryDB::new(&self.db_path).map_err(|e| anyhow::anyhow!("Failed to open DB: {}", e))?;
        let connector = DataverseConnector::new(base_url, token);
        let tables = connector.get_tables().await?;

        for table in tables.iter().take(3) { // Limit to 3 tables for discovery demo
            let records = connector.get_records(&table.logical_name, Some(10)).await?;
            
            for record in records {
                let id = format!("dv_rec_{}_{}", table.logical_name, record.id);
                let title = record.fields.get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&format!("Record {}", record.id))
                    .to_string();

                let summary = format!("A record from the {} table in Dataverse.", table.display_name);

                // Index the record for semantic search
                let text_to_embed = format!("Title: {}\nEntity: {}\nSummary: {}", title, table.display_name, summary);
                if let Ok(vec) = self.agent.embed_text(&text_to_embed).await {
                    db.save_embedding(&id, &vec).ok();
                }

                db.save_discovery_item(
                    &id,
                    &title,
                    "Dataverse",
                    &table.display_name,
                    &summary,
                    &json!(record).to_string()
                ).context("Failed to save discovery item")?;
            }
        }

        Ok(())
    }
}

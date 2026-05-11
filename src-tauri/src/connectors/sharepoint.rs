use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// SharePoint connector for fetching files and documents
pub struct SharePointConnector {
    client: Client,
    site_url: String,
    access_token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SharePointFile {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub last_modified: String,
    pub content_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SharePointListItem {
    pub id: String,
    pub title: String,
    pub fields: HashMap<String, serde_json::Value>,
}

impl SharePointConnector {
    /// Create a new SharePoint connector
    pub fn new(site_url: String, access_token: String) -> Self {
        Self {
            client: Client::new(),
            site_url,
            access_token,
        }
    }

    /// Get all files from a document library
    pub async fn get_files(&self, library_name: &str) -> Result<Vec<SharePointFile>> {
        let url = format!(
            "{}/_api/web/lists/getbytitle('{}')/items?$select=FileLeafRef,FileRef,File_x0020_Size,Modified,File_x0020_Type",
            self.site_url, library_name
        );

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("Accept", "application/json;odata=nometadata")
            .send()
            .await
            .context("Failed to send request to SharePoint")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("SharePoint API error: {}", response.status()));
        }

        let data: serde_json::Value = response.json().await?;
        let files = data["value"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|item| {
                Some(SharePointFile {
                    name: item["FileLeafRef"].as_str()?.to_string(),
                    url: item["FileRef"].as_str()?.to_string(),
                    size: item["File_x0020_Size"].as_u64()?,
                    last_modified: item["Modified"].as_str()?.to_string(),
                    content_type: item["File_x0020_Type"].as_str()?.to_string(),
                })
            })
            .collect();

        Ok(files)
    }

    /// Get raw bytes content of a specific file (for PDF, DOCX, XLSX extraction)
    pub async fn get_file_content_bytes(&self, file_url: &str) -> Result<Vec<u8>> {
        let url = format!("{}/_api/web/getfilebyserverrelativeurl('{}')/$value", self.site_url, file_url);

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send()
            .await
            .context("Failed to fetch file content bytes")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to get file bytes: {}", response.status()));
        }

        let bytes = response.bytes().await?;
        Ok(bytes.to_vec())
    }

    /// Get content of a specific file as plain text (legacy)
    pub async fn get_file_content(&self, file_url: &str) -> Result<String> {
        let url = format!("{}/_api/web/getfilebyserverrelativeurl('{}')/$value", self.site_url, file_url);

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send()
            .await
            .context("Failed to fetch file content")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to get file content: {}", response.status()));
        }

        let content = response.text().await?;
        Ok(content)
    }

    /// Get list items from a SharePoint list
    pub async fn get_list_items(&self, list_name: &str) -> Result<Vec<SharePointListItem>> {
        let url = format!(
            "{}/_api/web/lists/getbytitle('{}')/items",
            self.site_url, list_name
        );

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("Accept", "application/json;odata=nometadata")
            .send()
            .await
            .context("Failed to fetch list items")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("SharePoint API error: {}", response.status()));
        }

        let data: serde_json::Value = response.json().await?;
        let items = data["value"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|item| {
                let fields = item.as_object()?
                    .iter()
                    .filter(|(k, _)| !k.starts_with("odata") && *k != "ID")
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                Some(SharePointListItem {
                    id: item["ID"].as_str()?.to_string(),
                    title: item["Title"].as_str().unwrap_or("").to_string(),
                    fields,
                })
            })
            .collect();

        Ok(items)
    }
}
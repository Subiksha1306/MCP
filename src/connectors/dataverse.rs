use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Dataverse connector for fetching tables and records
pub struct DataverseConnector {
    client: Client,
    base_url: String,
    access_token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataverseTable {
    pub logical_name: String,
    pub display_name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataverseRecord {
    pub id: String,
    pub logical_name: String,
    pub fields: HashMap<String, serde_json::Value>,
}

impl DataverseConnector {
    /// Create a new Dataverse connector
    pub fn new(base_url: String, access_token: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
            access_token,
        }
    }

    /// Get all tables (entities) in the Dataverse environment
    pub async fn get_tables(&self) -> Result<Vec<DataverseTable>> {
        let url = format!("{}/api/data/v9.2/EntityDefinitions?$select=LogicalName,DisplayName,Description", self.base_url);

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to fetch tables from Dataverse")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Dataverse API error: {}", response.status()));
        }

        let data: serde_json::Value = response.json().await?;
        let tables = data["value"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|table| {
                Some(DataverseTable {
                    logical_name: table["LogicalName"].as_str()?.to_string(),
                    display_name: table["DisplayName"].as_object()?
                        .get("UserLocalizedLabel")?
                        .get("Label")?
                        .as_str()?
                        .to_string(),
                    description: table["Description"].as_object()
                        .and_then(|d| d.get("UserLocalizedLabel"))
                        .and_then(|l| l.get("Label"))
                        .and_then(|l| l.as_str())
                        .map(|s| s.to_string()),
                })
            })
            .collect();

        Ok(tables)
    }

    /// Get records from a specific table
    pub async fn get_records(&self, table_name: &str, top: Option<u32>) -> Result<Vec<DataverseRecord>> {
        let mut url = format!("{}/api/data/v9.2/{}", self.base_url, table_name);
        if let Some(limit) = top {
            url.push_str(&format!("?$top={}", limit));
        }

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to fetch records from Dataverse")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Dataverse API error: {}", response.status()));
        }

        let data: serde_json::Value = response.json().await?;
        let records = data["value"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|record| {
                let id = record[format!("{}id", table_name)].as_str()?.to_string();
                let fields = record.as_object()?
                    .iter()
                    .filter(|(k, _)| !k.ends_with("id") && *k != "@odata.etag" && *k != "@odata.context")
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                Some(DataverseRecord {
                    id,
                    logical_name: table_name.to_string(),
                    fields,
                })
            })
            .collect();

        Ok(records)
    }

    /// Query records with OData filter
    pub async fn query_records(&self, table_name: &str, filter: &str) -> Result<Vec<DataverseRecord>> {
        let url = format!(
            "{}/api/data/v9.2/{}?$filter={}",
            self.base_url, table_name, urlencoding::encode(filter)
        );

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to query records from Dataverse")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Dataverse API error: {}", response.status()));
        }

        let data: serde_json::Value = response.json().await?;
        let records = data["value"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|record| {
                let id = record[format!("{}id", table_name)].as_str()?.to_string();
                let fields = record.as_object()?
                    .iter()
                    .filter(|(k, _)| !k.ends_with("id") && *k != "@odata.etag" && *k != "@odata.context")
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                Some(DataverseRecord {
                    id,
                    logical_name: table_name.to_string(),
                    fields,
                })
            })
            .collect();

        Ok(records)
    }

    /// Create a new record in a table
    pub async fn create_record(&self, table_name: &str, data: HashMap<String, serde_json::Value>) -> Result<String> {
        let url = format!("{}/api/data/v9.2/{}", self.base_url, table_name);

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("Content-Type", "application/json")
            .json(&data)
            .send()
            .await
            .context("Failed to create record in Dataverse")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to create record: {}", response.status()));
        }

        // Extract the record ID from the response headers
        let location = response.headers()
            .get("odata-entityid")
            .and_then(|h| h.to_str().ok())
            .context("No entity ID returned")?;

        // Extract ID from the URL
        let id = location.split('/').last()
            .context("Invalid entity ID format")?
            .to_string();

        Ok(id)
    }
}
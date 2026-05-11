use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Serialize, Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
}

pub async fn get_azure_token(resource: &str) -> Result<String> {
    let client_id = env::var("AZURE_CLIENT_ID").context("AZURE_CLIENT_ID not found")?;
    let tenant_id = env::var("AZURE_TENANT_ID").context("AZURE_TENANT_ID not found")?;
    let client_secret = env::var("AZURE_CLIENT_SECRET").context("AZURE_CLIENT_SECRET not found")?;

    let url = format!("https://login.microsoftonline.com/{}/oauth2/v2.0/token", tenant_id);
    
    let client = Client::new();
    let mut params = std::collections::HashMap::new();
    params.insert("client_id", client_id);
    params.insert("scope", format!("{}/.default", resource));
    params.insert("client_secret", client_secret);
    params.insert("grant_type", "client_credentials".to_string());

    let response = client
        .post(&url)
        .form(&params)
        .send()
        .await
        .context("Failed to send token request")?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("Failed to get token: {} - {}", status, error_text));
    }

    let token_data: TokenResponse = response.json().await.context("Failed to parse token response")?;
    Ok(token_data.access_token)
}

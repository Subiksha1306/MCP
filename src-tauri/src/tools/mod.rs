use crate::permissions::PermissionLevel;
use serde_json::{json, Value};
use std::collections::HashMap;

pub type ToolFn = fn(&Value) -> Value;


pub struct Tool {
    pub name: String,
    pub func: ToolFn,
    pub required_permission: PermissionLevel,
}

pub fn get_tools() -> HashMap<String, Tool> {
    let mut tools: HashMap<String, Tool> = HashMap::new();

    let tool_list = vec![
        Tool { name: "echo".to_string(), func: echo, required_permission: PermissionLevel::ReadOnly },
        Tool { name: "search".to_string(), func: search, required_permission: PermissionLevel::ReadOnly },
        Tool { name: "search_documents".to_string(), func: search_documents, required_permission: PermissionLevel::ReadOnly },
        Tool { name: "fetch_sharepoint_files".to_string(), func: fetch_sharepoint_files, required_permission: PermissionLevel::ReadOnly },
        Tool { name: "query_dataverse".to_string(), func: query_dataverse, required_permission: PermissionLevel::ReadOnly },
    ];

    for t in tool_list {
        tools.insert(t.name.clone(), t);
    }

    tools
}

fn echo(args: &Value) -> Value {
    json!({
        "echo": args
    })
}

fn search(args: &Value) -> Value {
    let query = args["query"].as_str().unwrap_or("");

    json!({
        "results": [format!("Found docs for: {}", query)]
    })
}

fn search_documents(args: &Value) -> Value {
    let query = args["query"].as_str().unwrap_or("");
    let top_k = args["top_k"].as_u64().unwrap_or(5) as usize;

    // Mock implementation - in production, this would use the vector store
    json!({
        "query": query,
        "results": [
            {
                "content": "Sample document content related to the query",
                "source": "sample.pdf",
                "similarity": 0.85
            }
        ],
        "top_k": top_k
    })
}

fn fetch_sharepoint_files(args: &Value) -> Value {
    let library_name = args["library_name"].as_str().unwrap_or("Documents");

    // Mock implementation - in production, this would use SharePointConnector
    json!({
        "library": library_name,
        "files": [
            {
                "name": "document1.docx",
                "url": "/sites/test/Shared Documents/document1.docx",
                "size": 1024000,
                "last_modified": "2024-01-15T10:30:00Z"
            }
        ]
    })
}

fn query_dataverse(args: &Value) -> Value {
    let table_name = args["table_name"].as_str().unwrap_or("accounts");
    let filter = args["filter"].as_str();

    // Mock implementation - in production, this would use DataverseConnector
    let mut response = json!({
        "table": table_name,
        "records": [
            {
                "id": "12345",
                "name": "Sample Account",
                "type": "Customer"
            }
        ]
    });

    if let Some(f) = filter {
        response["filter"] = json!(f);
    }

    response
}
use rusqlite::{params, Connection, Result};
use std::path::Path;

pub struct MemoryDB {
    conn: Connection,
}

impl MemoryDB {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        
        // Initialize tables
        conn.execute(
            "CREATE TABLE IF NOT EXISTS chat_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS sync_metadata (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                connector TEXT NOT NULL UNIQUE,
                last_sync TEXT,
                doc_count INTEGER DEFAULT 0
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS enterprise_data (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                source_type TEXT NOT NULL,
                category TEXT,
                summary TEXT,
                metadata TEXT,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        Ok(Self { conn })
    }

    pub fn save_message(&self, role: &str, content: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO chat_history (role, content) VALUES (?1, ?2)",
            params![role, content],
        )?;
        Ok(())
    }

    pub fn get_chat_history(&self, limit: usize) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT role, content FROM chat_history ORDER BY timestamp DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        results.reverse();
        Ok(results)
    }

    pub fn update_sync_stats(&self, connector: &str, count: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sync_metadata (connector, last_sync, doc_count)
             VALUES (?1, CURRENT_TIMESTAMP, ?2)
             ON CONFLICT(connector) DO UPDATE SET 
             last_sync = CURRENT_TIMESTAMP,
             doc_count = ?2",
            params![connector, count],
        )?;
        Ok(())
    }

    pub fn get_sync_stats(&self) -> Result<Vec<(String, String, i64)>> {
        let mut stmt = self.conn.prepare("SELECT connector, last_sync, doc_count FROM sync_metadata")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    // --- Discovery Methods ---

    pub fn save_discovery_item(
        &self,
        id: &str,
        title: &str,
        source: &str,
        category: &str,
        summary: &str,
        metadata: &str
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO enterprise_data (id, title, source_type, category, summary, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
             title = ?2,
             category = ?4,
             summary = ?5,
             metadata = ?6,
             timestamp = CURRENT_TIMESTAMP",
            params![id, title, source, category, summary, metadata],
        )?;
        Ok(())
    }

    pub fn get_paginated_discovery(&self, page: u32, limit: u32) -> Result<Vec<serde_json::Value>> {
        let offset = (page - 1) * limit;
        let mut stmt = self.conn.prepare(
            "SELECT id, title, source_type, category, summary, metadata, timestamp 
             FROM enterprise_data 
             ORDER BY timestamp DESC 
             LIMIT ?1 OFFSET ?2"
        )?;
        
        let rows = stmt.query_map(params![limit, offset], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "title": row.get::<_, String>(1)?,
                "source_type": row.get::<_, String>(2)?,
                "category": row.get::<_, String>(3).unwrap_or_default(),
                "summary": row.get::<_, String>(4).unwrap_or_default(),
                "metadata": row.get::<_, String>(5).unwrap_or_default(),
                "timestamp": row.get::<_, String>(6)?
            }))
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn search_discovery(&self, query: &str) -> Result<Vec<serde_json::Value>> {
        let sql_query = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            "SELECT id, title, source_type, category, summary, metadata, timestamp 
             FROM enterprise_data 
             WHERE title LIKE ?1 OR summary LIKE ?1 OR category LIKE ?1
             ORDER BY timestamp DESC LIMIT 20"
        )?;
        
        let rows = stmt.query_map(params![sql_query], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "title": row.get::<_, String>(1)?,
                "source_type": row.get::<_, String>(2)?,
                "category": row.get::<_, String>(3).unwrap_or_default(),
                "summary": row.get::<_, String>(4).unwrap_or_default(),
                "metadata": row.get::<_, String>(5).unwrap_or_default(),
                "timestamp": row.get::<_, String>(6)?
            }))
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn seed_initial_data(&self) -> Result<()> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM enterprise_data",
            [],
            |row| row.get(0),
        )?;

        if count > 0 {
            return Ok(());
        }

        println!("🌱 Seeding initial discovery data...");

        let mock_items = vec![
            ("sp_handbook", "Employee_Handbook_2026.pdf", "SharePoint", "Policy", "Official organizational policies, conduct codes, and benefits overview for the 2026 fiscal year."),
            ("sp_finance", "Q3_Financial_Projections.xlsx", "SharePoint", "Finance", "Internal financial models and revenue forecasts for the current fiscal quarter."),
            ("sp_onboarding", "Client_Onboarding_Protocol.docx", "SharePoint", "Operations", "Standard operating procedures for managing new enterprise client integrations."),
            ("sp_arch", "Nexus_Architecture_V2.pdf", "SharePoint", "Engineering", "Deep technical documentation for the Nexus AI core and data orchestration layer."),
            ("sp_security", "Zero_Trust_Strategy.pdf", "SharePoint", "Security", "Comprehensive roadmap for implementing Zero Trust security across all enterprise endpoints."),
        ];

        for (id, title, source, cat, sum) in mock_items {
            let metadata = serde_json::json!({
                "file_size": 1024 * rand::random::<u32>() % 5000,
                "author": "Nexus System",
                "version": "1.0"
            }).to_string();

            self.save_discovery_item(id, title, source, cat, sum, &metadata)?;
        }

        Ok(())
    }
}

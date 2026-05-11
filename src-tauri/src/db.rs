use rusqlite::{params, Connection, Result};
use std::path::Path;

// Utility to convert f32 vector to bytes
fn vec_to_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|&f| f.to_le_bytes().to_vec()).collect()
}

// Utility to convert bytes back to f32 vector
fn bytes_to_vec(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

pub struct MemoryDB {
    conn: Connection,
}

impl MemoryDB {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        
        // Initialize custom cosine similarity function for SQLite
        conn.create_scalar_function("cosine_similarity", 2, rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC, move |ctx| {
            let vec1_bytes = ctx.get::<Vec<u8>>(0)?;
            let vec2_bytes = ctx.get::<Vec<u8>>(1)?;
            
            let vec1 = bytes_to_vec(&vec1_bytes);
            let vec2 = bytes_to_vec(&vec2_bytes);
            
            if vec1.len() != vec2.len() || vec1.is_empty() {
                return Ok(0.0f64);
            }
            
            let dot_product: f32 = vec1.iter().zip(vec2.iter()).map(|(a, b)| a * b).sum();
            let norm1: f32 = vec1.iter().map(|a| a * a).sum::<f32>().sqrt();
            let norm2: f32 = vec2.iter().map(|b| b * b).sum::<f32>().sqrt();
            
            let similarity = dot_product / (norm1 * norm2);
            Ok(similarity as f64)
        })?;

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

        conn.execute(
            "CREATE TABLE IF NOT EXISTS document_embeddings (
                id TEXT PRIMARY KEY,
                embedding BLOB NOT NULL,
                FOREIGN KEY(id) REFERENCES enterprise_data(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // RAG: Document chunks table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS document_chunks (
                id TEXT PRIMARY KEY,
                doc_id TEXT NOT NULL,
                chunk_index INTEGER NOT NULL,
                content TEXT NOT NULL,
                content_hash TEXT,
                embedding BLOB,
                FOREIGN KEY(doc_id) REFERENCES enterprise_data(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Security: Audit log
        conn.execute(
            "CREATE TABLE IF NOT EXISTS audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                user_id TEXT DEFAULT 'system',
                action TEXT NOT NULL,
                resource TEXT,
                status TEXT DEFAULT 'success',
                details TEXT
            )",
            [],
        )?;

        // Performance: Query log
        conn.execute(
            "CREATE TABLE IF NOT EXISTS query_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                prompt TEXT NOT NULL,
                chunks_retrieved INTEGER DEFAULT 0,
                response_time_ms INTEGER DEFAULT 0,
                sources TEXT
            )",
            [],
        )?;

        // Add content_hash to enterprise_data if not exists
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_chunks_doc_id ON document_chunks(doc_id)",
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
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
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
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
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

    pub fn save_embedding(&self, id: &str, embedding: &[f32]) -> Result<()> {
        let bytes = vec_to_bytes(embedding);
        self.conn.execute(
            "INSERT INTO document_embeddings (id, embedding) VALUES (?1, ?2)
             ON CONFLICT(id) DO UPDATE SET embedding = ?2",
            params![id, bytes],
        )?;
        Ok(())
    }

    pub fn semantic_search(&self, query_embedding: &[f32], limit: u32) -> Result<Vec<serde_json::Value>> {
        let bytes = vec_to_bytes(query_embedding);
        let mut stmt = self.conn.prepare(
            "SELECT e.id, e.title, e.source_type, e.category, e.summary, e.metadata, e.timestamp,
                    cosine_similarity(v.embedding, ?1) as similarity
             FROM enterprise_data e
             JOIN document_embeddings v ON e.id = v.id
             ORDER BY similarity DESC
             LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![bytes, limit], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "title": row.get::<_, String>(1)?,
                "source_type": row.get::<_, String>(2)?,
                "category": row.get::<_, String>(3).unwrap_or_default(),
                "summary": row.get::<_, String>(4).unwrap_or_default(),
                "metadata": row.get::<_, String>(5).unwrap_or_default(),
                "timestamp": row.get::<_, String>(6)?,
                "similarity": row.get::<_, f64>(7)?
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
            |row| row.get::<_, i64>(0),
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
            ("dv_accounts", "Active_Accounts_Master", "Dataverse", "CRM", "Unified view of high-priority corporate accounts including revenue tiers and relationship health status."),
            ("dv_leads", "Inbound_Leads_Q2", "Dataverse", "Sales", "Consolidated list of marketing-qualified leads captured through the global enterprise portal."),
            ("dv_tickets", "Priority_Support_Queue", "Dataverse", "Service", "Real-time list of high-severity support tickets requiring immediate technical intervention."),
        ];

        for (id, title, source, cat, sum) in mock_items {
            let metadata = match source {
                "SharePoint" => serde_json::json!({
                    "file_size": format!("{} KB", 1024 + rand::random::<u32>() % 5000),
                    "author": "Nexus System",
                    "version": "1.0",
                    "permissions": "Read/Write"
                }).to_string(),
                "Dataverse" => serde_json::json!({
                    "record_count": rand::random::<u32>() % 500,
                    "last_modified_by": "System Architect",
                    "entity_type": "Organization",
                    "sync_status": "Synchronized"
                }).to_string(),
                _ => "{}".to_string()
            };

            self.save_discovery_item(id, title, source, cat, sum, &metadata)?;
            
            // Generate a mock chunk for RAG to function in Demo Mode
            let chunk_id = format!("{}_chunk_0", id);
            let content = format!("Document Context for {}:\nCategory: {}\nSummary: {}\nThis is simulated content demonstrating the RAG retrieval pipeline.", title, cat, sum);
            self.save_chunk(&chunk_id, id, 0, &content, "mock_hash")?;
            
            // Save dummy embedding (Gemini uses 768 dimensions)
            let dummy_embedding = vec![0.01f32; 768];
            self.save_chunk_embedding(&chunk_id, &dummy_embedding)?;
            
            // Also save embedding for the document itself
            self.save_embedding(id, &dummy_embedding)?;
        }

        Ok(())
    }

    /// Fetch all enterprise data for full-context AI queries
    pub fn get_all_enterprise_data(&self) -> Result<Vec<(String, String, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT title, source_type, summary, metadata FROM enterprise_data ORDER BY timestamp DESC"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2).unwrap_or_default(),
                row.get::<_, String>(3).unwrap_or_default(),
            ))
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    // ============================
    // RAG: Document Chunk Methods
    // ============================

    pub fn save_chunk(&self, id: &str, doc_id: &str, chunk_index: usize, content: &str, content_hash: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO document_chunks (id, doc_id, chunk_index, content, content_hash)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET content = ?4, content_hash = ?5",
            params![id, doc_id, chunk_index as i64, content, content_hash],
        )?;
        Ok(())
    }

    pub fn save_chunk_embedding(&self, chunk_id: &str, embedding: &[f32]) -> Result<()> {
        let bytes = vec_to_bytes(embedding);
        self.conn.execute(
            "UPDATE document_chunks SET embedding = ?1 WHERE id = ?2",
            params![bytes, chunk_id],
        )?;
        Ok(())
    }

    pub fn get_chunk_count(&self) -> Result<i64> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM document_chunks WHERE embedding IS NOT NULL",
            [],
            |row| row.get::<_, i64>(0),
        )
    }

    pub fn get_doc_count_by_source(&self) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT source_type, COUNT(*) FROM enterprise_data GROUP BY source_type"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Semantic search over document chunks — the core RAG retrieval
    pub fn semantic_chunk_search(
        &self, 
        query_embedding: &[f32], 
        top_k: u32,
        source_filter: Option<String>,
        category_filter: Option<String>
    ) -> Result<Vec<serde_json::Value>> {
        let bytes = vec_to_bytes(query_embedding);
        let s_filter = source_filter.unwrap_or_default();
        let c_filter = category_filter.unwrap_or_default();
        
        let mut sql = "SELECT c.id, c.doc_id, c.content, c.chunk_index,
                    e.title, e.source_type, e.category,
                    cosine_similarity(c.embedding, ?1) as similarity
             FROM document_chunks c
             JOIN enterprise_data e ON c.doc_id = e.id
             WHERE c.embedding IS NOT NULL".to_string();

        if !s_filter.is_empty() {
            sql.push_str(" AND e.source_type = ?3");
        } else {
            sql.push_str(" AND (?3 = '' OR 1=1)");
        }
        
        if !c_filter.is_empty() {
            sql.push_str(" AND e.category = ?4");
        } else {
            sql.push_str(" AND (?4 = '' OR 1=1)");
        }
        
        sql.push_str(" ORDER BY similarity DESC LIMIT ?2");

        let mut stmt = self.conn.prepare(&sql)?;

        let rows = stmt.query_map(params![
            bytes, 
            top_k, 
            s_filter, 
            c_filter
        ], |row| {
            Ok(serde_json::json!({
                "chunk_id": row.get::<_, String>(0)?,
                "doc_id": row.get::<_, String>(1)?,
                "content": row.get::<_, String>(2)?,
                "chunk_index": row.get::<_, i64>(3)?,
                "doc_title": row.get::<_, String>(4)?,
                "source_type": row.get::<_, String>(5)?,
                "category": row.get::<_, String>(6).unwrap_or_default(),
                "similarity": row.get::<_, f64>(7)?
            }))
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Delete all chunks for a document (for re-indexing)
    pub fn delete_chunks_for_doc(&self, doc_id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM document_chunks WHERE doc_id = ?1",
            params![doc_id],
        )?;
        Ok(())
    }

    // ============================
    // Security: Audit Log Methods
    // ============================

    pub fn log_audit(&self, user_id: &str, action: &str, resource: &str, status: &str, details: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO audit_log (user_id, action, resource, status, details) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![user_id, action, resource, status, details],
        )?;
        Ok(())
    }

    pub fn get_audit_logs(&self, limit: u32) -> Result<Vec<serde_json::Value>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, user_id, action, resource, status, details
             FROM audit_log ORDER BY timestamp DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "timestamp": row.get::<_, String>(1)?,
                "user_id": row.get::<_, String>(2)?,
                "action": row.get::<_, String>(3)?,
                "resource": row.get::<_, String>(4).unwrap_or_default(),
                "status": row.get::<_, String>(5)?,
                "details": row.get::<_, String>(6).unwrap_or_default()
            }))
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    // ============================
    // Performance: Query Log Methods
    // ============================

    pub fn log_query(&self, prompt: &str, chunks_retrieved: i64, response_time_ms: i64, sources: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO query_log (prompt, chunks_retrieved, response_time_ms, sources) VALUES (?1, ?2, ?3, ?4)",
            params![prompt, chunks_retrieved, response_time_ms, sources],
        )?;
        Ok(())
    }

    pub fn get_query_history(&self, limit: u32) -> Result<Vec<serde_json::Value>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, prompt, chunks_retrieved, response_time_ms, sources
             FROM query_log ORDER BY timestamp DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "timestamp": row.get::<_, String>(1)?,
                "prompt": row.get::<_, String>(2)?,
                "chunks_retrieved": row.get::<_, i64>(3)?,
                "response_time_ms": row.get::<_, i64>(4)?,
                "sources": row.get::<_, String>(5).unwrap_or_default()
            }))
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

}

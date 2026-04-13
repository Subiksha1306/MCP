use anyhow::{Result, Context};
use std::collections::HashMap;

/// Document chunk for embedding and storage
#[derive(Debug, Clone)]
pub struct DocumentChunk {
    pub id: String,
    pub content: String,
    pub metadata: HashMap<String, String>,
    pub embedding: Option<Vec<f32>>,
}

/// Embedding service for generating vector representations
pub struct EmbeddingService {
    // For now, we'll use a simple mock implementation
    // In production, this would integrate with Claude or other embedding models
}

impl EmbeddingService {
    pub fn new() -> Self {
        Self {}
    }

    /// Generate embeddings for text (mock implementation)
    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        // Mock embedding - in production, this would call Claude or another embedding API
        // For now, we'll create a simple hash-based vector
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let hash = hasher.finish();

        // Create a 384-dimensional vector (similar to some embedding models)
        let mut embedding = Vec::with_capacity(384);
        for i in 0..384 {
            let value = ((hash.wrapping_mul(i as u64 + 1)) % 1000) as f32 / 1000.0;
            embedding.push(value);
        }

        Ok(embedding)
    }

    /// Generate embeddings for multiple texts
    pub async fn generate_embeddings(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut embeddings = Vec::new();
        for text in texts {
            let embedding = self.generate_embedding(text).await?;
            embeddings.push(embedding);
        }
        Ok(embeddings)
    }
}

/// Document processor for chunking and preparing documents
pub struct DocumentProcessor {
    chunk_size: usize,
    overlap: usize,
}

impl DocumentProcessor {
    pub fn new(chunk_size: usize, overlap: usize) -> Self {
        Self { chunk_size, overlap }
    }

    /// Split text into chunks with overlap
    pub fn chunk_text(&self, text: &str, source: &str) -> Vec<DocumentChunk> {
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut chunks = Vec::new();
        let mut start = 0;

        while start < words.len() {
            let end = (start + self.chunk_size).min(words.len());
            let chunk_text = words[start..end].join(" ");

            let chunk = DocumentChunk {
                id: format!("{}_{}", source, start),
                content: chunk_text,
                metadata: HashMap::from([
                    ("source".to_string(), source.to_string()),
                    ("chunk_index".to_string(), start.to_string()),
                ]),
                embedding: None,
            };

            chunks.push(chunk);

            if end == words.len() {
                break;
            }

            start = end.saturating_sub(self.overlap);
        }

        chunks
    }

    /// Process a document and generate chunks with embeddings
    pub async fn process_document(
        &self,
        content: &str,
        source: &str,
        embedding_service: &EmbeddingService,
    ) -> Result<Vec<DocumentChunk>> {
        let mut chunks = self.chunk_text(content, source);

        // Generate embeddings for each chunk
        for chunk in &mut chunks {
            let embedding = embedding_service.generate_embedding(&chunk.content).await?;
            chunk.embedding = Some(embedding);
        }

        Ok(chunks)
    }
}

/// Vector store for similarity search
pub struct VectorStore {
    chunks: Vec<DocumentChunk>,
}

impl VectorStore {
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
        }
    }

    /// Add chunks to the store
    pub fn add_chunks(&mut self, chunks: Vec<DocumentChunk>) {
        self.chunks.extend(chunks);
    }

    /// Search for similar chunks using cosine similarity
    pub fn search(&self, query_embedding: &[f32], top_k: usize) -> Vec<(DocumentChunk, f32)> {
        let mut results: Vec<(DocumentChunk, f32)> = self.chunks
            .iter()
            .filter_map(|chunk| {
                chunk.embedding.as_ref().map(|emb| {
                    let similarity = cosine_similarity(query_embedding, emb);
                    (chunk.clone(), similarity)
                })
            })
            .collect();

        // Sort by similarity (descending)
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Return top k results
        results.into_iter().take(top_k).collect()
    }

    /// Get all chunks
    pub fn get_all_chunks(&self) -> &[DocumentChunk] {
        &self.chunks
    }
}

/// Calculate cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let mut dot_product = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;

    for i in 0..a.len() {
        dot_product += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot_product / (norm_a.sqrt() * norm_b.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_embedding_generation() {
        let service = EmbeddingService::new();
        let embedding = service.generate_embedding("test text").await.unwrap();
        assert_eq!(embedding.len(), 384);
    }

    #[test]
    fn test_document_chunking() {
        let processor = DocumentProcessor::new(10, 2);
        let chunks = processor.chunk_text("This is a test document with many words to chunk properly", "test.txt");
        assert!(!chunks.is_empty());
        assert!(chunks[0].content.contains("This is a test"));
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let similarity = cosine_similarity(&a, &b);
        assert!((similarity - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        let similarity2 = cosine_similarity(&a, &c);
        assert!(similarity2.abs() < 0.001);
    }
}
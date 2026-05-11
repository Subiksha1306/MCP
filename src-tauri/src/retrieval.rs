use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use uuid::Uuid;

/// A single chunk of a document, ready for embedding and retrieval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChunk {
    pub id: String,
    pub doc_id: String,
    pub chunk_index: usize,
    pub content: String,
    pub content_hash: String,
}

/// Result from a semantic chunk search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkSearchResult {
    pub chunk_id: String,
    pub doc_id: String,
    pub doc_title: String,
    pub source_type: String,
    pub content: String,
    pub similarity: f64,
    pub chunk_index: usize,
}

/// Configuration for chunking behavior
pub struct ChunkConfig {
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub max_chunks_per_doc: usize,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            chunk_size: 500,
            chunk_overlap: 50,
            max_chunks_per_doc: 100,
        }
    }
}

/// Hash content for deduplication
pub fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Split text into overlapping chunks for embedding
pub fn chunk_text(content: &str, config: &ChunkConfig) -> Vec<String> {
    let content = content.trim();
    if content.is_empty() {
        return vec![];
    }

    // For very short content, return as single chunk
    if content.len() <= config.chunk_size {
        return vec![content.to_string()];
    }

    let mut chunks = Vec::new();
    let words: Vec<&str> = content.split_whitespace().collect();

    if words.is_empty() {
        return vec![];
    }

    // Calculate approximate words per chunk (assuming ~5 chars per word)
    let words_per_chunk = config.chunk_size / 5;
    let overlap_words = config.chunk_overlap / 5;
    let step = if words_per_chunk > overlap_words {
        words_per_chunk - overlap_words
    } else {
        1
    };

    let mut start = 0;
    while start < words.len() && chunks.len() < config.max_chunks_per_doc {
        let end = (start + words_per_chunk).min(words.len());
        let chunk: String = words[start..end].join(" ");
        if !chunk.trim().is_empty() {
            chunks.push(chunk);
        }
        start += step;
        if end >= words.len() {
            break;
        }
    }

    chunks
}

/// Create DocumentChunk structs from raw text
pub fn create_chunks(doc_id: &str, content: &str, config: &ChunkConfig) -> Vec<DocumentChunk> {
    let raw_chunks = chunk_text(content, config);
    raw_chunks
        .into_iter()
        .enumerate()
        .map(|(i, text)| DocumentChunk {
            id: format!("chunk_{}_{}", doc_id, Uuid::new_v4().to_string()[..8].to_string()),
            doc_id: doc_id.to_string(),
            chunk_index: i,
            content_hash: hash_content(&text),
            content: text,
        })
        .collect()
}

/// Extract text content from raw file bytes based on file type
pub fn extract_text_content(filename: &str, bytes: &[u8]) -> Result<String, String> {
    let lower = filename.to_lowercase();

    if lower.ends_with(".txt") || lower.ends_with(".log") || lower.ends_with(".md") {
        // Plain text
        String::from_utf8(bytes.to_vec())
            .map_err(|e| format!("UTF-8 decode error: {}", e))
    } else if lower.ends_with(".csv") {
        // CSV — return as-is text
        String::from_utf8(bytes.to_vec())
            .map_err(|e| format!("CSV decode error: {}", e))
    } else if lower.ends_with(".docx") {
        extract_docx_text(bytes)
    } else if lower.ends_with(".xlsx") {
        extract_xlsx_text(bytes)
    } else if lower.ends_with(".pdf") {
        extract_pdf_text(bytes)
    } else {
        // Attempt plain text as fallback
        String::from_utf8(bytes.to_vec())
            .map_err(|_| format!("Unsupported file type: {}", filename))
    }
}

/// Extract text from DOCX (Office Open XML)
fn extract_docx_text(bytes: &[u8]) -> Result<String, String> {
    use std::io::Cursor;

    let reader = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| format!("Failed to open DOCX as ZIP: {}", e))?;

    // Read word/document.xml
    let mut doc_xml = String::new();
    if let Ok(mut file) = archive.by_name("word/document.xml") {
        use std::io::Read;
        file.read_to_string(&mut doc_xml)
            .map_err(|e| format!("Failed to read document.xml: {}", e))?;
    } else {
        return Err("No word/document.xml found in DOCX".to_string());
    }

    // Strip XML tags to extract text
    Ok(strip_xml_tags(&doc_xml))
}

/// Extract text from XLSX (Office Open XML Spreadsheet)
fn extract_xlsx_text(bytes: &[u8]) -> Result<String, String> {
    use std::io::Cursor;

    let reader = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| format!("Failed to open XLSX as ZIP: {}", e))?;

    // First read shared strings
    let mut shared_strings: Vec<String> = Vec::new();
    if let Ok(mut file) = archive.by_name("xl/sharedStrings.xml") {
        use std::io::Read;
        let mut xml = String::new();
        file.read_to_string(&mut xml).ok();
        // Extract <t> tags from shared strings
        shared_strings = extract_xml_values(&xml, "t");
    }

    // Read sheet1.xml for cell data
    let mut output = String::new();
    if let Ok(mut file) = archive.by_name("xl/worksheets/sheet1.xml") {
        use std::io::Read;
        let mut xml = String::new();
        file.read_to_string(&mut xml).ok();

        // Extract cell values
        let values = extract_xml_values(&xml, "v");
        for val in values {
            // Check if it's a shared string reference
            if let Ok(idx) = val.parse::<usize>() {
                if let Some(s) = shared_strings.get(idx) {
                    output.push_str(s);
                    output.push('\t');
                    continue;
                }
            }
            output.push_str(&val);
            output.push('\t');
        }
    }

    if output.is_empty() {
        Err("No sheet data found in XLSX".to_string())
    } else {
        Ok(output)
    }
}

/// Extract text from PDF using pdf-extract
fn extract_pdf_text(bytes: &[u8]) -> Result<String, String> {
    pdf_extract::extract_text_from_mem(bytes)
        .map_err(|e| format!("PDF extraction error: {}", e))
}

/// Strip XML tags, returning only text content
fn strip_xml_tags(xml: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    let mut last_was_space = true;

    for ch in xml.chars() {
        match ch {
            '<' => {
                in_tag = true;
            }
            '>' => {
                in_tag = false;
                // Add space after closing tags to separate words
                if !last_was_space {
                    result.push(' ');
                    last_was_space = true;
                }
            }
            _ if !in_tag => {
                result.push(ch);
                last_was_space = ch.is_whitespace();
            }
            _ => {}
        }
    }

    // Clean up excessive whitespace
    result
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Extract values from specific XML tags (simple parser)
fn extract_xml_values(xml: &str, tag: &str) -> Vec<String> {
    let open = format!("<{}", tag);
    let close = format!("</{}>", tag);
    let mut results = Vec::new();
    let mut search_from = 0;

    while let Some(start_pos) = xml[search_from..].find(&open) {
        let abs_start = search_from + start_pos;
        // Find the end of the opening tag
        if let Some(tag_end) = xml[abs_start..].find('>') {
            let content_start = abs_start + tag_end + 1;
            // Find closing tag
            if let Some(end_pos) = xml[content_start..].find(&close) {
                let content = &xml[content_start..content_start + end_pos];
                results.push(content.to_string());
                search_from = content_start + end_pos + close.len();
            } else {
                break;
            }
        } else {
            break;
        }
    }

    results
}

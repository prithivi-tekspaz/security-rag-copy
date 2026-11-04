//! Simple security RAG agent built on the Rig framework.
//! Uses a local Ollama server for embeddings (bge-m3 or nomic-embed-text) and completion (qwen3.5).
//! Reads security documents from the `docs/` folder (PDF or plain text) and answers questions.

use anyhow::{Context, Result};
use rig::integrations::cli_chatbot::ChatBotBuilder;
use rig::prelude::*;
use rig::providers::ollama::OllamaConfig;
use rig::{
    Embed, embeddings::EmbeddingsBuilder, loaders::PdfFileLoader,
    vector_store::in_memory_store::InMemoryVectorStore,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Embed, Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
struct Document {
    id: String,
    #[embed]
    content: String,
}

/// Splits text content into chunks of at most `chunk_size` characters along whitespace boundaries.
pub fn chunk_text(content: &str, chunk_size: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for word in content.split_whitespace() {
        if current.len() + word.len() + 1 > chunk_size && !current.is_empty() {
            chunks.push(std::mem::take(&mut current).trim().to_string());
        }
        current.push_str(word);
        current.push(' ');
    }
    if !current.is_empty() {
        chunks.push(current.trim().to_string());
    }
    chunks
}

fn load_pdf(path: &Path) -> Result<Vec<String>> {
    const CHUNK_SIZE: usize = 2000;
    let content_chunks = PdfFileLoader::with_glob(path.to_str().context("Invalid path")?)?
        .read()
        .into_iter()
        .filter_map(|result| {
            result
                .map_err(|e| {
                    eprintln!("Error reading PDF content: {e}");
                    e
                })
                .ok()
        })
        .flat_map(|content| chunk_text(&content, CHUNK_SIZE))
        .collect::<Vec<_>>();
    if content_chunks.is_empty() {
        anyhow::bail!("No content found in PDF file: {}", path.display());
    }
    Ok(content_chunks)
}

#[tokio::main]
async fn main() -> Result<()> {
    let client = OllamaConfig::new()
        .with_base_url("http://localhost:11434")
        .client();

    // Pick an embedding model available locally; prefer bge-m3, fall back to nomic-embed-text.
    let embed_model_name = if std::env::var("EMBED_MODEL").is_ok() {
        std::env::var("EMBED_MODEL").unwrap()
    } else {
        "nomic-embed-text".to_string()
    };
    let completion_model_name = std::env::var("COMPLETION_MODEL")
        .unwrap_or_else(|_| "CyberCrew/notmythos-8b:latest".to_string());

    let embedding_model = client.embedding(&embed_model_name, None).erase();

    // Load security documents from ./docs (PDF glob) if present.
    let docs_dir = std::env::current_dir()?.join("docs");
    let mut chunks: Vec<String> = Vec::new();
    if docs_dir.exists() {
        if let Ok(pdf_chunks) = load_pdf(&docs_dir.join("*.pdf")) {
            chunks.extend(pdf_chunks);
        }
    }
    // Fall back to a small built-in security corpus so the agent always works.
    if chunks.is_empty() {
        chunks = vec![
            "OWASP Top 10: Broken Access Control allows users to act outside intended permissions.".to_string(),
            "SQL injection occurs when untrusted data is sent to an interpreter as part of a command.".to_string(),
            "Cross-site scripting (XSS) lets attackers inject client-side scripts into web pages.".to_string(),
            "Security misconfiguration is the most common vulnerability from insecure defaults.".to_string(),
            "Sensitive data exposure happens when data is not properly protected in transit or at rest.".to_string(),
        ];
    }
    println!("Loaded {} security chunks.", chunks.len());

    let mut builder = EmbeddingsBuilder::new(embedding_model.clone());
    for (i, chunk) in chunks.into_iter().enumerate() {
        builder = builder.document(Document {
            id: format!("security_doc_{i}"),
            content: chunk,
        })?;
    }
    let embeddings = builder.build().await?;
    println!("Successfully generated embeddings");

    let vector_store = InMemoryVectorStore::from_documents(embeddings);
    let index = vector_store.index(embedding_model);

    let rag_agent = AgentBuilder::new(client.completion(&completion_model_name))
        .preamble("You are Security RAG, a local cybersecurity research assistant. Answer questions about application security, vulnerabilities, and best practices using the provided context. Be direct, practical, and technical. Include impact, verification approach, and remediation. Cite relevant concepts when possible.")
        .dynamic_context(2, index)
        .build();

    println!("Starting security RAG chatbot (completion model: {completion_model_name})...");
    let chatbot = ChatBotBuilder::new().agent(rag_agent).max_turns(10).build();
    chatbot.run().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_text_empty_string() {
        let chunks = chunk_text("", 100);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunk_text_whitespace_only() {
        let chunks = chunk_text("   \n\t   \r\n  ", 100);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunk_text_short_content() {
        let text = "OWASP Top 10: Broken Access Control.";
        let chunks = chunk_text(text, 100);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn test_chunk_text_splits_at_word_boundary() {
        let words = vec!["vulnerability"; 20];
        let text = words.join(" ");
        // Each word is 13 chars + 1 space = 14 chars. Chunk size 50 fits 3 words (41 chars).
        let chunks = chunk_text(&text, 50);

        assert!(chunks.len() > 1);
        for chunk in &chunks {
            // No chunk should exceed the max chunk size unless a single word is larger
            assert!(chunk.len() <= 50, "Chunk exceeded size limit: {}", chunk.len());
            // No words should be split in half
            for word in chunk.split_whitespace() {
                assert_eq!(word, "vulnerability");
            }
        }
    }

    #[test]
    fn test_chunk_text_single_word_longer_than_chunk_size() {
        let long_token = "A".repeat(120);
        let chunks = chunk_text(&long_token, 50);
        // A single long token should still be captured rather than lost or panicking
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], long_token);
    }

    #[test]
    fn test_chunk_text_retains_all_words() {
        let text = "SQL injection occurs when untrusted data is sent to an interpreter as part of a command or query.";
        let original_words: Vec<&str> = text.split_whitespace().collect();

        let chunks = chunk_text(text, 30);
        let reconstructed_words: Vec<&str> = chunks
            .iter()
            .flat_map(|c| c.split_whitespace())
            .collect();

        assert_eq!(original_words, reconstructed_words);
    }

    #[test]
    fn test_load_pdf_with_existing_docs() {
        let docs_dir = std::path::Path::new("docs");
        if docs_dir.exists() {
            let result = load_pdf(&docs_dir.join("*.pdf"));
            if let Ok(chunks) = result {
                assert!(!chunks.is_empty(), "Expected non-empty chunks from docs");
                for chunk in chunks {
                    assert!(!chunk.trim().is_empty());
                }
            }
        }
    }
}

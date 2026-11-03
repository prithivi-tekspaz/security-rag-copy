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
        .flat_map(|content| {
            let mut chunks = Vec::new();
            let mut current = String::new();
            for word in content.split_whitespace() {
                if current.len() + word.len() + 1 > CHUNK_SIZE && !current.is_empty() {
                    chunks.push(std::mem::take(&mut current).trim().to_string());
                }
                current.push_str(word);
                current.push(' ');
            }
            if !current.is_empty() {
                chunks.push(current.trim().to_string());
            }
            chunks
        })
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
        .unwrap_or_else(|_| "tinyllama".to_string());

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
        .preamble("You are a security assistant. Answer questions about application security, vulnerabilities, and best practices using the provided context. Cite relevant concepts when possible.")
        .dynamic_context(2, index)
        .build();

    println!("Starting security RAG chatbot (completion model: {completion_model_name})...");
    let chatbot = ChatBotBuilder::new().agent(rag_agent).max_turns(10).build();
    chatbot.run().await?;

    Ok(())
}

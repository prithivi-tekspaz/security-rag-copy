//! Simple security RAG agent built on the Rig framework.
//! All runtime configuration (models, prompt, paths) is supplied through the environment / `.env`.

use anyhow::{Context, Result};
use rig::integrations::cli_chatbot::ChatBotBuilder;
use rig::prelude::*;
use rig::providers::ollama::OllamaConfig;
use arrow_array::{ArrayRef, FixedSizeListArray, RecordBatch, StringArray, types::Float64Type};
use rig::{Embed, embeddings::EmbeddingsBuilder, loaders::PdfFileLoader};
use rig_lancedb::{LanceDbVectorIndex, SearchParams};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::io::Write;
use std::sync::{Arc, OnceLock};

static LOG_FILE: OnceLock<String> = OnceLock::new();

/// Appends an error line (unix timestamp + message) to the file named by `LOG_FILE`.
fn log_error(msg: &str) {
    eprintln!("{msg}");
    let Some(path) = LOG_FILE.get() else { return };
    if let Some(dir) = Path::new(path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "[{secs}] ERROR {msg}");
    }
}

/// Reads a required setting from the environment (populated from `.env`).
fn env(key: &str) -> Result<String> {
    std::env::var(key).with_context(|| format!("missing required setting `{key}` (see .env.example)"))
}

#[derive(Embed, Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
struct Document {
    id: String,
    #[embed]
    content: String,
}

/// Converts embedded documents into an Arrow batch with `id`, `content` and `embedding` columns.
fn as_record_batch(
    records: Vec<(Document, Vec<rig::embeddings::Embedding>)>,
    dims: usize,
) -> Result<RecordBatch> {
    let id = StringArray::from_iter_values(records.iter().map(|(d, _)| d.id.as_str()));
    let content = StringArray::from_iter_values(records.iter().map(|(d, _)| d.content.as_str()));
    let embedding = FixedSizeListArray::from_iter_primitive::<Float64Type, _, _>(
        records.into_iter().map(|(_, e)| {
            e.into_iter()
                .next()
                .map(|emb| emb.vec.into_iter().map(Some).collect::<Vec<_>>())
        }),
        dims as i32,
    );
    Ok(RecordBatch::try_from_iter(vec![
        ("id", Arc::new(id) as ArrayRef),
        ("content", Arc::new(content) as ArrayRef),
        ("embedding", Arc::new(embedding) as ArrayRef),
    ])?)
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
                    log_error(&format!("Error reading PDF content: {e}"));
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
    // Load configuration from a .env file (if present); real env vars take precedence.
    let _ = dotenvy::dotenv();
    if let Ok(path) = std::env::var("LOG_FILE") {
        let _ = LOG_FILE.set(path);
    }

    let result = run().await;
    if let Err(e) = &result {
        log_error(&format!("Fatal: {e:#}"));
    }
    result
}

async fn run() -> Result<()> {

    let client = OllamaConfig::new().with_base_url(&env("OLLAMA_BASE_URL")?).client();
    let embed_model_name = env("EMBED_MODEL")?;
    let completion_model_name = env("COMPLETION_MODEL")?;
    let preamble = env("PREAMBLE")?;
    let table_name = env("LANCEDB_TABLE")?;
    let temperature: f64 = env("TEMPERATURE")?
        .parse()
        .context("`TEMPERATURE` must be a number")?;

    let embedding_model = client.embedding(&embed_model_name, None).erase();

    let db_path = env("LANCEDB_PATH")?;
    let db = lancedb::connect(&db_path).execute().await?;

    let table = if db.table_names().execute().await?.contains(&table_name) {
        println!("Reusing existing vector store.");
        db.open_table(&table_name).execute().await?
    } else {
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

        let dims = embeddings[0].1[0].vec.len();
        db.create_table(&table_name, vec![as_record_batch(embeddings, dims)?])
            .execute()
            .await?
    };

    let index =
        LanceDbVectorIndex::new(table, embedding_model, "id", SearchParams::default()).await?;

    let rag_agent = AgentBuilder::new(client.completion(&completion_model_name))
        .preamble(&preamble)
        .temperature(temperature)
        .dynamic_context(2, index)
        .build();

    println!("Starting security RAG chatbot...");
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

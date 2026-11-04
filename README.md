# security-rag

A simple security RAG (Retrieval-Augmented Generation) agent built with the [Rig](https://github.com/0xPlaygrounds/rig) framework and a local [Ollama](https://ollama.com/) server.

## Tech Stack

- **Language**: Rust (edition 2021)
- **LLM framework**: Rig 0.44 (agent, embeddings, vector store, PDF loader, CLI chatbot)
- **LLM provider**: Ollama (local, no API keys)
- **Embedding model**: `nomic-embed-text` (default; override via `EMBED_MODEL`)
- **Completion model**: `tinyllama` (default; override via `COMPLETION_MODEL`)
- **Vector store**: In-memory (`InMemoryVectorStore`)
- **Async runtime**: Tokio (full features)
- **Serialization**: serde + serde_derive
- **Error handling**: anyhow
- **Document parsing**: Rig `PdfFileLoader` (PDF feature)

## File Structure

```
security-rag/
├── Cargo.toml          # Crate manifest and dependencies
├── Cargo.lock          # Locked dependency versions
├── README.md           # This file
├── progress-tracker.md # Task tracker (ASD-STE100 style)
├── .gitignore          # Ignores target/ build artifacts
└── src/
    └── main.rs         # Application entry point and RAG pipeline
```

## How It Works

1. Connects to a local Ollama server at `http://localhost:11434`.
2. Loads security documents from a `docs/` folder (PDF glob) if present; otherwise uses a built-in OWASP-style corpus.
3. Chunks each document into ~2000-character segments.
4. Embeds chunks with the configured embedding model.
5. Stores embeddings in an in-memory vector store and builds a search index.
6. Creates a RAG agent with a security-focused preamble and dynamic context retrieval.
7. Runs an interactive CLI chatbot (max 10 turns per session).

## Prerequisites

- Rust toolchain (cargo)
- Ollama running locally on port 11434
- Models pulled locally:

```bash
ollama pull nomic-embed-text
ollama pull tinyllama
```

## Build

```bash
cargo build
```

## Run

```bash
cargo run
```

Or run the binary directly:

```bash
./target/debug/security_rag.exe
```

Type a question at the `>` prompt. Type `exit` to quit.

## Configuration

Override models with environment variables:

```bash
EMBED_MODEL=nomic-embed-text COMPLETION_MODEL=tinyllama cargo run
```

## Security Documents

Place PDF files in a `docs/` folder next to the binary. The agent chunks and indexes them at startup. If no docs are present, a small built-in security corpus is used.

## License

MIT

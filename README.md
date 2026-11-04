# Security RAG

A local, privacy-focused Security Retrieval-Augmented Generation (RAG) agent built with Rust, the [Rig](https://github.com/0xPlaygrounds/rig) LLM framework, and [Ollama](https://ollama.com/).

This project indexes application security documents (such as OWASP guidelines, vulnerability advisories, and architecture hardening guides) and provides an interactive terminal chatbot to answer security questions with grounded citations. Everything runs 100% locally on your machine—no external API keys, telemetry, or cloud services required.

---

## Architecture & Workflow

The following diagram illustrates the complete end-to-end pipeline from document ingestion to RAG generation:

```mermaid
flowchart TD
    subgraph Ingestion ["1. Document Ingestion and Chunking"]
        A["PDF Files in docs/*.pdf"] -->|PdfFileLoader| B["Extract Text Stream"]
        B --> C["Split into Chunks (~2000 characters)"]
        FB["docs/ absent or empty?"] -->|Fallback| FC["Built-in OWASP Security Corpus"]
        C --> D["Document Chunks"]
        FC --> D
    end

    subgraph Embedding ["2. Embedding and Vector Indexing"]
        D --> E["Ollama Embeddings (nomic-embed-text)"]
        E --> F["InMemoryVectorStore"]
        F --> G["Search Index"]
    end

    subgraph Chatbot ["3. RAG Agent and Interactive CLI"]
        H["User Question at > prompt"] --> I["Dynamic Retrieval (Top 2 chunks)"]
        G -.->|Similarity Search| I
        I --> J["AgentBuilder with Security Preamble"]
        J --> K["Ollama Completion (CyberCrew/notmythos-8b:latest)"]
        K --> L["Security Answer in CLI (up to 10 turns)"]
    end
```

### Detailed Pipeline Stages

1. **Document Loading**:
   - At startup, the application checks for a `docs/` folder in the project root.
   - If present, it loads all matching `*.pdf` files using `rig::loaders::PdfFileLoader`.
   - If `docs/` is absent or contains no readable PDFs, it falls back to an embedded 5-item OWASP vulnerability corpus so the agent is immediately usable out of the box.

2. **Word-Boundary Chunking**:
   - Extracted document text is processed through a word-boundary chunker with a 2,000-character limit (`CHUNK_SIZE = 2000`).
   - Words are grouped along whitespace breaks to prevent tokens or security terms from being sliced mid-word.

3. **Vector Embeddings**:
   - Each chunk is converted into a `Document { id, content }` and batched into an `EmbeddingsBuilder`.
   - Embeddings are generated locally via the Ollama `/api/embeddings` endpoint using `nomic-embed-text`.

4. **In-Memory Vector Store**:
   - Embeddings and document payloads are held in RAM within an `InMemoryVectorStore`.
   - A cosine similarity search index is constructed to serve context retrieval requests.

5. **Security Agent & Interactive Chatbot**:
   - The RAG agent is instantiated with a security expert system preamble (`You are a security assistant. Answer questions about application security, vulnerabilities, and best practices using the provided context...`).
   - For every question, the agent dynamically retrieves the top 2 most relevant context chunks and passes them to the completion model (`CyberCrew/notmythos-8b:latest` by default).
   - An interactive CLI session allows up to 10 turns of dialogue per execution.

---

## Tech Stack & Dependencies

| Component | Technology | Description |
| :--- | :--- | :--- |
| **Language** | Rust (2021 Edition) | High-performance, memory-safe systems language (`forbid(unsafe_code)`) |
| **LLM Framework** | [Rig](https://github.com/0xPlaygrounds/rig) (v0.44) | Modular LLM framework for agents, embeddings, loaders, and stores |
| **LLM Host** | [Ollama](https://ollama.com/) | Local LLM runner exposing REST endpoints on port 11434 |
| **Embedding Model** | `nomic-embed-text` | 768-dimensional text embeddings for semantic similarity search |
| **Completion Model**| `CyberCrew/notmythos-8b:latest` | Lightweight completion model suitable for fast local inference |
| **Vector Store** | `InMemoryVectorStore` | Ephemeral in-memory vector index |
| **Async Runtime** | Tokio (v1, full) | Multi-threaded asynchronous runtime |
| **Serialization** | Serde / Serde JSON | Data serialization and deserialization |
| **Error Handling** | Anyhow | Context-rich error handling |

---

## File Structure

```
security-rag/
├── docs/                   # Optional: folder for your custom security PDFs
│   └── *.pdf
├── src/
│   └── main.rs             # Application entry point, chunking logic, and RAG pipeline
├── Cargo.toml              # Crate manifest, dependencies (rig with pdf feature)
├── Cargo.lock              # Reproducible dependency lockfile
├── progress-tracker.md     # Task progress tracker (ASD-STE100 format)
├── .gitignore              # Ignores target/ build output and local artifacts
└── README.md               # Complete project documentation and workflow guide
```

---

## Prerequisites

1. **Rust Toolchain**: Install [Rust & Cargo](https://rustup.rs/) (1.75+ recommended).
2. **Ollama**: Download and install [Ollama](https://ollama.com/).
3. **Pull Models**: Start Ollama and pull the required default models:

```bash
ollama pull nomic-embed-text
ollama pull CyberCrew/notmythos-8b:latest
```

*(Optional: You can also use other models such as `bge-m3`, `qwen2.5`, or `llama3.2` via environment variables).*

---

## Ingesting Documents (How to Add PDFs)

The system automatically handles chunking and embedding upon startup:

1. **Create the `docs/` folder** in the project root:
   - **PowerShell**:
     ```powershell
     New-Item -ItemType Directory -Name docs
     ```
   - **Bash / Linux**:
     ```bash
     mkdir docs
     ```

2. **Add your PDF files**:
   Place any PDF documents (e.g. security policy, vulnerability guides, audit reports) into `docs/`.

3. **Run the program**:
   ```bash
   cargo run
   ```

At startup, the application outputs:
```text
Loaded 18 security chunks.
Successfully generated embeddings
Starting security RAG chatbot (completion model: CyberCrew/notmythos-8b:latest)...
>
```

> [!NOTE]
> Chunks and embeddings are stored in memory (`InMemoryVectorStore`). If you add or remove PDF files from `docs/`, restart the application with `cargo run` to re-chunk and re-embed.

---

## Build & Run

### Build

```bash
cargo build
```

For an optimized release binary:
```bash
cargo build --release
```

### Run (Interactive CLI)

```bash
cargo run
```

Or run the compiled binary directly:
- **Windows**:
  ```powershell
  .\target\debug\security_rag.exe
  ```
- **Linux / macOS**:
  ```bash
  ./target/debug/security_rag
  ```

### Example Session

```text
Loaded 11 security chunks.
Successfully generated embeddings
Starting security RAG chatbot (completion model: CyberCrew/notmythos-8b:latest)...
> What is SQL injection and how can we prevent it?
SQL injection occurs when untrusted user input is sent directly to an interpreter as part of a database command. To prevent it, use parameterized queries, prepared statements, and ORMs rather than string concatenation.

> Who are you?
I'm Security RAG, a local cybersecurity research assistant. I'm here to provide practical advice and guidance on application security, vulnerabilities, and best practices.

> exit
```

---

## Configuration

You can override the embedding or completion models at runtime using environment variables:

### Linux / macOS (Bash)
```bash
EMBED_MODEL=bge-m3 COMPLETION_MODEL=qwen2.5:7b cargo run
```

### Windows (PowerShell)
```powershell
$env:EMBED_MODEL="nomic-embed-text"
$env:COMPLETION_MODEL="CyberCrew/notmythos-8b:latest"
cargo run
```

### Windows (Command Prompt)
```cmd
set EMBED_MODEL=nomic-embed-text
set COMPLETION_MODEL=CyberCrew/notmythos-8b:latest
cargo run
```

| Variable | Default Value | Description |
| :--- | :--- | :--- |
| `EMBED_MODEL` | `nomic-embed-text` | Ollama model tag used for generating chunk embeddings |
| `COMPLETION_MODEL` | `CyberCrew/notmythos-8b:latest` | Ollama model tag used by the RAG agent for answer synthesis |

---

## Chunking Customization

The chunk size can be configured directly in [`src/main.rs`](src/main.rs):

```rust
fn load_pdf(path: &Path) -> Result<Vec<String>> {
    const CHUNK_SIZE: usize = 2000; // Adjust chunk size in characters
    ...
}
```

- **Smaller chunks (e.g. 500–1,000 chars)**: Better for pinpoint semantic retrieval and precise definitions.
- **Larger chunks (e.g. 2,000–3,000 chars)**: Better for preserving broader paragraph context and multi-step security explanations.

---

## Troubleshooting

- **`Connection refused (os error 10061 / 111)`**:
  Ollama is not running. Start Ollama in the background or run `ollama serve`.
- **`model 'nomic-embed-text' not found` or `model 'CyberCrew/notmythos-8b:latest' not found`**:
  Make sure you pulled the models via `ollama pull nomic-embed-text` and `ollama pull CyberCrew/notmythos-8b:latest`.
- **`No content found in PDF file`**:
  Ensure the PDF contains selectable text (not scanned images/bitmaps requiring OCR).

---

## License

MIT

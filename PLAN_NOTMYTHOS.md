# Plan: Use NotMythos (lightweight) as the LLM for security-rag

Status: PROPOSED (no code changes yet). Awaiting user approval.

## 1. Model facts (from https://ollama.com/CyberCrew/notmythos-8b/tags)

Available tags:
- `CyberCrew/notmythos-8b:latest` — 2.0 GB, 128K context, Text only, 5 months old
- `CyberCrew/notmythos-8b:mini` — 398 MB, 32K context, Text only
- `CyberCrew/notmythos-8b:3.8-27b-uncensored` — 12 GB, 256K context, Text+Image

Selected (per user): `CyberCrew/notmythos-8b:latest` (2 GB, 128K context, text-only).

## 2. Hardware feasibility check

- RAM: 16 GB total, ~6 GB free. 2 GB model fits comfortably (no OOM risk).
- Disk: 43 GB free on D: -> enough for 2 GB pull.
- Conclusion: feasible. No need to free RAM or reduce context.

## 3. Current code state

- src/main.rs uses:
  - embed model: nomic-embed-text (default, overridable via EMBED_MODEL)
  - completion model: tinyllama (default, overridable via COMPLETION_MODEL)
- Agent preamble is generic security assistant text.
- dynamic_context(2, index).

## 4. Proposed changes (features/functions, one by one)

Step A - Pull and verify the model
- Run: ollama pull CyberCrew/notmythos-8b:latest
- Verify: ollama list shows the model; ollama run ... identity test returns "I am Security RAG...".
- No code change.

Step B - Switch default completion model to NotMythos latest
- Change default COMPLETION_MODEL from "tinyllama" to "CyberCrew/notmythos-8b:latest".
- Keep EMBED_MODEL default as nomic-embed-text (unchanged).
- Update README default model name and prerequisites (pull latest, not 27b).

Step C - Apply recommended generation parameters
- Rig 0.44 Ollama options may not expose all sampling params; verify API first.
- If supported: temperature 0.35, top_p 0.9, top_k 40, repeat_penalty 1.15, num_ctx 8192 (or up to 128K), num_predict 2048.
- If unsupported: keep defaults and document limitation.
- Verify build.

Step D - Update agent preamble to Security RAG identity
- Replace generic preamble with Security RAG identity:  "I am Security RAG, a local cybersecurity research assistant."
- Include authorized-environment assumption, direct technical answers, no unnecessary refusals, defensive value, impact/verification/remediation guidance.
- Keep safety boundary (no illegal access, credential theft, malware deployment, ransomware, data theft, attacks on real third parties).

Step E - Add identity test
- Add manual test: prompt "Who are you?" and check response contains "Security RAG".
- Document expected output in README.

Step F - Add security-focused RAG corpus (docs/)
- Create docs/ with sample security PDFs (OWASP, secure coding, incident response).
- Verify chunking + retrieval still works.

Step G - Add unit tests for chunking
- Extract load_pdf chunking into a pure function and add #[cfg(test)] tests.
- Run cargo test.

Step H - Run end-to-end and update progress-tracker.md
- Run cargo build, cargo run; ask security questions; verify quality.
- Mark steps done in progress-tracker.md.

Step I - Commit and push
- git add, commit, push to origin (skilled-coderAI/Security-RAG).

## 5. Risks / open questions

- 2 GB model is heavily quantized; answer quality may be lower than 27B. Evaluate during Step H.
- Rig 0.44 Ollama options API may not expose all sampling params; verify before coding.
- "Uncensored" framing must keep safety boundary; do not remove refusals for illegal acts.
- Model is 5 months old; may lag newer security knowledge. RAG corpus mitigates.

## 6. Approval gate

No code changes will be made until you approve this plan.
Reply with "approve" (or list changes) to proceed.

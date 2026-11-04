# Plan: Use NotMythos as the LLM for security-rag

Status: PROPOSED (no code changes yet). Awaiting user approval.

## 1. Model facts (from https://ollama.com/CyberCrew/notmythos-8b)

- Model name: `CyberCrew/notmythos-8b:3.8-27b-uncensored`
- Base: Qwen3.8 27B (uncensored GGUF, IQ2_M quantization)
- Size on disk: ~11 GB
- Context window (num_ctx): 8192
- Max new tokens (num_predict): 2048
- Recommended params: temperature 0.35, top_p 0.9, top_k 40, repeat_penalty 1.15
- Runtime: Ollama (local)
- Purpose: cybersecurity research assistant (low-refusal for authorized work)
- Identity: "NotMythos" (must not call itself Mythos)

## 2. Hardware feasibility check

- RAM: 16 GB total, ~4.7 GB free now. Model is 11 GB at IQ2_M.
- Risk: will likely need to free memory (close apps) or run with CPU offload.
- Disk: 43 GB free on D: -> enough for 11 GB pull.
- Conclusion: feasible only if RAM is freed before run; otherwise OOM like prior qwen3.5 attempts.
- Action before pull: close heavy apps; optionally set OLLAMA_NUM_PARALLEL=1.

## 3. Current code state

- src/main.rs uses:
  - embed model: nomic-embed-text (default, overridable via EMBED_MODEL)
  - completion model: tinyllama (default, overridable via COMPLETION_MODEL)
- Agent preamble is generic security assistant text.
- dynamic_context(2, index).

## 4. Proposed changes (features/functions, one by one)

Step A - Pull and verify the model
- Run: ollama pull CyberCrew/notmythos-8b:3.8-27b-uncensored
- Verify: ollama list shows the model; ollama run ... identity test returns "I am NotMythos...".
- No code change.

Step B - Switch default completion model to NotMythos
- Change default COMPLETION_MODEL from "tinyllama" to "CyberCrew/notmythos-8b:3.8-27b-uncensored".
- Keep EMBED_MODEL default as nomic-embed-text (unchanged).
- Update README default model name and prerequisites.

Step C - Apply recommended generation parameters
- Rig Ollama client currently uses defaults. Need to set temperature 0.35, top_p 0.9, top_k 40, repeat_penalty 1.15, num_ctx 8192, num_predict 2048.
- Check Rig 0.44 Ollama API for how to pass options (CompletionRequest options / OllamaConfig). If unsupported directly, document limitation and keep defaults.
- Verify build.

Step D - Update agent preamble to NotMythos identity
- Replace generic preamble with NotMythos system prompt excerpt: identity, authorized-environment assumption, direct technical answers, no unnecessary refusals, defensive value, impact/verification/remediation guidance.
- Keep safety boundary (no illegal access, credential theft, malware deployment, ransomware, data theft, attacks on real third parties).

Step E - Add identity test
- Add a small integration/manual test: prompt "Who are you?" and check response contains "NotMythos".
- Document expected output in README or a tests/ note.

Step F - Add security-focused RAG corpus (docs/)
- Create docs/ with sample security PDFs (OWASP, secure coding, incident response).
- Verify chunking + retrieval still works with 8192 context.

Step G - Add unit tests for chunking
- Extract load_pdf chunking into a pure function and add #[cfg(test)] tests.
- Run cargo test.

Step H - Run end-to-end and update progress-tracker.md
- Run cargo build, cargo run; ask security questions; verify quality.
- Mark steps done in progress-tracker.md.

Step I - Commit and push
- git add, commit, push to origin (skilled-coderAI/Security-RAG).

## 5. Risks / open questions

- Memory: 11 GB model on 16 GB machine may OOM; may need to free RAM or reduce num_ctx.
- Rig 0.44 Ollama options API may not expose all sampling params; verify before coding.
- "Uncensored" framing must keep safety boundary; do not remove refusals for illegal acts.
- Model pull time: ~11 GB download.

## 6. Approval gate

No code changes will be made until you approve this plan.
Reply with "approve" (or list changes) to proceed.

# Plan: Deploy security-rag to AWS Lambda (and test it)

Status: PROPOSED (no code changes yet). Awaiting approval.

## 1. Reality check (constraints)

- The current binary is a CLI chatbot (stdin/stdout, interactive). AWS Lambda is request/response, stateless, and has no stdin. It cannot run as-is.
- Lambda cold start + 15-min max execution. Loading an 11 GB model is impossible; the 2 GB NotMythos model is borderline but too large for Lambda storage (50 MB code + 10 GB /tmp ephemeral). Ollama itself cannot run inside Lambda.
- Therefore: Lambda must NOT host the LLM. It must call an external LLM endpoint (Bedrock, or a remote Ollama/OpenAI-compatible server) and only do retrieval + prompt assembly in Lambda, OR host the whole app on a container (ECS/Fargate) instead.
- Embeddings + vector store must also move out of Lambda or be precomputed (Lambda has no persistent RAM across invocations; cold starts re-embed every time -> too slow/costly).

## 2. Recommended architecture for Lambda

Option A (serverless, recommended for testing):
- Precompute embeddings once (local or one-off job) -> store vectors in S3 / DynamoDB / OpenSearch.
- Lambda function (Rust, runtime provided.al2023) handles POST /ask:
  1. Parse question from event.
  2. Embed question via Bedrock (Titan embeddings) or a remote embedding API.
  3. Retrieve top-k chunks from stored vectors (S3/DynamoDB/OpenSearch).
  4. Call Bedrock (Claude/Titan) or remote LLM with preamble + context.
  5. Return JSON { answer, sources }.
- No Ollama, no interactive CLI.

Option B (container, closest to current app):
- Package the whole Rust binary + Ollama into a Lambda container image (up to 10 GB). Still needs a running Ollama; Lambda does not keep processes alive between invocations -> not suitable. Use ECS Fargate or EC2 instead.

## 3. Code changes needed (one by one, after approval)

Step 1 - Add HTTP handler crate
- Add dependencies: lambda_runtime, serde, serde_json, aws-sdk-bedrockruntime (or reqwest for remote Ollama), tokio (already present).
- Replace CLI chatbot with a Lambda handler: async fn handler(event: LambdaEvent<ApiGatewayProxyRequest>) -> Result<ApiGatewayProxyResponse>.

Step 2 - Extract pure functions
- Make chunk_text, load_pdf, embed, retrieve, build_prompt reusable (already chunk_text is pub).
- Move vector store to a pluggable trait (InMemory for local, S3/DynamoDB for Lambda).

Step 3 - Replace Ollama calls
- Embeddings: use Bedrock Titan Embeddings (aws-sdk-bedrockruntime) or a remote OpenAI-compatible /embeddings endpoint.
- Completion: use Bedrock (Claude 3 Haiku/Sonnet) or remote endpoint.

Step 4 - Precompute & store corpus
- One-off script (or a separate binary) chunks docs/ and writes vectors to S3 (JSON) or DynamoDB.
- Lambda reads vectors at cold start (or via OpenSearch k-NN).

Step 5 - Build for Lambda
- rustup target add x86_64-unknown-linux-gnu (or aarch64 for Graviton).
- cargo build --release --target x86_64-unknown-linux-gnu
- Package with bootstrap + runtime (cargo-lambda or manual zip).

Step 6 - Deploy
- Create IAM role with bedrock:InvokeModel + s3:GetObject.
- Create Lambda function (runtime provided.al2023, handler bootstrap).
- Attach API Gateway (HTTP API) with POST /ask.
- Set env vars: EMBED_MODEL, COMPLETION_MODEL (Bedrock model id), CORPUS_BUCKET, etc.

Step 7 - Test
- Local: cargo lambda watch + curl http://localhost:9000/2015-03-31/functions/function/invocations
- Remote: curl https://<api-id>.execute-api.<region>.amazonaws.com/ask -d '{"question":"What is SQL injection?"}'
- Validate: answer contains Security RAG identity and grounded citations; latency < 10s; cold start < 30s.

Step 8 - Observability
- CloudWatch logs, X-Ray tracing, alarms on errors/throttles.

## 4. Access & test (what "access" means for Lambda)

- There is no interactive CLI in Lambda. Access is via HTTP (API Gateway) or direct Invoke.
- Test commands:
  - Local:  cargo lambda invoke --data-file event.json security-rag
  - Remote: aws lambda invoke --function-name security-rag --payload '{"question":"..."}' out.json
  - HTTP:   curl -X POST https://.../ask -H 'Content-Type: application/json' -d '{"question":"What is XSS?"}'

## 5. Risks / open questions

- Bedrock model availability and cost in your region.
- Vector storage choice (S3 JSON vs DynamoDB vs OpenSearch) affects retrieval latency and cost.
- Cold start with large embedding matrix; consider provisioned concurrency or smaller corpus.
- Ollama is not available in Lambda; must switch to Bedrock/remote LLM.

## 6. Approval gate

No code changes until you approve. Reply "approve" (or request edits).

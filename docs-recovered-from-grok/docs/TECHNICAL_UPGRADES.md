# Technical Upgrade Roadmap (avrag-rs)

This document tracks approved technical upgrades and library migrations inspired by `awesome-rust` and modern engineering standards.

## 🛠 Approved Import List

### 1. API Layer & Type Safety
- **`utoipa`**: Generate OpenAPI (Swagger) documentation and shared type definitions.
  - *Benefit*: Reduces friction between `avrag-api` and `frontend_next`.
- **`axum-extra`**: Use typed headers and better extraction utilities.
  - *Benefit*: Enhances API type safety.

### 2. Engineering Quality & Testing
- **`insta`**: Implement snapshot testing for `DocumentIr` and RAG outputs.
  - *Benefit*: Ensures stability in complex parsing and synthesis logic.
- **`proptest`**: Property-based testing for `guardrails` and complex business logic.
  - *Benefit*: Eliminates edge cases in safety-critical code.

### 3. Performance & Data Processing
- **`rayon`**: Introduce parallel processing for RRF (Reciprocal Rank Fusion) and data merging.
  - *Benefit*: Reduces P99 latency in high-volume retrieval tasks.
- **`apache-arrow`**: (Optional/Evaluation) Efficient data exchange between the JVM parser and Rust ingestion.
  - *Benefit*: Low-overhead serialization for massive document blocks.

### 4. Code Analysis & RAG Context
- **`syn`**: Replace/augment `tree-sitter-rust` for deeper semantic analysis of Rust code.
  - *Benefit*: Enables high-fidelity code-base RAG (e.g., call-graph navigation).

### 5. Maintenance & DB Access
- **`sea-query`**: Dynamic SQL query building to complement `sqlx`.
  - *Benefit*: Handles complex, multi-parameter filtering without manual string concatenation.

---
*Note: `unstructured-client` was evaluated and excluded in favor of existing service-based architecture.*

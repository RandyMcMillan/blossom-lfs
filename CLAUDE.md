# CLAUDE.md

## Project Overview

blossom-lfs is a Git LFS custom transfer agent that bridges Git LFS with Blossom blob storage. It reads JSON-line requests from Git LFS on stdin, uploads/downloads blobs via a Blossom server, and writes responses to stdout.

## Build & Test

```bash
cargo build                      # default (HTTP only)
cargo build --features iroh      # with iroh QUIC transport
cargo test                       # all tests (49 pass, 4 live tests ignored)
cargo test --test live_server_tests -- --ignored  # requires BLOSSOM_TEST_SERVER + BLOSSOM_TEST_NSEC env vars
cargo fmt --check                # CI checks this
cargo clippy                     # CI checks this
cargo doc --no-deps              # generate rustdocs
```

## Architecture

- **`src/agent.rs`** — Git LFS protocol handler. Spawns async tasks per upload/download. Uses `Arc<Transport>` for blob operations.
- **`src/transport.rs`** — Enum dispatch over `BlossomClient` (HTTP) and `IrohBlossomClient` (QUIC). Calls blossom-rs `BlobClient` trait methods.
- **`src/chunking/`** — File splitting (`Chunker`), Merkle tree (`MerkleTree`), manifest serialization (`Manifest`). This is ours, not in blossom-rs.
- **`src/config.rs`** — Loads from `.lfsdalconfig` → `.git/config` → env vars. Supports `transport = http | iroh`.
- **`src/protocol.rs`** — Git LFS JSON wire format types (Request, ProgressResponse, TransferResponse).
- **`src/error.rs`** — `BlossomLfsError` enum, `Result<T>` alias.
- **`src/main.rs`** — CLI entry point (clap), tracing setup, stdin/stdout loop.

## Key Design Decisions

- **blossom-rs as the Blossom layer**: All HTTP client, Nostr auth, and protocol types come from `blossom-rs` (v0.3+). We don't implement Blossom protocol ourselves.
- **Dedup via `exists()`**: Before every upload (chunks and final blob), we HEAD-check the server and skip if the blob is already there.
- **Transport enum, not trait objects**: `Transport` is a simple enum dispatching to concrete client types. Mirrors blossom-rs's `BlobClient` trait pattern (`Address = ()` for HTTP, `Address = EndpointAddr` for iroh).
- **Agent::new is async**: Required because iroh endpoint binding is async.
- **Structured tracing**: All spans/events use OTEL-style semantic fields matching blossom-rs conventions (`blob.oid`, `blob.size`, `chunk.sha256`, `chunks.skipped`).

## Feature Flags

- `default` — HTTP transport only
- `iroh` — Adds iroh QUIC transport, enables `blossom-rs/iroh-transport` + `blossom-rs/pkarr-discovery` + direct `iroh` dep

## Test Structure

- `tests/auth_tests.rs` — Smoke tests for blossom-rs auth (Signer, build_blossom_auth)
- `tests/chunker_tests.rs` — File chunking, hash verification
- `tests/merkle_tests.rs` — Merkle tree construction, proofs, verification
- `tests/manifest_tests.rs` — Manifest creation, serialization, chunk info
- `tests/integration_tests.rs` — BlossomClient with wiremock, chunker + manifest integration
- `tests/e2e_tests.rs` — Full workflow with mock axum Blossom server
- `tests/live_server_tests.rs` — Gated (`#[ignore]`) tests against a real server, configured via `BLOSSOM_TEST_SERVER` and `BLOSSOM_TEST_NSEC` env vars

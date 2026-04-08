# CLAUDE.md

## Project Overview

blossom-lfs is a Git LFS daemon (v0.4.0) that bridges vanilla `git lfs` with Blossom blob storage. A local HTTP server on `localhost:31921` handles all Git LFS operations: batch, upload, download, verify, and locks. No custom transfer agent configuration needed.

## Build & Test

```bash
cargo build                      # default (HTTP only)
cargo build --features iroh      # with iroh QUIC transport
cargo test                       # 92 tests (5 ignored)
cargo test --test live_server_tests -- --ignored  # requires BLOSSOM_TEST_SERVER + BLOSSOM_TEST_NSEC env vars
cargo test --test git_lfs_e2e_tests -- --ignored  # requires git-lfs binary installed
cargo fmt --check                # CI checks this
cargo clippy -- -D warnings      # CI checks this
cargo doc --no-deps              # generate rustdocs
```

## Architecture

- **`src/daemon.rs`** — Local axum HTTP server. Handles all Git LFS wire protocol: batch, streaming download/upload, verify, and lock API. Base64url-decodes repo filesystem path from URL, reads per-repo Blossom config, forwards lock requests to Blossom BUD-19 endpoints.
- **`src/lock_client.rs`** — HTTP client for Blossom BUD-19 lock endpoints with Nostr auth signing.
- **`src/transport.rs`** — Enum dispatch over `BlossomClient` (HTTP) and `IrohBlossomClient` (QUIC). Created per-request from per-repo config.
- **`src/chunking/`** — File splitting (`Chunker`), Merkle tree (`MerkleTree`), manifest serialization (`Manifest`).
- **`src/config.rs`** — Loads from `.lfsdalconfig` → `.git/config` → env vars. Supports `transport = http | iroh`, `daemon-port`.
- **`src/error.rs`** — `BlossomLfsError` enum, `Result<T>` alias.
- **`src/main.rs`** — CLI entry point (clap) with `daemon` and `setup` subcommands.

## Daemon Routes

```
POST /lfs/<b64>/objects/batch           Git LFS batch API (basic transfer)
GET  /lfs/<b64>/objects/<oid>           Streaming download (chunked reassembly)
PUT  /lfs/<b64>/objects/<oid>           Streaming upload (chunking pipeline)
POST /lfs/<b64>/objects/<oid>/verify    Post-upload verify (HEAD check)
POST /lfs/<b64>/locks                   Create lock  (→ Blossom BUD-19)
GET  /lfs/<b64>/locks                   List locks   (→ Blossom BUD-19)
POST /lfs/<b64>/locks/verify            Verify locks (→ Blossom BUD-19)
POST /lfs/<b64>/locks/<id>/unlock       Unlock       (→ Blossom BUD-19)
```

## Key Design Decisions

- **Pure HTTP, no custom agent**: Vanilla `git lfs` talks to `localhost:31921`. No `lfs.standalonetransferagent` or custom transfer config needed.
- **Stateless daemon**: Each request loads per-repo config via `Config::from_repo_path()`. No cached state.
- **Streaming**: Downloads use `Body::from_stream()` for chunked reassembly. Uploads stream to tempfile, then chunk and upload.
- **blossom-rs as the Blossom layer**: All HTTP client, Nostr auth, and protocol types come from `blossom-rs` (v0.4.0+).
- **BUD-20 compression**: Daemon sends `["t","lfs"]` + `["path",...]` + `["repo",...]` tags in upload auth events. Server applies zstd/xdelta3 transparently.
- **Dedup via `exists()`**: Before every upload, HEAD-check the server and skip if the blob is already there.
- **Transport enum, not trait objects**: `Transport` is a simple enum dispatching to concrete client types.
- **Structured tracing**: OTEL-style semantic fields (`blob.oid`, `blob.size`, `chunk.sha256`).

## Feature Flags

- `default` — HTTP transport only
- `iroh` — Adds iroh QUIC transport, enables `blossom-rs/iroh-transport` + `blossom-rs/pkarr-discovery` + direct `iroh` dep

## Test Structure

- `tests/daemon_tests.rs` — 10 integration tests for batch/upload/download/verify with mock Blossom server
- `tests/lock_tests.rs` — 14 tests for lock client + daemon lock proxy (mock server)
- `tests/lock_integration_tests.rs` — 7 full-stack lock tests (real blossom-rs server): conflict, non-owner unlock, admin force, verify ours/theirs, lifecycle, 404
- `tests/bud20_integration_tests.rs` — 5 full-stack BUD-20 tests: compressed round-trip, full LFS workflow, dedup, multi-blob, chunked upload/download
- `tests/concurrent_tests.rs` — 3 concurrent operation tests: parallel same-blob uploads, different-blob uploads, lock contention
- `tests/cross_repo_lock_tests.rs` — 2 tests for lock isolation between repos
- `tests/git_lfs_e2e_tests.rs` — 1 test using real `git lfs` binary (#[ignore])
- `tests/auth_tests.rs` — Smoke tests for blossom-rs auth (Signer, build_blossom_auth)
- `tests/chunker_tests.rs` — File chunking, hash verification
- `tests/merkle_tests.rs` — Merkle tree construction, proofs, verification
- `tests/manifest_tests.rs` — Manifest creation, serialization, chunk info
- `tests/integration_tests.rs` — BlossomClient with wiremock, chunker + manifest integration
- `tests/e2e_tests.rs` — Full workflow with mock axum Blossom server
- `tests/chunked_streaming_tests.rs` — Property-based tests for chunked upload/download
- `tests/live_server_tests.rs` — 4 gated (`#[ignore]`) tests against a real server

## Dependencies

- `blossom-rs` — v0.4.0 from crates.io

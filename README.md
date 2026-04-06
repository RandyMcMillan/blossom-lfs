# BlossomLFS

Git LFS custom transfer agent for [Blossom](https://github.com/hzrd149/blossom) blob storage with chunking support for large files (2GB+).

Built on [blossom-rs](https://crates.io/crates/blossom-rs) for HTTP client, Nostr authentication, and optional iroh QUIC transport.

## Features

- **Large file support** (2GB+) via automatic chunking (16 MB default, configurable)
- **Merkle tree integrity verification** for all chunks
- **Deduplication** — skips uploading blobs that already exist on the server
- **Nostr authentication** (BIP-340 Schnorr, kind-24242 events) via blossom-rs
- **Pluggable transport** — HTTP (default) or iroh QUIC peer-to-peer
- **Structured tracing** — OTEL-style semantic fields, optional JSON output
- **Parallel chunk uploads/downloads** for performance

## Installation

```bash
# From crates.io
cargo install blossom-lfs

# Or build from source
git clone https://github.com/MonumentalSystems/BlossomLFS.git
cd blossom-lfs
cargo build --release

# With iroh QUIC transport support
cargo build --release --features iroh
```

### Verify Installation

```bash
blossom-lfs --help
blossom-lfs --config-info
```

## Configuration

### Git LFS Setup

```bash
git lfs install --local
git config lfs.standalonetransferagent blossom-lfs
git config lfs.customtransfer.blossom-lfs.path /path/to/blossom-lfs
git config -f .lfsconfig lfs.url blossom-lfs
```

### Blossom Configuration

Create `.lfsdalconfig` in your repository root:

```ini
[lfs-dal]
    server = https://your-blossom-server.com
    private-key = nsec1...       # Nostr private key (nsec or hex)
    chunk-size = 16777216        # 16 MB (optional, default)
    max-concurrent-uploads = 8   # (optional, default)
    max-concurrent-downloads = 8 # (optional, default)
    transport = http             # http (default) or iroh
```

Or use environment variables:

```bash
export BLOSSOM_SERVER_URL="https://your-blossom-server.com"
export NOSTR_PRIVATE_KEY="nsec1..."
export BLOSSOM_TRANSPORT="http"  # or "iroh"
```

**Security**: Never commit your private key. Use environment variables or `.git/config` (not tracked).

### iroh QUIC Transport

For peer-to-peer transfers over iroh QUIC (requires `--features iroh`):

```ini
[lfs-dal]
    server = <iroh-endpoint-id>  # base32-encoded iroh endpoint ID
    transport = iroh
    private-key = nsec1...
```

## How It Works

### Upload Flow

```
Large File (2GB+)
    ↓
Split into 16 MB chunks
    ↓
For each chunk:
  ├─ Check if server already has it (HEAD) → skip if exists
  └─ Upload chunk (PUT)
    ↓
Build Merkle tree from chunk hashes
    ↓
Upload JSON manifest
    ↓
Upload complete file by OID (skip if exists)
```

### Download Flow

```
Git LFS OID
    ↓
Download blob from Blossom
    ↓
Try parse as manifest
  ├─ Manifest: verify Merkle root → download chunks → reassemble
  └─ Raw blob: write directly
```

### Manifest Format

```json
{
  "version": "1.0",
  "file_size": 2147483648,
  "chunk_size": 16777216,
  "chunks": 128,
  "merkle_root": "abc123...",
  "chunk_hashes": ["hash1", "hash2", "..."],
  "original_filename": "large_file.bin",
  "content_type": "application/octet-stream",
  "created_at": 1234567890,
  "blossom_server": "https://cdn.example.com"
}
```

## Architecture

```
┌──────────┐  stdin/stdout  ┌───────┐  HTTP/QUIC  ┌────────────────┐
│  Git LFS │ ◄────────────► │ Agent │ ◄──────────► │ Blossom Server │
└──────────┘   JSON lines   └───────┘              └────────────────┘
                                │
                         ┌──────┴──────┐
                         │  Chunking   │
                         │  + Merkle   │
                         └─────────────┘
```

| Module | Purpose |
|---|---|
| `agent` | Git LFS custom transfer protocol handler |
| `transport` | Pluggable transport (HTTP via `BlossomClient`, QUIC via `IrohBlossomClient`) |
| `chunking` | File splitting, Merkle tree, manifest serialization |
| `config` | Configuration from `.lfsdalconfig` / `.git/config` / env vars |
| `protocol` | Git LFS JSON wire format types |
| `error` | Typed error definitions |

## Logging

Structured tracing with OTEL-style semantic fields (`blob.oid`, `blob.size`, `chunk.sha256`, `chunks.skipped`, etc.):

```bash
# Default: human-readable to stderr
blossom-lfs --log-level debug

# JSON output for observability pipelines
blossom-lfs --log-json --log-level info

# Log to file
blossom-lfs --log-output /tmp/blossom-lfs.log
```

## Development

### Run Tests

```bash
# Unit + integration + e2e tests
cargo test

# Live server tests (requires credentials)
BLOSSOM_TEST_SERVER=https://blossom.example.com \
BLOSSOM_TEST_NSEC=nsec1... \
  cargo test --test live_server_tests -- --ignored
```

### Build Documentation

```bash
cargo doc --open
```

## Dependencies

- [blossom-rs](https://crates.io/crates/blossom-rs) — Blossom HTTP client, Nostr auth, BlobClient trait, optional iroh transport
- `tokio` — async runtime
- `sha2` / `hex` — SHA-256 hashing
- `tracing` / `tracing-subscriber` — structured logging
- `clap` — CLI argument parsing
- `nostr` — nsec key parsing

## License

MIT

## Credits

Based on [lfs-dal](https://github.com/regen100/lfs-dal) by regen100.

## References

- [Git LFS Custom Transfers](https://github.com/git-lfs/git-lfs/blob/main/docs/custom-transfers.md)
- [Blossom Protocol](https://github.com/hzrd149/blossom)
- [blossom-rs crate](https://crates.io/crates/blossom-rs)
- [BUD-01: Server Requirements](https://github.com/hzrd149/blossom/blob/master/buds/01.md)
- [BUD-02: Blob Upload](https://github.com/hzrd149/blossom/blob/master/buds/02.md)

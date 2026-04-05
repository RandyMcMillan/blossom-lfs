# BlossomLFS Quick Start Guide

## Prerequisites

- **Git LFS** (2.13.0 or later)
- **Rust** (1.70 or later) - only needed if building from source
- **Blossom server** - A running Blossom server to store your blobs

## Installation

### Option 1: Install from Source

```bash
# Clone the repository
git clone https://github.com/your-org/blossom-lfs.git
cd blossom-lfs

# Build release binary
cargo build --release

# Binary will be at: target/release/blossom-lfs
```

### Option 2: Install with Cargo

```bash
cargo install --path .
```

## Quick Setup

Run the automated setup script:

```bash
./setup.sh
```

This script will:
1. Check if Git LFS is installed
2. Initialize Git LFS in your repository
3. Build the release binary
4. Configure Git LFS to use blossom-lfs

## Manual Configuration

If the setup script doesn't work, follow these manual steps:

### 1. Initialize Git LFS

```bash
# Check if Git LFS is installed
git lfs version

# If not installed:
# macOS:      brew install git-lfs
# Ubuntu:     sudo apt-get install git-lfs
# RHEL/CentOS: sudo yum install git-lfs
# Windows:    choco install git-lfs

# Initialize Git LFS in your repository
git lfs install
```

### 2. Configure BlossomLFS

```bash
# Set the transfer agent
git config lfs.standalonetransferagent blossom-lfs

# Set the path to the binary (use absolute path)
git config lfs.customtransfer.blossom-lfs.path "$(pwd)/target/release/blossom-lfs"

# Prevent pushing to default LFS server
git config -f .lfsconfig lfs.url blossom-lfs
```

### 3. Configure Blossom Server

```bash
# Required: Set your Blossom server URL
git config lfs-dal.server https://your-blossom-server.com

# Required: Set your Nostr private key
# Option A: In config (not recommended for shared repos)
git config lfs-dal.private-key 'nsec1...'

# Option B: Environment variable (recommended)
export NOSTR_PRIVATE_KEY='nsec1...'
# Add to your shell profile: ~/.bashrc, ~/.zshrc, etc.
```

### 4. Configure LFS Tracking

Create or edit `.gitattributes`:

```bash
# Track large files
*.bin filter=lfs diff=lfs merge=lfs -text
*.tar.gz filter=lfs diff=lfs merge=lfs -text
*.zip filter=lfs diff=lfs merge=lfs -text

# Or track all files above a size
*.* filter=lfs diff=lfs merge=lfs -text
```

## Configuration Options

All configuration options can be set in `.lfsdalconfig` or `.git/config`:

```ini
[lfs-dal]
    # Blossom server URL (required)
    server = https://your-blossom-server.com
    
    # Nostr private key in nsec or hex format (required)
    private-key = nsec1...
    
    # Chunk size in bytes (default: 16MB)
    chunk-size = 16777216
    
    # Max concurrent uploads (default: 8)
    max-concurrent-uploads = 8
    
    # Max concurrent downloads (default: 8)
    max-concurrent-downloads = 8
    
    # Auth token expiration in seconds (default: 3600)
    auth-expiration = 3600
```

## Usage

### Basic Workflow

```bash
# 1. Add a large file to Git LFS
git add large-file.bin

# 2. Commit
git commit -m "Add large file"

# 3. Push (blossom-lfs will handle the upload)
git push
```

### Verify Configuration

```bash
# Check Git LFS configuration
git lfs env

# Test blossom-lfs binary
./target/release/blossom-lfs --config-info

# Dry run (no network changes)
./target/release/blossom-lfs --help
```

## Troubleshooting

### Git LFS Not Found

**Error:** `git: 'lfs' is not a git command`

**Solution:**
```bash
# Install Git LFS
brew install git-lfs        # macOS
sudo apt-get install git-lfs  # Ubuntu/Debian
sudo yum install git-lfs      # RHEL/CentOS

# Initialize
git lfs install
```

### BlossomLFS Not Found

**Error:** `Custom transfer agent 'blossom-lfs' not found`

**Solution:**
```bash
# Check if the path is absolute and correct
git config lfs.customtransfer.blossom-lfs.path

# Should return something like:
# /Users/you/projects/blossom-lfs/target/release/blossom-lfs

# If path is wrong, set it again with absolute path:
git config lfs.customtransfer.blossom-lfs.path "$(pwd)/target/release/blossom-lfs"
```

### Authentication Errors

**Error:** `Failed to create auth token` or `Invalid private key`

**Solution:**
```bash
# Verify your private key is valid (nsec or hex format)
# Make sure you're using the correct nsec from your Nostr wallet

# Option 1: Set in config
git config lfs-dal.private-key 'nsec1...'

# Option 2: Use environment variable
export NOSTR_PRIVATE_KEY='nsec1...'
```

### Upload Failures

**Error:** `Upload failed: connection refused`

**Solution:**
```bash
# 1. Check Blossom server is accessible
curl -I https://your-blossom-server.com

# 2. Verify server URL in config
git config lfs-dal.server

# 3. Check auth token expiration
git config lfs-dal.auth-expiration

# 4. Enable debug logging
git config lfs.customtransfer.blossom-lfs.args "--log-output=debug.log --log-level=debug"
```

### Large File Issues

**Error:** `File too large for single upload`

**Solution:**
```bash
# BlossomLFS automatically chunks files > 16MB
# Adjust chunk size if needed:
git config lfs-dal.chunk-size 16777216  # 16MB (default)

# For very large files (>2GB), consider:
git config lfs-dal.chunk-size 67108864  # 64MB chunks
git config lfs-dal.max-concurrent-uploads 4  # Reduce concurrency
```

### Download Failures

**Error:** `Manifest not found`

**Solution:**
```bash
# The file was never uploaded, or server URL is wrong
# 1. Verify server URL
git config lfs-dal.server

# 2. Check if file exists on server
curl https://your-blossom-server.com/<sha256>

# 3. Re-upload the file
git lfs push --object-id <oid>
```

## Environment Variables

You can also configure via environment variables:

```bash
export BLOSSOM_SERVER_URL="https://your-blossom-server.com"
export NOSTR_PRIVATE_KEY="nsec1..."
```

## Log Files

Enable debug logging:

```bash
git config lfs.customtransfer.blossom-lfs.args "--log-output=blossom-lfs.log --log-level=debug"
git config lfs.customtransfer.blossom-lfs.concurrent false  # Avoid log interleaving
```

## Testing Your Setup

```bash
# Create a test file
dd if=/dev/urandom of=test-large.bin bs=1M count=20

# Configure Git LFS tracking
echo "*.bin filter=lfs diff=lfs merge=lfs -text" > .gitattributes
git add .gitattributes
git commit -m "Configure LFS for binary files"

# Add and commit the large file
git add test-large.bin
git commit -m "Add test large file"

# Push to trigger upload
git push

# Verify the file was uploaded to Blossom
# Check your Blossom server dashboard or:
curl -I https://your-blossom-server.com/<sha256>
```

## Getting Help

- **GitHub Issues:** https://github.com/your-org/blossom-lfs/issues
- **Documentation:** See README.md for technical details
- **Blossom Protocol:** https://github.com/hzrd149/blossom

## Security Notes

⚠️ **Never commit your private key to version control!**

Use environment variables or `.git/config` (not tracked):
```bash
# Good: Environment variable
export NOSTR_PRIVATE_KEY='nsec1...'

# Good: In .git/config (local only)
git config lfs-dal.private-key 'nsec1...'

# BAD: In .lfsdalconfig (tracked in repo)
# Don't do this if you share the repo!
```

## Performance Tips

1. **Use appropriate chunk size:** Smaller chunks = more parallel uploads, but more overhead
2. **Adjust concurrency:** Lower values for slower connections
3. **Enable caching:** Blossom servers may cache frequently-accessed blobs
4. **Use CDN:** Configure your Blossom server behind a CDN for faster downloads

## Uninstalling

```bash
# Remove Git LFS tracking
git lfs uninstall --local

# Remove configuration
git config --unset lfs.standalonetransferagent
git config --unset lfs.customtransfer.blossom-lfs.path
git config --unset lfs-dal.server
git config --unset lfs-dal.private-key

# Remove .lfsconfig if created
rm .lfsconfig

# Remove binary
rm target/release/blossom-lfs
```
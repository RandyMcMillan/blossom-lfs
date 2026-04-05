#!/bin/bash
# Setup script for BlossomLFS
# Run this script to configure Git LFS to use blossom-lfs

set -e

echo "=== BlossomLFS Setup ==="
echo ""

# Check if Git LFS is installed
if ! command -v git-lfs &> /dev/null; then
    echo "❌ Git LFS is not installed!"
    echo ""
    echo "Please install Git LFS first:"
    echo "  macOS:    brew install git-lfs"
    echo "  Linux:    sudo apt-get install git-lfs (Ubuntu/Debian)"
    echo "            sudo yum install git-lfs (RHEL/CentOS)"
    echo ""
    exit 1
fi

echo "✓ Git LFS is installed: $(git lfs version)"
echo ""

# Check if in a git repository
if [ ! -d ".git" ]; then
    echo "❌ Not in a Git repository!"
    echo "Please run this script from the root of your Git repository."
    exit 1
fi

echo "✓ Git repository detected"
echo ""

# Initialize Git LFS if not already initialized
if [ ! -d ".git/lfs" ]; then
    echo "Initializing Git LFS..."
    git lfs install
    echo "✓ Git LFS initialized"
else
    echo "✓ Git LFS already initialized"
fi
echo ""

# Build release binary if not exists
if [ ! -f "target/release/blossom-lfs" ]; then
    echo "Building release binary..."
    cargo build --release
    echo "✓ Build complete"
else
    echo "✓ Release binary already exists"
fi
echo ""

# Get absolute path to binary
BLOSSOM_LFS_PATH="$(pwd)/target/release/blossom-lfs"

echo "Configuring Git LFS to use blossom-lfs..."
echo ""

# Configure Git LFS
git config lfs.standalonetransferagent blossom-lfs
git config lfs.customtransfer.blossom-lfs.path "$BLOSSOM_LFS_PATH"

# Avoid pushing to default LFS server
if ! git config --file .lfsconfig lfs.url > /dev/null 2>&1; then
    git config -f .lfsconfig lfs.url blossom-lfs
    echo "✓ Created .lfsconfig to prevent default LFS server usage"
fi

echo ""
echo "=== Configuration Complete ==="
echo ""
echo "Git LFS is now configured to use blossom-lfs"
echo ""
echo "Configuration:"
echo "  Transfer agent:   $(git config lfs.standalonetransferagent)"
echo "  Binary path:      $(git config lfs.customtransfer.blossom-lfs.path)"
echo ""
echo "Next steps:"
echo "  1. Configure your Blossom server:"
echo "     git config lfs-dal.server https://your-blossom-server.com"
echo ""
echo "  2. Set your Nostr private key (choose one):"
echo "     Option A (in config): git config lfs-dal.private-key 'nsec1...'"
echo "     Option B (env var):  export NOSTR_PRIVATE_KEY='nsec1...'"
echo ""
echo "  3. (Optional) Customize chunk size:"
echo "     git config lfs-dal.chunk-size 16777216  # 16MB (default)"
echo ""
echo "  4. Track files with LFS:"
echo "     git lfs track '*.large'"
echo "     git add .gitattributes"
echo "     git commit -m 'Configure Git LFS'"
echo ""
echo "  5. Commit and push:"
echo "     git add your-large-file.bin"
echo "     git commit -m 'Add large file'"
echo "     git push"
echo ""

# Test the binary
echo "Testing blossom-lfs binary..."
if "$BLOSSOM_LFS_PATH" --config-info > /dev/null 2>&1; then
    echo "✓ Binary is functioning correctly"
else
    echo "⚠ Warning: Binary test failed, but continuing setup"
fi

echo ""
echo "Setup complete! 🎉"
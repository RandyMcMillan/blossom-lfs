#!/bin/bash
# Test script for BlossomLFS
# This script creates a simple test to verify your BlossomLFS setup

set -e

echo "=== BlossomLFS Test Script ==="
echo ""

# Check configuration
echo "Checking configuration..."
if ! git config lfs.standalonetransferagent &> /dev/null; then
    echo "❌ BlossomLFS not configured!"
    echo "Run ./setup.sh first"
    exit 1
fi

TRANSFER_AGENT=$(git config lfs.standalonetransferagent)
if [ "$TRANSFER_AGENT" != "blossom-lfs" ]; then
    echo "❌ Wrong transfer agent: $TRANSFER_AGENT"
    exit 1
fi

echo "✓ Transfer agent: $TRANSFER_AGENT"

# Check binary
BINARY_PATH=$(git config lfs.customtransfer.blossom-lfs.path)
if [ ! -f "$BINARY_PATH" ]; then
    echo "❌ Binary not found at: $BINARY_PATH"
    exit 1
fi

echo "✓ Binary exists: $BINARY_PATH"

# Check server configuration
if ! git config lfs-dal.server &> /dev/null && [ -z "$BLOSSOM_SERVER_URL" ]; then
    echo "⚠ Warning: Blossom server URL not configured"
    echo "  Set via: git config lfs-dal.server https://your-server.com"
    echo "  Or: export BLOSSOM_SERVER_URL=https://your-server.com"
else
    if git config lfs-dal.server &> /dev/null; then
        echo "✓ Server URL: $(git config lfs-dal.server)"
    else
        echo "✓ Server URL (env): $BLOSSOM_SERVER_URL"
    fi
fi

# Check private key
if ! git config lfs-dal.private-key &> /dev/null && [ -z "$NOSTR_PRIVATE_KEY" ]; then
    echo "⚠ Warning: Nostr private key not configured"
    echo "  Set via: git config lfs-dal.private-key 'nsec1...'"
    echo "  Or: export NOSTR_PRIVATE_KEY='nsec1...'"
else
    echo "✓ Private key configured"
fi

echo ""
echo "=== Creating Test File ==="
echo ""

# Create test file
TEST_FILE="blossom-lfs-test-$(date +%s).bin"
dd if=/dev/urandom of="$TEST_FILE" bs=1M count=1 2>/dev/null
echo "✓ Created test file: $TEST_FILE (1MB)"

# Configure LFS for test file
echo "*.bin filter=lfs diff=lfs merge=lfs -text" > .gitattributes
echo "✓ Configured .gitattributes"

echo ""
echo "=== Test Instructions ==="
echo ""
echo "To test upload:"
echo "  1. git add $TEST_FILE .gitattributes"
echo "  2. git commit -m 'Test BlossomLFS upload'"
echo "  3. git push"
echo ""
echo "To test download:"
echo "  1. rm $TEST_FILE"
echo "  2. git checkout $TEST_FILE"
echo ""
echo "To clean up:"
echo "  rm $TEST_FILE .gitattributes"
echo ""
echo "✓ Configuration is complete and ready to use!"
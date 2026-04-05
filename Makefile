.PHONY: all build release test clean install setup run check

# Default target
all: test

# Build in debug mode
build:
	cargo build

# Build optimized release binary
release:
	cargo build --release
	@echo "✓ Release binary created at: target/release/blossom-lfs"

# Run all tests
test:
	cargo test
	@echo "✓ All tests passed"

# Clean build artifacts
clean:
	cargo clean
	rm -f *.log
	@echo "✓ Cleaned"

# Install binary (requires cargo)
install: release
	cargo install --path .
	@echo "✓ Installed blossom-lfs to cargo bin directory"

# Run setup script for Git LFS configuration
setup: release
	./setup.sh

# Run test setup script
test-setup:
	./test-setup.sh

# Run with example echo request
run: release
	@echo '{"event":"init"}' | target/release/blossom-lfs

# Check code quality
check:
	cargo clippy -- -D warnings
	cargo fmt -- --check
	@echo "✓ Code quality checks passed"

# Generate documentation
doc:
	cargo doc --open

# Show configuration info
config-info: release
	./target/release/blossom-lfs --config-info

# Build and test everything
ci: check test release
	@echo "✓ CI checks complete"

# Development watch mode (requires cargo-watch)
watch:
	cargo watch -x test -x build

# Format code
fmt:
	cargo fmt

# Security audit
audit:
	cargo audit

# Performance benchmark (requires cargo-criterion)
bench:
	cargo criterion

# Update dependencies
update:
	cargo update
	cargo test
	@echo "✓ Dependencies updated and tested"
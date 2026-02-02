.PHONY: all build test clippy fmt check wasm clean install

# Default target
all: check build test

# Build all crates
build:
	cargo build --all

# Build release
release:
	cargo build --release --all

# Run all tests
test:
	cargo test --all

# Run clippy lints
clippy:
	cargo clippy --all -- -D warnings

# Check formatting
fmt:
	cargo fmt --all -- --check

# Format code
fmt-fix:
	cargo fmt --all

# Full CI check (fmt + clippy + test)
check: fmt clippy

# Build WASM package
wasm:
	cd crates/spindle-wasm && wasm-pack build --target web --release

# Build WASM for Node.js
wasm-node:
	cd crates/spindle-wasm && wasm-pack build --target nodejs --release

# Build WASM for bundlers
wasm-bundler:
	cd crates/spindle-wasm && wasm-pack build --target bundler --release

# Install CLI
install:
	cargo install --path crates/spindle-cli

# Clean build artifacts
clean:
	cargo clean
	rm -rf crates/spindle-wasm/pkg

# Run benchmarks (if any)
bench:
	cargo bench --all

# Generate documentation
doc:
	cargo doc --all --no-deps

# Open documentation in browser
doc-open:
	cargo doc --all --no-deps --open

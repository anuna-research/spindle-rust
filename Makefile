.PHONY: all build test clippy fmt check wasm clean install bench bench-scaling bench-compare bench-memory

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

# Run benchmarks (quick suite, ~1-2 min)
bench:
	cargo bench --package spindle-core

# Run large-scale benchmarks (finds algorithm crossover points)
bench-scaling:
	cargo bench --package spindle-core -- "scaling"

# Compare benchmarks between two commits
# Usage:
#   make bench-compare                              # Compare HEAD~1 vs HEAD (quick suite)
#   make bench-compare BASELINE=main                # Compare main vs HEAD
#   make bench-compare BASELINE=v1.0 COMPARE=v1.1   # Compare tags
#   make bench-compare FILTER=scaling               # Run large-scale benchmarks
#   make bench-compare FILTER=reason                # Only reasoning benchmarks
BASELINE ?= HEAD~1
COMPARE ?= HEAD
FILTER ?=
bench-compare:
	@./scripts/bench-compare.sh "$(BASELINE)" "$(COMPARE)" "$(FILTER)"

# Profile memory usage (generates dhat-heap.json)
# View at: https://nnethercote.github.io/dh_view/dh_view.html
bench-memory:
	cargo run --package spindle-core --example memory_profile --features dhat-heap --release
	@echo "Memory profile saved to dhat-heap.json"
	@echo "View at: https://nnethercote.github.io/dh_view/dh_view.html"

# Generate documentation
doc:
	cargo doc --all --no-deps

# Open documentation in browser
doc-open:
	cargo doc --all --no-deps --open

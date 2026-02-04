#!/usr/bin/env bash
# bench-compare.sh - Compare benchmark performance between two commits
#
# Usage: ./scripts/bench-compare.sh [BASELINE] [COMPARE] [FILTER]
#   BASELINE: commit/branch to use as baseline (default: HEAD~1)
#   COMPARE:  commit/branch to compare against baseline (default: HEAD)
#   FILTER:   benchmark filter regex (default: runs quick suite, use "scaling" for large tests)
#
# Example:
#   ./scripts/bench-compare.sh HEAD~5 HEAD           # Compare last 5 commits
#   ./scripts/bench-compare.sh main feature          # Compare branches
#   ./scripts/bench-compare.sh v1.0.0 v1.1.0         # Compare tags
#   ./scripts/bench-compare.sh HEAD~1 HEAD scaling   # Run large-scale benchmarks
#   ./scripts/bench-compare.sh HEAD~1 HEAD "reason"  # Only reasoning benchmarks

set -e

BASELINE="${1:-HEAD~1}"
COMPARE="${2:-HEAD}"
FILTER="${3:-}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log() {
    echo -e "${BLUE}[bench-compare]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[bench-compare]${NC} $1"
}

error() {
    echo -e "${RED}[bench-compare]${NC} $1"
    exit 1
}

success() {
    echo -e "${GREEN}[bench-compare]${NC} $1"
}

# Resolve commit refs to short hashes for display
BASELINE_HASH=$(git rev-parse --short "$BASELINE" 2>/dev/null) || error "Invalid baseline: $BASELINE"
COMPARE_HASH=$(git rev-parse --short "$COMPARE" 2>/dev/null) || error "Invalid compare: $COMPARE"

log "Comparing benchmarks:"
log "  Baseline: $BASELINE ($BASELINE_HASH)"
log "  Compare:  $COMPARE ($COMPARE_HASH)"
echo

# Save current state
ORIGINAL_REF=$(git symbolic-ref --short HEAD 2>/dev/null || git rev-parse --short HEAD)
log "Current position: $ORIGINAL_REF"

# Check for uncommitted changes
STASHED=false
if ! git diff --quiet || ! git diff --cached --quiet; then
    warn "Uncommitted changes detected, stashing..."
    git stash push -m "bench-compare: temporary stash"
    STASHED=true
fi

# Cleanup function for restoration
cleanup() {
    log "Restoring original state..."
    git checkout "$ORIGINAL_REF" --quiet 2>/dev/null || git checkout "$ORIGINAL_REF"
    if [ "$STASHED" = true ]; then
        log "Restoring stashed changes..."
        git stash pop --quiet
    fi
}

# Set trap for cleanup on exit (success or failure)
trap cleanup EXIT

# Benchmark baseline
log "Checking out baseline: $BASELINE"
git checkout "$BASELINE" --quiet

log "Building baseline (release mode)..."
cargo build --release --all --quiet

log "Running baseline benchmarks..."
if [ -n "$FILTER" ]; then
    cargo bench --package spindle-core -- "$FILTER" --save-baseline baseline
else
    cargo bench --package spindle-core -- --save-baseline baseline
fi

echo
echo "=============================================="
echo

# Benchmark comparison target
log "Checking out compare target: $COMPARE"
git checkout "$COMPARE" --quiet

log "Building compare target (release mode)..."
cargo build --release --all --quiet

log "Running comparison benchmarks..."
if [ -n "$FILTER" ]; then
    cargo bench --package spindle-core -- "$FILTER" --baseline baseline
else
    cargo bench --package spindle-core -- --baseline baseline
fi

echo
success "Benchmark comparison complete!"
log "Baseline: $BASELINE ($BASELINE_HASH)"
log "Compare:  $COMPARE ($COMPARE_HASH)"

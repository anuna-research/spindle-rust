#!/bin/bash
# SPDX-License-Identifier: LGPL-3.0-or-later
# Copyright (c) 2026 Anuna Research

set -e

# Spindle Release Script
# Usage: ./release.sh [version]
# Example: ./release.sh 0.2.0

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info() { echo -e "${GREEN}[INFO]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Must be on main
BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [[ "$BRANCH" != "main" ]]; then
  error "Releases must be cut from main (currently on '$BRANCH')"
fi

# Get version from argument or prompt
VERSION="${1:-}"

if [[ -z "$VERSION" ]]; then
  # Get current version from workspace Cargo.toml
  CURRENT=$(grep '^version' Cargo.toml | head -1 | grep -o '"[^"]*"' | tr -d '"')
  echo "Current version: $CURRENT"
  read -p "Enter new version (without 'v' prefix): " VERSION
fi

if [[ -z "$VERSION" ]]; then
  error "Version is required"
fi

# Validate semver format (X.Y.Z with optional pre-release and build metadata)
SEMVER_RE='^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$'
if ! [[ "$VERSION" =~ $SEMVER_RE ]]; then
  error "Invalid semver format. Examples: 0.2.0, 1.0.0-alpha.1, 1.0.0-rc.1+build.42"
fi

TAG="v$VERSION"

# Check for uncommitted changes
if ! git diff --quiet || ! git diff --cached --quiet; then
  error "You have uncommitted changes. Commit or stash them first."
fi

# Check if tag already exists
if git rev-parse "$TAG" >/dev/null 2>&1; then
  error "Tag $TAG already exists"
fi

# Confirmation prompt
echo ""
info "Release summary:"
echo "  Version:  $VERSION"
echo "  Tag:      $TAG"
echo "  Branch:   main"
echo ""
read -p "Proceed with release? [y/N] " CONFIRM
if [[ "$CONFIRM" != "y" && "$CONFIRM" != "Y" ]]; then
  echo "Aborted."
  exit 0
fi

# Update workspace version in Cargo.toml (portable sed)
info "Updating workspace version in Cargo.toml..."
if sed --version >/dev/null 2>&1; then
  # GNU sed (Linux)
  sed -i "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" Cargo.toml
else
  # BSD sed (macOS)
  sed -i '' "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" Cargo.toml
fi

# Regenerate Cargo.lock with version change only
info "Updating Cargo.lock..."
cargo check --quiet

# Run tests
info "Running tests..."
cargo test --all

# Check clippy
info "Running clippy..."
cargo clippy --all -- -D warnings

# Commit version bump
info "Committing version bump..."
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to $VERSION"

# Create and push tag
info "Creating tag $TAG..."
git tag -a "$TAG" -m "Release $VERSION"

# Push to remote
info "Pushing to origin..."
git push origin main
git push origin "$TAG"

echo ""
info "Release $TAG published!"
echo ""
echo "Woodpecker release pipeline triggered."
echo "Release will be available at:"
echo "  https://files.anuna.io/spindle/$TAG/"
echo "  https://files.anuna.io/spindle/latest/"

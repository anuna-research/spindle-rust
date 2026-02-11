#!/usr/bin/env bash
set -euo pipefail

# Deploy docs to Cloudflare Pages (local equivalent of CI deploy-docs step).
# Requires: mdbook, npx/wrangler, CLOUDFLARE_API_TOKEN, CLOUDFLARE_ACCOUNT_ID

cd "$(git rev-parse --show-toplevel)"

mdbook build docs
npx wrangler pages deploy docs/book --project-name=spindle-docs --commit-dirty=true

#!/usr/bin/env bash
set -euo pipefail

if ! command -v gh >/dev/null 2>&1; then
    echo "GitHub CLI (gh) is required. Install it from https://cli.github.com/" >&2
    exit 127
fi

gh auth login
gh auth setup-git
gh auth status

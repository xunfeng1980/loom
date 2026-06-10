#!/usr/bin/env bash
# L2Core AST sync checklist wrapper (Phase 48 P5).
# Runs the Python checker and exits with its status.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "${SCRIPT_DIR}/l2core-sync-checklist.py" "$@"

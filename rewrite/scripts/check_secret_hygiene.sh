#!/usr/bin/env bash
# Fails if secret-handling rules are violated:
# 1. expose_secret() outside the two permitted call sites.
# 2. Anything resembling a real bearer token in committed test fixtures.
set -euo pipefail
cd "$(dirname "$0")/.."

allowed_files=("crates/grid-core/src/secrets.rs" "crates/grid-core/src/romm/mod.rs")
violations=$(grep -rn "expose_secret" crates app/src-tauri --include="*.rs" \
  | grep -vF -e "${allowed_files[0]}" -e "${allowed_files[1]}" || true)
if [ -n "$violations" ]; then
  echo "expose_secret() outside permitted call sites:" >&2
  echo "$violations" >&2
  exit 1
fi

# Real-looking secrets in tests/fixtures: long bearer-ish strings that are not
# the sanctioned fake. The fake token is allowed everywhere.
suspicious=$(grep -rnE "(Bearer|token|password)[\"': =]+[A-Za-z0-9+/_-]{30,}" \
  crates app/src --include="*.rs" --include="*.ts" --include="*.json" \
  | grep -v "FAKE-TEST-TOKEN-not-real" || true)
if [ -n "$suspicious" ]; then
  echo "Possible real credential in committed code/fixtures:" >&2
  echo "$suspicious" >&2
  exit 1
fi
echo "secret hygiene OK"

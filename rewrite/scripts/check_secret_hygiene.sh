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
# the sanctioned fakes. The fake tokens are allowed everywhere.
scan_dirs=(crates app/src app/src-tauri)
if [ -d e2e ]; then
  scan_dirs+=(e2e)
fi
suspicious=$(grep -rnE "(Bearer|token|password)[\"': =]+[A-Za-z0-9+/_-]{30,}" \
  "${scan_dirs[@]}" --include="*.rs" --include="*.ts" --include="*.svelte" --include="*.json" \
  | grep -v -e "FAKE-TEST-TOKEN-not-real" -e "FAKE-E2E-TOKEN-not-real" || true)
if [ -n "$suspicious" ]; then
  echo "Possible real credential in committed code/fixtures:" >&2
  echo "$suspicious" >&2
  exit 1
fi

# The `e2e` cargo feature embeds a WebDriver automation server
# (tauri-plugin-wdio + tauri-plugin-wdio-webdriver). It must never appear in
# the default (release) dependency tree, and it must be reachable when the
# feature is explicitly enabled — otherwise the e2e harness is silently
# broken. Run from the workspace root so `-p app` resolves either way.
# This first check only holds while `app` defines no `[features] default`
# key: if one is ever added and lists `e2e`, `cargo tree -p app` would enable
# the feature and the check would fail loudly rather than pass wrongly.
if cargo tree -p app --quiet 2>/dev/null | grep -qi wdio; then
  echo "wdio plugin found in the DEFAULT dependency tree of 'app' (no --features e2e)." >&2
  echo "The embedded WebDriver server must never ship in a release build." >&2
  echo "Check the [features] e2e = [...] wiring in app/src-tauri/Cargo.toml." >&2
  exit 1
fi
if ! cargo tree -p app --features e2e --quiet 2>/dev/null | grep -qi wdio; then
  echo "wdio plugin NOT found in 'app' with --features e2e enabled." >&2
  echo "The e2e feature should pull in tauri-plugin-wdio and tauri-plugin-wdio-webdriver." >&2
  echo "Check the [features] e2e = [...] wiring in app/src-tauri/Cargo.toml." >&2
  exit 1
fi

echo "secret hygiene OK"

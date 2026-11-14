#!/usr/bin/env bash
# Fails if secret-handling rules are violated:
# 1. expose_secret() outside the permitted call sites.
# 2. Anything resembling a real bearer token in committed test fixtures.
set -euo pipefail
cd "$(dirname "$0")/.."

# `retroachievements.rs` is on this list for two calls: the blank-password
# check and the RA login endpoint, which takes the password as a query
# parameter and has no header form.
# `tests/ra_login.rs` is on it for one assertion: that the FAKE token the mock
# server returns is the one the login hands back.
# `stores/gog/auth.rs` and `stores/gog/api.rs` are on it for the GOG login
# code and the in-memory bearer the GOG HTTP APIs take (the store-token
# exception in AGENTS.md).
# `stores/runner.rs` is on it for writing a tool's stdin secret (legendary's
# login code) and for handing a tool's kept stdout to a JSON parse.
# `stores/epic/launch.rs` is on it for Epic's exchange code, which goes into
# the game's argv as a redacted argument.
# `stores/amazon/auth.rs` is on it for the Amazon login: the code and nile's
# client_id, code_verifier and serial, which only nile register's argv takes
# (nile has no stdin or environment path for them).
# `stores/steam/api.rs` is on it for the Steam Web API key, which only goes
# into a request's query at send time (Steam has no header form).
allowed_files=("crates/grid-core/src/secrets.rs" "crates/grid-core/src/romm/mod.rs" "crates/grid-core/src/autoconfig/mod.rs" "crates/grid-core/src/retroachievements.rs" "crates/grid-core/tests/ra_login.rs" "crates/grid-core/src/stores/gog/auth.rs" "crates/grid-core/src/stores/gog/api.rs" "crates/grid-core/src/stores/runner.rs" "crates/grid-core/src/stores/epic/launch.rs" "crates/grid-core/src/stores/amazon/auth.rs" "crates/grid-core/src/stores/steam/api.rs")
allowed_args=()
for f in "${allowed_files[@]}"; do
  allowed_args+=(-e "$f")
done
violations=$(grep -rn "expose_secret" crates app/src-tauri --include="*.rs" \
  | grep -vF "${allowed_args[@]}" || true)
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
  "${scan_dirs[@]}" --include="*.rs" --include="*.ts" --include="*.svelte" --include="*.json" --include="*.mjs" \
  | grep -v -e "FAKE-TEST-TOKEN-not-real" -e "FAKE-E2E-TOKEN-not-real" -e "FAKE-RA-TOKEN-not-real" \
      -e "FAKE-STORE-TOKEN-not-real" -e "FAKE-STEAM-KEY-not-real" || true)
if [ -n "$suspicious" ]; then
  echo "Possible real credential in committed code/fixtures:" >&2
  echo "$suspicious" >&2
  exit 1
fi

# External game stores (AGENTS.md "Secrets"): legendary, gogdl and nile print
# tokens and account details, so their stdout/stderr is never logged or
# forwarded. In the store code, a log or print macro whose invocation (up to
# its closing `;`, so multi-line calls count) mentions stdout or stderr fails,
# and println!/eprintln!/dbg! are refused outright. The app-side store files
# are checked too once they exist.
store_paths=()
for p in crates/grid-core/src/stores app/src-tauri/src/stores_service \
         app/src-tauri/src/store_auth.rs app/src-tauri/src/commands/stores.rs; do
  if [ -e "$p" ]; then
    store_paths+=("$p")
  fi
done
if [ "${#store_paths[@]}" -gt 0 ]; then
  store_logs=$(find "${store_paths[@]}" -name '*.rs' -print0 | xargs -0 awk '
    FNR == 1 { inside = 0 }
    {
      if (!inside && $0 ~ /(println|eprintln|print|eprint|dbg)!/ && $0 !~ /^[[:space:]]*\/\//) {
        print FILENAME ":" FNR ": print macro in store code: " $0
      }
      if (!inside && $0 ~ /(tracing::|log::)?(trace|debug|info|warn|error|event)!/ && $0 !~ /^[[:space:]]*\/\//) {
        inside = 1; start = FNR; text = ""
      }
      if (inside) {
        text = text " " $0
        if ($0 ~ /;/) {
          if (tolower(text) ~ /std(out|err)/) {
            print FILENAME ":" start ": store tool output in a log macro:" text
          }
          inside = 0
        }
      }
    }' || true)
  if [ -n "$store_logs" ]; then
    echo "Store code must never log or print a store tool's stdout/stderr:" >&2
    echo "$store_logs" >&2
    exit 1
  fi
fi

# The RetroAchievements token's keyring account name may appear only in
# secrets.rs — the one file that opens that keyring item.
ra_account_users=$(grep -rl "RA_ACCOUNT\|retroachievements-token" crates app/src-tauri/src app/src --include="*.rs" --include="*.ts" --include="*.svelte" || true)
if [ -n "$ra_account_users" ] && [ "$ra_account_users" != "crates/grid-core/src/secrets.rs" ]; then
  echo "RA_ACCOUNT/retroachievements-token referenced outside secrets.rs:" >&2
  echo "$ra_account_users" >&2
  exit 1
fi

# The Steam Web API key's keyring account name may appear only in
# secrets.rs, as the RA slot's.
steam_account_users=$(grep -rl "STEAM_KEY_ACCOUNT\|steam-web-api-key" crates app/src-tauri/src app/src --include="*.rs" --include="*.ts" --include="*.svelte" || true)
if [ -n "$steam_account_users" ] && [ "$steam_account_users" != "crates/grid-core/src/secrets.rs" ]; then
  echo "STEAM_KEY_ACCOUNT/steam-web-api-key referenced outside secrets.rs:" >&2
  echo "$steam_account_users" >&2
  exit 1
fi

# The Steam Web API key has no config key, IPC field or struct field outside
# the keyring code: a field named like one means the key is (or was meant to
# be) serialized somewhere.
steam_key_field=$(grep -rnwE "steam_api_key|web_api_key|steam_key|steamApiKey|webApiKey" crates app/src-tauri/src app/src --include="*.rs" --include="*.ts" --include="*.svelte" | grep -v -e "^crates/grid-core/src/secrets.rs:" || true)
if [ -n "$steam_key_field" ]; then
  echo "A Steam key field (steam_api_key / web_api_key / steam_key) was found outside secrets.rs:" >&2
  echo "$steam_key_field" >&2
  exit 1
fi

# The Steam key travels only in a request's query, so in the Steam store code
# no format or log macro may name a URL and the key together (it could print
# a URL that carries the key). A macro's text runs to its balanced closing
# parenthesis, so multi-line calls count.
steam_paths=()
for p in crates/grid-core/src/stores/steam app/src-tauri/src/stores_service/steam.rs; do
  if [ -e "$p" ]; then
    steam_paths+=("$p")
  fi
done
if [ "${#steam_paths[@]}" -gt 0 ]; then
  steam_url_key=$(find "${steam_paths[@]}" -name '*.rs' -print0 | xargs -0 awk '
    FNR == 1 { inside = 0 }
    {
      line = $0
      if (!inside && line !~ /^[[:space:]]*\/\// && match(line, /(format|format_args|write|writeln|panic|print|println|eprint|eprintln|trace|debug|info|warn|error|event)!\(/)) {
        inside = 1; start = FNR; text = ""; depth = 0
        line = substr(line, RSTART)
      }
      if (inside) {
        text = text " " line
        n = length(line)
        for (i = 1; i <= n; i++) {
          c = substr(line, i, 1)
          if (c == "(") depth++
          else if (c == ")") { depth--; if (depth == 0) break }
        }
        if (depth <= 0) {
          if (text ~ /([Uu]rl|URL)/ && text ~ /[Kk]ey/) {
            print FILENAME ":" start ": a URL and the key in one macro:" text
          }
          inside = 0
        }
      }
    }' || true)
  if [ -n "$steam_url_key" ]; then
    echo "Steam code must never format a URL together with the key:" >&2
    echo "$steam_url_key" >&2
    exit 1
  fi
fi

# `Config` carries `retroachievements_username` only; the token has no config
# key. A field literally named `retroachievements_token` would mean the
# token is (or was meant to be) serialized to config.toml/an IPC payload
# instead of living only in the keyring.
ra_token_field=$(grep -rn "retroachievements_token" crates app/src-tauri/src app/src --include="*.rs" --include="*.ts" --include="*.svelte" | grep -v -e "^crates/grid-core/src/secrets.rs:" || true)
if [ -n "$ra_token_field" ]; then
  echo "A field literally named 'retroachievements_token' was found outside secrets.rs:" >&2
  echo "$ra_token_field" >&2
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

# The `e2e` cargo feature on grid-core enables a request-time forge-URL
# redirect (GRID_LAUNCHER_E2E_FORGE_BASE, launch/forge.rs) used only by the
# E2E harness to point emulator downloads at a local fixture server. It must
# never be reachable from a default (release) build, and the env var it reads
# must be read nowhere else, so a release binary can never be pointed at an
# attacker-controlled host via that variable.
grid_core_default_features=$(cargo tree -p app --format "{p} [{f}]" --quiet 2>/dev/null | grep -i "grid-core v")
if echo "$grid_core_default_features" | grep -q '\[e2e\]\|,e2e\|e2e,'; then
  echo "grid-core feature \"e2e\" found in the DEFAULT dependency tree of 'app' (no --features e2e)." >&2
  echo "The forge request-time redirect must never be reachable in a release build." >&2
  echo "Check the [features] e2e = [...] wiring in app/src-tauri/Cargo.toml." >&2
  exit 1
fi
grid_core_e2e_features=$(cargo tree -p app --features e2e --format "{p} [{f}]" --quiet 2>/dev/null | grep -i "grid-core v")
if ! echo "$grid_core_e2e_features" | grep -q '\[e2e\]\|,e2e\|e2e,'; then
  echo "grid-core feature \"e2e\" NOT found in 'app' with --features e2e enabled." >&2
  echo "Check the [features] e2e = [...] wiring in app/src-tauri/Cargo.toml." >&2
  exit 1
fi

env_var_users=$(grep -rl "GRID_LAUNCHER_E2E_FORGE_BASE" crates app/src-tauri/src --include="*.rs" || true)
if [ -n "$env_var_users" ] && [ "$env_var_users" != "crates/grid-core/src/launch/forge.rs" ]; then
  echo "GRID_LAUNCHER_E2E_FORGE_BASE referenced outside launch/forge.rs:" >&2
  echo "$env_var_users" >&2
  exit 1
fi

echo "secret hygiene OK"

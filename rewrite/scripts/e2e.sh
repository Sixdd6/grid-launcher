#!/usr/bin/env bash
#
# End-to-end test runner for the Rust rewrite.
#
# Runs WebdriverIO against a real, locally built Tauri binary that talks to a
# mock RomM server. Nothing here touches ~/.config/grid-launcher or the real
# OS keyring: every stage gets a temp GRID_LAUNCHER_DATA_DIR, and the whole
# run happens inside a private D-Bus session with its own gnome-keyring.
#
#   rewrite/scripts/e2e.sh              # build, then run every stage
#   rewrite/scripts/e2e.sh connect      # run only the named stage groups
#   E2E_SKIP_BUILD=1 rewrite/scripts/e2e.sh
#   E2E_KEEP=1 rewrite/scripts/e2e.sh   # keep the temp run directory
#
# Exit codes: 0 pass, 1 a stage failed, 2 a prerequisite is missing.

set -uo pipefail

REWRITE_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
E2E_DIR="$REWRITE_DIR/e2e"
APP_DIR="$REWRITE_DIR/app"
APP_BINARY="$REWRITE_DIR/target/debug/app"

# A stage group = "name:spec [spec...]". Each group gets one fresh data
# directory and one mock server; each spec inside it is a separate `wdio run`
# (and therefore a separate app process) sharing that state. That is how the
# restore pair works: (a) connects, (b) relaunches and must find the session.
STAGE_GROUPS=(
  "connect:specs/connect.spec.ts"
  "connect-restore:specs/connect-restore-a.spec.ts specs/connect-restore-b.spec.ts"
)

die() { printf 'e2e: %s\n' "$*" >&2; exit 1; }
say() { printf '\n== %s\n' "$*"; }

# ---------------------------------------------------------------------------
# Outer pass: preflight, build, then re-exec inside a private D-Bus session.
# ---------------------------------------------------------------------------
if [[ "${E2E_INNER:-}" != "1" ]]; then
  missing=()
  for tool in xvfb-run dbus-run-session gnome-keyring-daemon node npm; do
    command -v "$tool" >/dev/null 2>&1 || missing+=("$tool")
  done
  if (( ${#missing[@]} > 0 )); then
    printf 'e2e: missing prerequisite(s): %s\n' "${missing[*]}" >&2
    printf 'e2e: install them with:\n' >&2
    printf '  sudo dnf install -y xorg-x11-server-Xvfb dbus-daemon gnome-keyring nodejs npm\n' >&2
    exit 2
  fi

  if [[ "${E2E_SKIP_BUILD:-}" != "1" ]]; then
    say "building the e2e app binary"
    # The ONLY supported e2e build. The cargo feature and the merge config are
    # a pair and must never be used apart:
    #   --features e2e        links tauri-plugin-wdio{,-webdriver} AND makes
    #                         build.rs glob capabilities/e2e.json
    #   --config ...e2e.conf  selects that capability at runtime and turns on
    #                         withGlobalTauri
    # Feature without config => "capability with identifier e2e not found".
    # Config without feature  => the same, from the other direction.
    # VITE_E2E=1 reaches the beforeBuildCommand (vite build) by inheritance
    # and is what pulls @wdio/tauri-plugin into the frontend bundle.
    ( cd "$APP_DIR" && VITE_E2E=1 npx tauri build \
        --debug --no-bundle --features e2e --config src-tauri/tauri.e2e.conf.json ) \
      || die "build failed"
  fi
  [[ -x "$APP_BINARY" ]] || die "no app binary at $APP_BINARY (run without E2E_SKIP_BUILD=1)"

  if [[ ! -d "$E2E_DIR/node_modules" ]]; then
    say "installing e2e dependencies"
    if [[ -f "$E2E_DIR/package-lock.json" ]]; then
      ( cd "$E2E_DIR" && npm ci --no-audit --no-fund ) || die "npm ci failed"
    else
      ( cd "$E2E_DIR" && npm install --no-audit --no-fund ) || die "npm install failed"
    fi
  fi

  RUN_DIR="$(mktemp -d -t grid-e2e-XXXXXXXX)"
  export E2E_RUN_DIR="$RUN_DIR"
  # gnome-keyring stores its keyring files under XDG_DATA_HOME. Redirecting it
  # keeps the throwaway "test"-password keyring out of ~/.local/share/keyrings.
  export XDG_DATA_HOME="$RUN_DIR/xdg-data"
  mkdir -p "$XDG_DATA_HOME"
  export E2E_APP_BINARY="$APP_BINARY"
  export E2E_INNER=1
  exec dbus-run-session -- "${BASH_SOURCE[0]}" "$@"
fi

# ---------------------------------------------------------------------------
# Inner pass: private D-Bus session is live. Unlock a keyring, run the stages.
# ---------------------------------------------------------------------------
RUN_DIR="$E2E_RUN_DIR"
LOG_DIR="$RUN_DIR/logs"
mkdir -p "$LOG_DIR"

say "unlocking a throwaway gnome-keyring"
echo -n 'test' | gnome-keyring-daemon --unlock --components=secrets >/dev/null 2>&1 \
  || die "gnome-keyring-daemon --unlock failed"

selected=("$@")
failed_stages=()
ran_any=0

group_matches() {
  (( ${#selected[@]} == 0 )) && return 0
  local want
  for want in "${selected[@]}"; do [[ "$want" == "$1" ]] && return 0; done
  return 1
}

dump_failure() {
  local stage="$1" wdio_log="$2" mock_log="$3" request_log="$4"
  printf '\n---- FAILURE: stage %s ----\n' "$stage" >&2
  printf '\n-- wdio output (last 120 lines; includes forwarded app backend/frontend logs)\n' >&2
  tail -n 120 "$wdio_log" >&2 2>/dev/null || printf '(no wdio log)\n' >&2
  printf '\n-- mock server stderr/stdout (last 40 lines)\n' >&2
  tail -n 40 "$mock_log" >&2 2>/dev/null || printf '(no mock log)\n' >&2
  printf '\n-- mock request log\n' >&2
  if [[ -s "$request_log" ]]; then cat "$request_log" >&2; else printf '(no requests recorded)\n' >&2; fi
  printf '\n---- end failure dump for %s ----\n\n' "$stage" >&2
}

for group in "${STAGE_GROUPS[@]}"; do
  name="${group%%:*}"
  specs="${group#*:}"
  group_matches "$name" || continue
  ran_any=1

  data_dir="$RUN_DIR/$name/data"
  mkdir -p "$data_dir"
  mock_log="$LOG_DIR/$name.mock.log"
  request_log="$E2E_DIR/last-run-requests.log"
  rm -f "$request_log"

  say "stage group $name"
  ( cd "$E2E_DIR" && exec node mock-romm/server.mjs --port 0 ) >"$mock_log" 2>&1 &
  mock_pid=$!

  mock_url=""
  for _ in $(seq 1 100); do
    mock_url="$(sed -n 's/.*listening at \(http[^ ]*\).*/\1/p' "$mock_log" 2>/dev/null | head -n1)"
    [[ -n "$mock_url" ]] && break
    kill -0 "$mock_pid" 2>/dev/null || break
    sleep 0.1
  done
  if [[ -z "$mock_url" ]]; then
    printf 'e2e: mock server did not report a URL\n' >&2
    tail -n 40 "$mock_log" >&2
    kill "$mock_pid" 2>/dev/null
    failed_stages+=("$name (mock server)")
    continue
  fi
  printf 'e2e: mock RomM at %s, data dir %s\n' "$mock_url" "$data_dir"

  failed_stage=""
  failed_log=""
  for spec in $specs; do
    stage="$name/$(basename "$spec")"
    wdio_log="$LOG_DIR/$(basename "$spec").wdio.log"
    say "running $stage"
    (
      cd "$E2E_DIR" || exit 1
      export E2E_SPEC="$spec"
      export E2E_DATA_DIR="$data_dir"
      export E2E_MOCK_URL="$mock_url"
      export E2E_STAGE="$stage"
      export E2E_WDIO_LOG_DIR="$LOG_DIR/wdio-$(basename "$spec")"
      exec xvfb-run -a npx wdio run wdio.conf.ts
    ) 2>&1 | tee "$wdio_log"
    rc="${PIPESTATUS[0]}"
    if (( rc != 0 )); then
      failed_stage="$stage"
      failed_log="$wdio_log"
      failed_stages+=("$stage")
      break # later specs in a group depend on the earlier ones
    fi
  done

  # The mock writes its request log from close(), so it has to be stopped
  # before the dump — otherwise every failure report says "no requests".
  kill -TERM "$mock_pid" 2>/dev/null
  wait "$mock_pid" 2>/dev/null
  [[ -f "$request_log" ]] && cp "$request_log" "$LOG_DIR/$name.requests.log"
  [[ -n "$failed_stage" ]] && dump_failure "$failed_stage" "$failed_log" "$mock_log" "$request_log"
done

if (( ran_any == 0 )); then
  printf 'e2e: no stage group matched: %s\n' "${selected[*]}" >&2
  printf 'e2e: known groups:' >&2
  for group in "${STAGE_GROUPS[@]}"; do printf ' %s' "${group%%:*}" >&2; done
  printf '\n' >&2
  exit 2
fi

if (( ${#failed_stages[@]} > 0 )); then
  printf '\ne2e: FAILED stages: %s\n' "${failed_stages[*]}" >&2
  printf 'e2e: logs kept in %s\n' "$LOG_DIR" >&2
  exit 1
fi

say "e2e: all stages passed"
if [[ "${E2E_KEEP:-}" == "1" ]]; then
  printf 'e2e: run directory kept at %s\n' "$RUN_DIR"
else
  rm -rf "$RUN_DIR"
fi
exit 0

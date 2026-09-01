#!/usr/bin/env bash
#
# End-to-end test runner for the Rust rewrite.
#
# Runs WebdriverIO against a real, locally built Tauri binary that talks to a
# mock RomM server. Nothing here touches ~/.config/grid-launcher or the real
# OS keyring: every stage gets a temp GRID_LAUNCHER_DATA_DIR, and the whole
# run happens inside a private D-Bus session with its own gnome-keyring,
# XDG_DATA_HOME and XDG_RUNTIME_DIR, all under one temp run directory.
#
#   rewrite/scripts/e2e.sh              # build, then run every stage
#   rewrite/scripts/e2e.sh connect      # run only the named stage groups
#   E2E_SKIP_BUILD=1 rewrite/scripts/e2e.sh
#   E2E_KEEP=1 rewrite/scripts/e2e.sh   # keep the temp run directory
#
# Exit codes: 0 pass, 1 a stage group failed, 2 a prerequisite is missing or
# the binary cannot be trusted to be an e2e build.
#
# Behavior worth knowing before you read the code:
#
# * A failed stage group is RESET (fresh data dir, fresh mock server) and rerun
#   once before it counts as failed. Retrying at the group level rather than
#   per spec file is deliberate: a spec leaves the app and the data directory
#   mutated, so a bare spec-level retry re-runs against state the failed
#   attempt already changed and reports a misleading second error. Groups keep
#   their pairing semantics on retry — the whole group reruns from its first
#   spec.
# * A failing group does NOT stop the run; later groups still execute, and the
#   script exits nonzero at the end. This is a deliberate deviation from
#   "start strict": while specs are being written, seeing every group's result
#   in one pass is worth more than failing early.
# * Cleanup is a trap on EVERY exit path. D-Bus activates helpers (portals,
#   secret agents) inside the private bus that outlive `dbus-run-session`, so
#   the trap kills by run-directory marker rather than by process group.

set -uo pipefail

REWRITE_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
E2E_DIR="$REWRITE_DIR/e2e"
APP_DIR="$REWRITE_DIR/app"
APP_BINARY="$REWRITE_DIR/target/debug/app"
BUILD_STAMP="$REWRITE_DIR/target/debug/.e2e-build-stamp"

# A stage group = "name:spec [spec...]". Each group gets one fresh data
# directory and one mock server; each spec inside it is a separate `wdio run`
# (and therefore a separate app process) sharing that state. That is how the
# restore pair works: (a) connects, (b) relaunches and must find the session.
STAGE_GROUPS=(
  "connect:specs/connect.spec.ts"
  "connect-restore:specs/connect-restore-a.spec.ts specs/connect-restore-b.spec.ts"
  "library:specs/library.spec.ts"
  "install:specs/install-a.spec.ts specs/install-b.spec.ts"
  "downloads:specs/downloads.spec.ts"
)

# Run only the named groups by passing them as arguments, e.g.
# `rewrite/scripts/e2e.sh library downloads` — already supported by the
# positional-arg filter below (group_matches). There is no separate
# E2E_ONLY variable; the positional form is the single way to select groups.

# Extra attempts for a failed group, each from a clean slate.
GROUP_RETRIES=1

die() { printf 'e2e: %s\n' "$*" >&2; exit 1; }
say() { printf '\n== %s\n' "$*"; }

# Identifies the e2e build: the config files that must be paired with the
# `e2e` cargo feature, plus the build command itself. Deliberately NOT the git
# HEAD — committing a spec file must not force a pointless rebuild.
build_fingerprint() {
  cat "$APP_DIR/src-tauri/tauri.conf.json" "$APP_DIR/src-tauri/tauri.e2e.conf.json" 2>/dev/null |
    sha256sum | cut -d' ' -f1
}

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

  if [[ "${E2E_SKIP_BUILD:-}" == "1" ]]; then
    # A debug binary at this path proves nothing on its own: a plain
    # `cargo build -p app` writes the same file WITHOUT the e2e feature, and
    # the resulting run fails opaquely (the embedded WebDriver server never
    # comes up, so wdio just times out). Trust the stamp, not the binary.
    [[ -x "$APP_BINARY" ]] || die "no app binary at $APP_BINARY — run without E2E_SKIP_BUILD=1"
    if [[ ! -f "$BUILD_STAMP" ]]; then
      printf 'e2e: %s was not produced by this script (no build stamp).\n' "$APP_BINARY" >&2
      printf 'e2e: rerun without E2E_SKIP_BUILD=1.\n' >&2
      exit 2
    fi
    if [[ "$APP_BINARY" -nt "$BUILD_STAMP" ]]; then
      printf 'e2e: %s is newer than its e2e build stamp — it was rebuilt\n' "$APP_BINARY" >&2
      printf 'e2e: outside this script (probably without --features e2e).\n' >&2
      printf 'e2e: rerun without E2E_SKIP_BUILD=1.\n' >&2
      exit 2
    fi
    if ! grep -qx "fingerprint=$(build_fingerprint)" "$BUILD_STAMP" 2>/dev/null; then
      printf 'e2e: the Tauri config changed since the stamped e2e build.\n' >&2
      printf 'e2e: rerun without E2E_SKIP_BUILD=1.\n' >&2
      exit 2
    fi
  else
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
    rm -f "$BUILD_STAMP"
    ( cd "$APP_DIR" && VITE_E2E=1 npx tauri build \
        --debug --no-bundle --features e2e --config src-tauri/tauri.e2e.conf.json ) \
      || die "build failed"
    [[ -x "$APP_BINARY" ]] || die "the build reported success but $APP_BINARY is missing"
    {
      printf 'fingerprint=%s\n' "$(build_fingerprint)"
      printf 'git_head=%s\n' "$(git -C "$REWRITE_DIR" rev-parse HEAD 2>/dev/null || echo unknown)"
      printf 'built_at=%s\n' "$(date -Is)"
      printf 'command=VITE_E2E=1 npx tauri build --debug --no-bundle --features e2e --config src-tauri/tauri.e2e.conf.json\n'
    } > "$BUILD_STAMP"
  fi

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
  # gnome-keyring stores its keyring files under XDG_DATA_HOME, and its control
  # socket under XDG_RUNTIME_DIR. Redirecting BOTH into the run directory keeps
  # the throwaway "test"-password keyring out of ~/.local/share/keyrings and
  # keeps its socket out of /run/user/$UID, where a surviving daemon could
  # otherwise hijack the real session's unlocks.
  export XDG_DATA_HOME="$RUN_DIR/xdg-data"
  export XDG_RUNTIME_DIR="$RUN_DIR/xdg-runtime"
  mkdir -p "$XDG_DATA_HOME" "$XDG_RUNTIME_DIR"
  chmod 700 "$XDG_RUNTIME_DIR"
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
mock_pid=""

# GTK picks its backend by probing: with WAYLAND_DISPLAY set it connects to
# the session's real compositor through XDG_RUNTIME_DIR and ignores DISPLAY
# entirely — so the app would render on the developer's actual desktop instead
# of inside Xvfb, and it panics outright ("Failed to initialize GTK") once
# XDG_RUNTIME_DIR is redirected and that socket is gone. Pin the X11 backend
# and hide the Wayland socket so the run is genuinely headless.
unset WAYLAND_DISPLAY
export GDK_BACKEND=x11

# Never signal ourselves or anything we are running inside.
protected_pids() {
  local p=$$
  while [[ -n "$p" && "$p" != "0" && "$p" != "1" ]]; do
    printf '%s\n' "$p"
    p="$(ps -o ppid= -p "$p" 2>/dev/null | tr -d ' ')"
  done
}

# D-Bus activates helpers inside the private bus (xdg-desktop-portal-*,
# ksecretd, gnome-keyring-daemon) that are children of the bus, not of this
# script, and they survive `dbus-run-session` returning. They all inherit
# XDG_DATA_HOME/XDG_RUNTIME_DIR/E2E_RUN_DIR pointing at this run's unique
# mktemp directory, so that path is a reliable marker for "belongs to this
# run" — safer than a process-group kill, which misses them entirely.
kill_run_processes() {
  local sig="$1" p protected
  protected="$(protected_pids)"
  for p in /proc/[0-9]*; do
    p="${p#/proc/}"
    printf '%s\n' "$protected" | grep -qx "$p" && continue
    [[ -r "/proc/$p/environ" ]] || continue
    if grep -qzF -- "$RUN_DIR" "/proc/$p/environ" 2>/dev/null; then
      kill "-$sig" "$p" 2>/dev/null
    fi
  done
  return 0
}

stop_mock() {
  [[ -n "$mock_pid" ]] || return 0
  kill -TERM "$mock_pid" 2>/dev/null
  wait "$mock_pid" 2>/dev/null
  mock_pid=""
}

cleanup() {
  local rc=$?
  trap - EXIT INT TERM
  stop_mock
  kill_run_processes TERM
  sleep 1
  kill_run_processes KILL
  sleep 1
  if [[ "${E2E_KEEP:-}" == "1" ]]; then
    printf 'e2e: run directory kept at %s\n' "$RUN_DIR"
  elif (( rc != 0 )); then
    printf 'e2e: logs kept in %s\n' "$LOG_DIR" >&2
  else
    # Retry: a helper that ignored SIGTERM can recreate its data directory
    # after the first rm, which is how stale run dirs used to pile up.
    local i
    for i in 1 2 3; do
      rm -rf "$RUN_DIR"
      [[ -e "$RUN_DIR" ]] || break
      kill_run_processes KILL
      sleep 1
    done
    [[ -e "$RUN_DIR" ]] && printf 'e2e: warning: could not remove %s\n' "$RUN_DIR" >&2
  fi
  exit $rc
}
trap cleanup EXIT INT TERM

say "unlocking a throwaway gnome-keyring"
echo -n 'test' | gnome-keyring-daemon --unlock --components=secrets >/dev/null 2>&1 \
  || die "gnome-keyring-daemon --unlock failed"

selected=("$@")
failed_groups=()
ran_any=0

group_matches() {
  (( ${#selected[@]} == 0 )) && return 0
  local want
  for want in "${selected[@]}"; do [[ "$want" == "$1" ]] && return 0; done
  return 1
}

dump_failure() {
  local stage="$1" wdio_log="$2" mock_log="$3" request_log="$4" wdio_out_dir="$5"
  printf '\n---- FAILURE: stage %s ----\n' "$stage" >&2
  printf '\n-- wdio output (last 150 lines)\n' >&2
  tail -n 150 "$wdio_log" >&2 2>/dev/null || printf '(no wdio log)\n' >&2
  # The app's own stdout/stderr, forwarded by the tauri service's
  # captureBackendLogs/captureFrontendLogs and tagged [Tauri:Backend:N] /
  # [Tauri:Frontend:N]. Pulled out on their own because the same files also
  # hold thousands of lines of WebDriver protocol chatter that would bury them.
  printf '\n-- app stdout/stderr forwarded by the tauri service\n' >&2
  if [[ -d "$wdio_out_dir" ]] && compgen -G "$wdio_out_dir/*.log" >/dev/null; then
    if ! grep -hE '\[Tauri:(Backend|Frontend):[0-9]+\]' "$wdio_out_dir"/*.log >&2; then
      printf '(the app emitted nothing — it may have failed before starting)\n' >&2
    fi
    printf '\n-- wdio service/driver logs (tail of each file in %s)\n' "$wdio_out_dir" >&2
    local f
    for f in "$wdio_out_dir"/*.log; do
      printf '\n[%s]\n' "$(basename "$f")" >&2
      tail -n 40 "$f" >&2
    done
  else
    printf '(no wdio output-dir logs at %s)\n' "$wdio_out_dir" >&2
  fi
  printf '\n-- mock server stderr/stdout (last 40 lines)\n' >&2
  tail -n 40 "$mock_log" >&2 2>/dev/null || printf '(no mock log)\n' >&2
  printf '\n-- mock request log\n' >&2
  if [[ -s "$request_log" ]]; then cat "$request_log" >&2; else printf '(no requests recorded)\n' >&2; fi
  printf '\n---- end failure dump for %s ----\n\n' "$stage" >&2
}

# Extra CLI args for this group's mock server instance. Only `downloads`
# needs a throttled content endpoint (100ms per ~20KB chunk — see
# mock-romm/server.mjs — comfortable against the "Big Arcade Game" fixture
# for a real, cancellable in-flight download); every other group gets the
# plain, unthrottled server so its installs stay fast.
mock_args_for_group() {
  case "$1" in
    downloads) printf -- '--throttle-ms 100' ;;
    *) printf '' ;;
  esac
}

# Runs every spec of one group against a freshly created data dir and mock.
# Sets attempt_failed_stage / attempt_failed_log / attempt_out_dir on failure.
run_group_attempt() {
  local name="$1" specs="$2" attempt="$3"
  local data_dir="$RUN_DIR/$name/attempt-$attempt/data"
  attempt_failed_stage=""
  attempt_failed_log=""
  attempt_out_dir=""
  mkdir -p "$data_dir"

  attempt_mock_log="$LOG_DIR/$name-attempt-$attempt.mock.log"
  attempt_request_log="$E2E_DIR/last-run-requests.log"
  rm -f "$attempt_request_log"

  local mock_args
  mock_args="$(mock_args_for_group "$name")"
  # shellcheck disable=SC2086 # mock_args is a small, script-controlled word list
  ( cd "$E2E_DIR" && exec node mock-romm/server.mjs --port 0 $mock_args ) >"$attempt_mock_log" 2>&1 &
  mock_pid=$!

  local mock_url="" _
  for _ in $(seq 1 100); do
    mock_url="$(sed -n 's/.*listening at \(http[^ ]*\).*/\1/p' "$attempt_mock_log" 2>/dev/null | head -n1)"
    [[ -n "$mock_url" ]] && break
    kill -0 "$mock_pid" 2>/dev/null || break
    sleep 0.1
  done
  if [[ -z "$mock_url" ]]; then
    printf 'e2e: mock server did not report a URL\n' >&2
    tail -n 40 "$attempt_mock_log" >&2
    stop_mock
    attempt_failed_stage="$name (mock server)"
    return 1
  fi
  printf 'e2e: mock RomM at %s, data dir %s\n' "$mock_url" "$data_dir"

  local spec stage wdio_log out_dir rc
  for spec in $specs; do
    stage="$name/$(basename "$spec")"
    wdio_log="$LOG_DIR/$name-attempt-$attempt-$(basename "$spec").wdio.log"
    out_dir="$LOG_DIR/$name-attempt-$attempt-$(basename "$spec").wdio.d"
    say "running $stage (attempt $attempt)"
    (
      cd "$E2E_DIR" || exit 1
      export E2E_SPEC="$spec"
      export E2E_DATA_DIR="$data_dir"
      export E2E_MOCK_URL="$mock_url"
      export E2E_STAGE="$stage"
      export E2E_WDIO_LOG_DIR="$out_dir"
      exec xvfb-run -a npx wdio run wdio.conf.ts
    ) 2>&1 | tee "$wdio_log"
    rc="${PIPESTATUS[0]}"
    if (( rc != 0 )); then
      attempt_failed_stage="$stage"
      attempt_failed_log="$wdio_log"
      attempt_out_dir="$out_dir"
      break # later specs in a group depend on the earlier ones
    fi
  done

  # The mock writes its request log from close(), so it has to be stopped
  # before any dump — otherwise every failure report says "no requests".
  stop_mock
  [[ -f "$attempt_request_log" ]] && cp "$attempt_request_log" "$LOG_DIR/$name-attempt-$attempt.requests.log"
  [[ -z "$attempt_failed_stage" ]]
}

for group in "${STAGE_GROUPS[@]}"; do
  name="${group%%:*}"
  specs="${group#*:}"
  group_matches "$name" || continue
  ran_any=1

  say "stage group $name"
  attempt=1
  while true; do
    if run_group_attempt "$name" "$specs" "$attempt"; then
      break
    fi
    if (( attempt <= GROUP_RETRIES )); then
      printf '\ne2e: stage group %s failed at %s — resetting and retrying once\n' \
        "$name" "$attempt_failed_stage" >&2
      attempt=$(( attempt + 1 ))
      continue
    fi
    dump_failure "$attempt_failed_stage" "$attempt_failed_log" "$attempt_mock_log" \
      "$attempt_request_log" "$attempt_out_dir"
    failed_groups+=("$name (at $attempt_failed_stage)")
    break
  done
done

if (( ran_any == 0 )); then
  printf 'e2e: no stage group matched: %s\n' "${selected[*]}" >&2
  printf 'e2e: known groups:' >&2
  for group in "${STAGE_GROUPS[@]}"; do printf ' %s' "${group%%:*}" >&2; done
  printf '\n' >&2
  exit 2
fi

if (( ${#failed_groups[@]} > 0 )); then
  printf '\ne2e: FAILED stage groups: %s\n' "${failed_groups[*]}" >&2
  exit 1
fi

say "e2e: all stages passed"
exit 0

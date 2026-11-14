#!/usr/bin/env bash
# Podman build container for Linux builds, the gate and the end-to-end suite, with a
# CPU and memory cap so the desktop stays usable. See BUILD.md "Build container".
#
#   scripts/ctr.sh build              build or rebuild both images (fresh: newer base, no cache)
#   scripts/ctr.sh run <cmd...>       run a command in a throwaway container, cwd = repo
#   scripts/ctr.sh shell              interactive shell in the same setup
#   scripts/ctr.sh gate               run the gate (CLAUDE.md / BUILD.md), in its order
#   scripts/ctr.sh wincheck           run the check-windows job's two clippy commands for
#                                     --target x86_64-pc-windows-gnu (lint only, no link)
#   scripts/ctr.sh e2e [groups...]    run scripts/e2e.sh (env E2E_* is passed through)
#   scripts/ctr.sh appimage [ver]     build the AppImage exactly as the release job does
#                                     (default version 0.0.0-dev) into ctr-out/
#   scripts/ctr.sh packages [ver]     build the release job's .deb and .rpm into ctr-out/
#   scripts/ctr.sh clean [--all]      remove this checkout's target and node_modules volumes
#                                     (--all: also the shared cargo, npm and tool cache volumes)
#
# Two images, each matching the CI job it stands in for (.github/workflows/build.yml):
# Ubuntu 24.04 for the gate, e2e, run and shell (the check and e2e jobs), and Debian 13
# for appimage (the AppImage job), so a local AppImage bundles the same WebKitGTK as a
# released one. Each has its own target/ volume: builds against two
# glibc versions must not share one.
#
# Only the repo is mounted (read-write). No $HOME, no display or D-Bus socket, no keyring:
# the e2e runner starts its own Xvfb, D-Bus session and gnome-keyring inside the container.
# target/ and the node_modules folders are podman volumes, so container output never
# mixes with a host build in the same checkout.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
repo=$(pwd -P)

CHECK_TAG=24.04
CHECK_BASE=docker.io/library/ubuntu:24.04
# Debian 13 for the AppImage: the oldest glibc (2.41) that ships WebKitGTK 2.54, the
# first WebKitGTK that renders on the GPU with NVIDIA on Wayland (2026-10-30).
RELEASE_TAG=debian13
RELEASE_BASE=docker.io/library/debian:13
IMAGE_REPO=localhost/grid-launcher-build
IMAGE=$IMAGE_REPO:$CHECK_TAG
# Owner limit (2026-10-20): at most 4 CPUs and 8 GB per container.
# CTR_CPUS / CTR_MEMORY (in MB or with a g/m suffix) may lower the limit, never raise it.
MAX_CPUS=4
MAX_MEMORY_MB=8192
to_mb() { case "$1" in *g|*G) echo $(( ${1%[gG]} * 1024 )) ;; *m|*M) echo "${1%[mM]}" ;; *) echo "$1" ;; esac; }
CPUS=$(awk -v a="${CTR_CPUS:-$MAX_CPUS}" -v m="$MAX_CPUS" 'BEGIN { print (a + 0 > 0 && a + 0 < m) ? a + 0 : m }')
MEMORY_MB=$(to_mb "${CTR_MEMORY:-${MAX_MEMORY_MB}m}")
[ "$MEMORY_MB" -gt 0 ] 2>/dev/null && [ "$MEMORY_MB" -le "$MAX_MEMORY_MB" ] || MEMORY_MB=$MAX_MEMORY_MB
MEMORY=${MEMORY_MB}m
JOBS=$(awk -v c="$CPUS" 'BEGIN { j = int(c); print (j < 1) ? 1 : j }')
# Owner limit (2026-11-14): at most two containers at once across every project that uses
# this pattern (grid-launcher and free-budget share the slots), each within the cap above.
# A second run of the same checkout waits for the first, because they share its volumes.
SLOT_DIR=${XDG_RUNTIME_DIR:-/tmp}/ctr-slots
CHECKOUT_LOCK=${XDG_RUNTIME_DIR:-/tmp}/grid-launcher-ctr-$(printf %s "$repo" | sha256sum | cut -c1-8).lock
locked() {
  mkdir -p "$SLOT_DIR"
  exec 8>"$CHECKOUT_LOCK"
  if ! flock -n 8; then
    echo "ctr.sh: this checkout already has a container running; waiting for it to finish" >&2
    flock 8
  fi
  local waited=0 slot
  while :; do
    for slot in 1 2; do
      exec 9>"$SLOT_DIR/slot-$slot.lock"
      if flock -n 9; then
        local rc=0
        "$@" || rc=$?
        exec 9>&- 8>&-
        return "$rc"
      fi
      exec 9>&-
    done
    [ "$waited" = 1 ] || echo "ctr.sh: two containers are already running; waiting for a free slot" >&2
    waited=1
    sleep 3
  done
}
# target and node_modules are per checkout (a short hash of the repo path), so git worktrees
# can run containers without sharing build output. CTR_VOL_PREFIX overrides the prefix.
# The cargo registry/git and npm cache volumes are shared by every checkout.
CACHE_PREFIX=grid-launcher
VOL_PREFIX=${CTR_VOL_PREFIX:-grid-launcher-$(printf %s "$repo" | sha256sum | cut -c1-8)}
TARGET_VOLUME=$VOL_PREFIX-target
# Extra podman arguments for one command (appimage adds its tool cache).
EXTRA_ARGS=()

# A newer base image and no layer cache, so a rebuild is also a security refresh
# (the image's apt packages and the latest Node/Rust releases).
build_image() {
  podman build --pull=newer --no-cache --build-arg "BASE_IMAGE=$2" -t "$IMAGE_REPO:$1" \
    -f scripts/container/Containerfile scripts/container
}
build() { build_image "$CHECK_TAG" "$CHECK_BASE" && build_image "$RELEASE_TAG" "$RELEASE_BASE"; }

podman_args() {
  # Create the mount points as the host user, so they are not left owned by a subuid.
  mkdir -p target app/node_modules e2e/node_modules
  args=(--rm --userns=keep-id --cpus="$CPUS" --memory="$MEMORY" --memory-swap="$MEMORY"
    --cpu-shares=128 --pids-limit=8192
    --security-opt label=disable --security-opt no-new-privileges --cap-drop=all
    # WebKitGTK (the e2e app) needs more shared memory than podman's 64 MB default.
    --shm-size=1g
    # RLIMIT_CORE=1 keeps crashing test processes out of the host's systemd-coredump.
    --ulimit core=1:1
    -v "$repo:/work:rw"
    -v "$TARGET_VOLUME:/work/target"
    -v "$VOL_PREFIX-app-node_modules:/work/app/node_modules"
    -v "$VOL_PREFIX-e2e-node_modules:/work/e2e/node_modules"
    -v "$CACHE_PREFIX-cargo-registry:/opt/cargo/registry"
    -v "$CACHE_PREFIX-cargo-git:/opt/cargo/git"
    -v "$CACHE_PREFIX-npm:/cache/npm"
    -e npm_config_cache=/cache/npm -e CARGO_TERM_COLOR=always
    -e "CARGO_BUILD_JOBS=$JOBS" -e "RUST_TEST_THREADS=$JOBS"
    -w /work)
  args+=("${EXTRA_ARGS[@]}")
  # Pass the e2e runner's switches through (E2E_SKIP_BUILD, E2E_KEEP, ...).
  local v
  while IFS= read -r v; do args+=(-e "$v"); done < <(compgen -e | grep '^E2E_' || true)
  # In a git worktree, /work/.git is a file that points into the main checkout's .git
  # folder. Mount that folder read-only at the same path, so git works in the container.
  local common
  common=$(git -C "$repo" rev-parse --path-format=absolute --git-common-dir 2>/dev/null || true)
  if [ -n "$common" ] && [ "$common" != "$repo/.git" ]; then
    args+=(-v "$common:$common:ro")
  fi
}

# The base for an image tag: the release tag's, else the check tag's.
base_for() { [ "$1" = "$RELEASE_TAG" ] && echo "$RELEASE_BASE" || echo "$CHECK_BASE"; }
ensure_image() { podman image exists "$IMAGE" || locked build_image "${IMAGE##*:}" "$(base_for "${IMAGE##*:}")"; }

# Low priority inside the container on top of the cgroup cap.
in_ctr() { ensure_image; podman_args; locked podman run "${args[@]}" "$IMAGE" nice -n 19 ionice -c3 "$@"; }

GATE='set -euo pipefail
step() { printf "\n== gate: %s\n" "$*"; }
step secret hygiene; scripts/check_secret_hygiene.sh
step cargo fmt;      cargo fmt --check
step frontend;       (cd app && npm ci && npm run check && npm run build)
step clippy;         cargo clippy --workspace --all-targets -- -D warnings
step clippy e2e;     cargo clippy -p app --all-targets --features e2e -- -D warnings
step cargo test;     cargo test --workspace
step npm test;       (cd app && npm test)
printf "\n== gate: PASS\n"'

# The check-windows job's clippy step (build.yml), cross-checked for the GNU Windows
# target. clippy type-checks and lints every cfg(windows) path but never links, so
# unrar_sys's C++ goes to the image's stub compiler (see the Containerfile); aws-lc-sys
# builds with the real MinGW gcc. app/dist is a tmpfs with a placeholder index.html: the
# Tauri context macro only needs the frontend folder to exist, and the host's dist/ is
# never touched.
WINCHECK='set -euo pipefail
export CXX_x86_64_pc_windows_gnu=/opt/wincheck/cxx-stub
export AR_x86_64_pc_windows_gnu=/opt/wincheck/ar-stub
export CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc
echo "<!doctype html><title>wincheck</title>" > app/dist/index.html
step() { printf "\n== wincheck: %s\n" "$*"; }
step clippy;     cargo clippy --target x86_64-pc-windows-gnu --workspace --all-targets -- -D warnings
step clippy e2e; cargo clippy --target x86_64-pc-windows-gnu -p app --all-targets --features e2e -- -D warnings
printf "\n== wincheck: PASS\n"'

case "${1:-}" in
  build) locked build ;;
  run)
    shift; [ $# -gt 0 ] || { echo "usage: $0 run <cmd...>" >&2; exit 2; }
    in_ctr "$@" ;;
  shell)
    ensure_image; podman_args
    locked podman run -it "${args[@]}" "$IMAGE" nice -n 19 bash ;;
  gate) in_ctr bash -c "$GATE" ;;
  wincheck)
    # Images built before wincheck existed lack MinGW and the stubs; rebuild them first.
    if podman image exists "$IMAGE" \
        && [ "$(podman image inspect --format '{{index .Labels "org.grid-launcher.wincheck"}}' "$IMAGE")" != 1 ]; then
      echo "ctr.sh: $IMAGE predates wincheck; rebuilding the images (scripts/ctr.sh build)" >&2
      locked build
    fi
    # Its own target volume: Windows artifacts never mix with the Linux gate's.
    TARGET_VOLUME=$VOL_PREFIX-win-target
    mkdir -p app/dist
    EXTRA_ARGS=(--mount type=tmpfs,destination=/work/app/dist,tmpfs-mode=1777)
    in_ctr bash -c "$WINCHECK" ;;
  e2e)
    shift
    # e2e.sh installs dependencies only when a node_modules folder is missing, but here
    # each one is a volume mount point and always exists; install into an empty volume.
    in_ctr bash -c 'set -euo pipefail
      for d in app e2e; do
        [ -e "$d/node_modules/.package-lock.json" ] || (cd "$d" && npm ci --no-audit --no-fund)
      done
      exec scripts/e2e.sh "$@"' _ "$@" ;;
  appimage)
    version=${2:-0.0.0-dev}
    printf %s "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$' \
      || { echo "ctr.sh: version '$version' is not semver" >&2; exit 2; }
    mkdir -p ctr-out
    IMAGE=$IMAGE_REPO:$RELEASE_TAG
    TARGET_VOLUME=$VOL_PREFIX-release-target
    # Tauri downloads its bundling tools (linuxdeploy, its plugins, AppRun; about 40 MB)
    # into $XDG_CACHE_HOME/tauri on every run of a fresh container. Keep them in a shared
    # volume so a GitHub outage only breaks the first build. The AppImage runtime (about
    # 1 MB) that linuxdeploy's AppImage plugin and appimagetool would each download is
    # kept there too, in appimage-runtime/. It is refreshed once it is 30 days old (the
    # release job always takes the latest); a failed refresh keeps the cached copy. Only
    # those two folders are kept: the script below empties everything else in it.
    EXTRA_ARGS=(-v "$CACHE_PREFIX-xdg-cache:/cache/xdg" -e XDG_CACHE_HOME=/cache/xdg)
    # The release job's steps (build-linux in build.yml): build, then repack with
    # appimagetool to embed the update information. Same tool, version and digest as
    # there; keep the two in step.
    in_ctr bash -c 'set -euo pipefail
      version=$1
      trap '\''find /cache/xdg -mindepth 1 -maxdepth 1 ! -name tauri ! -name appimage-runtime -exec rm -rf {} +'\'' EXIT
      runtime=/cache/xdg/appimage-runtime/runtime-x86_64
      if [ ! -s "$runtime" ] || [ -n "$(find "$runtime" -mtime +30)" ]; then
        mkdir -p "${runtime%/*}"
        if curl -fsSL --retry 6 --retry-delay 10 --retry-all-errors -o "$runtime.part" \
            https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-x86_64 \
            && [ "$(head -c 4 "$runtime.part" | od -An -tx1 | tr -d " ")" = 7f454c46 ]; then
          mv "$runtime.part" "$runtime"
        else
          rm -f "$runtime.part"
          [ -s "$runtime" ] || { echo "ctr.sh: could not download the AppImage runtime" >&2; exit 1; }
          echo "ctr.sh: AppImage runtime refresh failed; using the cached copy" >&2
        fi
      fi
      export LDAI_RUNTIME_FILE=$runtime
      (cd app && npm ci --no-audit --no-fund && npx tauri build --bundles appimage --config "{\"version\":\"$version\"}")
      tool=target/tools/appimagetool-1.9.1
      if [ ! -x "$tool" ]; then
        mkdir -p target/tools
        curl -fsSL -o "$tool.part" \
          https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage
        echo "ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0  $tool.part" | sha256sum -c -
        chmod +x "$tool.part" && mv "$tool.part" "$tool"
      fi
      out=grid-launcher-$version-x86_64.AppImage
      work=$(mktemp -d)
      (cd "$work" && "/work/target/release/bundle/appimage/GRID Launcher_${version}_amd64.AppImage" --appimage-extract >/dev/null)
      rm -f "ctr-out/$out" "ctr-out/$out.zsync"
      (cd ctr-out && "/work/$tool" --appimage-extract-and-run --runtime-file "$runtime" \
        -u "gh-releases-zsync|Sixdd6|grid-launcher|latest|grid-launcher-*-x86_64.AppImage.zsync" \
        "$work/squashfs-root" "$out")
      rm -rf "$work"
      ls -l "ctr-out/$out" "ctr-out/$out.zsync"' _ "$version" ;;
  packages)
    # The release job's .deb and .rpm (build-linux-packages in build.yml): built in the
    # check image (Ubuntu 24.04, glibc 2.39) against the system WebKitGTK, which the
    # packages depend on instead of bundling. The installed binary is
    # /usr/bin/grid-launcher (mainBinaryName); an RPM version cannot hold "-", so its
    # pre-release separator becomes "+".
    version=${2:-0.0.0-dev}
    printf %s "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$' \
      || { echo "ctr.sh: version '$version' is not semver" >&2; exit 2; }
    mkdir -p ctr-out
    in_ctr bash -c 'set -euo pipefail
      version=$1
      rpm_version=${version//-/+}
      cd app && npm ci --no-audit --no-fund
      npx tauri build --bundles deb --config "{\"version\":\"$version\",\"mainBinaryName\":\"grid-launcher\"}"
      npx tauri build --bundles rpm --config "{\"version\":\"$rpm_version\",\"mainBinaryName\":\"grid-launcher\"}"
      cd ..
      cp "target/release/bundle/deb/GRID Launcher_${version}_amd64.deb" "ctr-out/grid-launcher_${version}_amd64.deb"
      cp "target/release/bundle/rpm/GRID Launcher-${rpm_version}-1.x86_64.rpm" "ctr-out/grid-launcher-${rpm_version}-1.x86_64.rpm"
      ls -l ctr-out/grid-launcher_*.deb ctr-out/grid-launcher-*.rpm' _ "$version" ;;
  clean)
    for v in target release-target win-target app-node_modules e2e-node_modules; do podman volume rm -f "$VOL_PREFIX-$v" >/dev/null; done
    if [ "${2:-}" = "--all" ]; then
      for v in cargo-registry cargo-git npm xdg-cache; do podman volume rm -f "$CACHE_PREFIX-$v" >/dev/null; done
    fi
    echo "volumes removed" ;;
  *) echo "usage: $0 {build|run <cmd...>|shell|gate|wincheck|e2e [groups...]|appimage [version]|packages [version]|clean [--all]}" >&2; exit 2 ;;
esac

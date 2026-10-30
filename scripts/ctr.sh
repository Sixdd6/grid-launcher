#!/usr/bin/env bash
# Podman build container for Linux builds, the gate and the end-to-end suite, with a
# CPU and memory cap so the desktop stays usable. See BUILD.md "Build container".
#
#   scripts/ctr.sh build              build or rebuild both images (fresh: newer base, no cache)
#   scripts/ctr.sh run <cmd...>       run a command in a throwaway container, cwd = repo
#   scripts/ctr.sh shell              interactive shell in the same setup
#   scripts/ctr.sh gate               run the gate (CLAUDE.md / BUILD.md), in its order
#   scripts/ctr.sh e2e [groups...]    run scripts/e2e.sh (env E2E_* is passed through)
#   scripts/ctr.sh appimage [ver]     build the AppImage exactly as the release job does
#                                     (default version 0.0.0-dev) into ctr-out/
#   scripts/ctr.sh packages [ver]     build the release job's .deb and .rpm into ctr-out/
#   scripts/ctr.sh clean [--all]      remove this checkout's target and node_modules volumes
#                                     (--all: also the shared cargo and npm cache volumes)
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
# Owner limit (2026-10-20): at most 4 CPUs and 8 GB for grid-launcher containers, in total.
# CTR_CPUS / CTR_MEMORY (in MB or with a g/m suffix) may lower the limit, never raise it.
MAX_CPUS=4
MAX_MEMORY_MB=8192
to_mb() { case "$1" in *g|*G) echo $(( ${1%[gG]} * 1024 )) ;; *m|*M) echo "${1%[mM]}" ;; *) echo "$1" ;; esac; }
CPUS=$(awk -v a="${CTR_CPUS:-$MAX_CPUS}" -v m="$MAX_CPUS" 'BEGIN { print (a + 0 > 0 && a + 0 < m) ? a + 0 : m }')
MEMORY_MB=$(to_mb "${CTR_MEMORY:-${MAX_MEMORY_MB}m}")
[ "$MEMORY_MB" -gt 0 ] 2>/dev/null && [ "$MEMORY_MB" -le "$MAX_MEMORY_MB" ] || MEMORY_MB=$MAX_MEMORY_MB
MEMORY=${MEMORY_MB}m
JOBS=$(awk -v c="$CPUS" 'BEGIN { j = int(c); print (j < 1) ? 1 : j }')
# One grid-launcher container at a time across every checkout and worktree, so the total
# stays within the limit. Parallel agents queue here instead of sharing the machine.
LOCK=${XDG_RUNTIME_DIR:-/tmp}/grid-launcher-ctr.lock
locked() {
  exec 9>"$LOCK"
  if ! flock -n 9; then
    echo "ctr.sh: another grid-launcher container is running; waiting for it to finish" >&2
    flock 9
  fi
  "$@"
}
# target and node_modules are per checkout (a short hash of the repo path), so git worktrees
# can run containers without sharing build output. CTR_VOL_PREFIX overrides the prefix.
# The cargo registry/git and npm cache volumes are shared by every checkout.
CACHE_PREFIX=grid-launcher
VOL_PREFIX=${CTR_VOL_PREFIX:-grid-launcher-$(printf %s "$repo" | sha256sum | cut -c1-8)}
TARGET_VOLUME=$VOL_PREFIX-target

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
step frontend;       (cd app && npm ci && npx svelte-check && npm run build)
step clippy;         cargo clippy --workspace --all-targets -- -D warnings
step clippy e2e;     cargo clippy -p app --all-targets --features e2e -- -D warnings
step cargo test;     cargo test --workspace
step npm test;       (cd app && npm test)
printf "\n== gate: PASS\n"'

case "${1:-}" in
  build) locked build ;;
  run)
    shift; [ $# -gt 0 ] || { echo "usage: $0 run <cmd...>" >&2; exit 2; }
    in_ctr "$@" ;;
  shell)
    ensure_image; podman_args
    locked podman run -it "${args[@]}" "$IMAGE" nice -n 19 bash ;;
  gate) in_ctr bash -c "$GATE" ;;
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
    # The release job's steps (build-linux in build.yml): build, then repack with
    # appimagetool to embed the update information. Same tool, version and digest as
    # there; keep the two in step.
    in_ctr bash -c 'set -euo pipefail
      version=$1
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
      (cd ctr-out && "/work/$tool" --appimage-extract-and-run \
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
    for v in target release-target app-node_modules e2e-node_modules; do podman volume rm -f "$VOL_PREFIX-$v" >/dev/null; done
    if [ "${2:-}" = "--all" ]; then
      for v in cargo-registry cargo-git npm; do podman volume rm -f "$CACHE_PREFIX-$v" >/dev/null; done
    fi
    echo "volumes removed" ;;
  *) echo "usage: $0 {build|run <cmd...>|shell|gate|e2e [groups...]|appimage [version]|packages [version]|clean [--all]}" >&2; exit 2 ;;
esac

# Plan: GRID-managed umu-run (umu-launcher)

User report (2026-10-20): launching Tamriel Rebuilt through Proton failed with "umu-run is not installed". GRID only looks up `umu-run` on PATH (`launch/native.rs:140-150`) and never installs it.

## User decisions (final)
1. One shared GRID-managed copy in `<compat::managed_root()>/umu-launcher/`, recorded in config with its release tag.
2. Installing a Proton compat tool while no usable umu-run exists (none on PATH, no managed copy) also queues umu-launcher as its own Downloads row. A umu failure never fails the Proton install.
3. Launch lookup: system `umu-run` on PATH first, then the managed copy. The managed copy needs python3 >= 3.10; a missing or old python3 gives a clear error.
4. Both, for existing setups: (a) Emulators › Compat tools shows a umu-run status line (system <path> / GRID-managed <tag> / not installed) with Install / Reinstall; (b) a Proton launch with no umu-run fails with a structured error (`code: "umu_missing"`) and the launch error offers "Install umu-launcher".
5. The download is checked against the GitHub API asset `digest` (sha256). A mismatch is a hard failure (the archive is deleted so Retry downloads again). A missing digest is accepted with a warning log.

Orchestrator choices from the planner's open questions: Proton stays first in the queue; no automatic updates (Reinstall re-resolves `latest`); a static note about the ~200 MB Steam Linux Runtime the first Proton launch downloads; the python3 probe runs on every managed-copy launch (not cached); reuse Downloads kind `"compat_tool"`; hide the umu line on Windows like the pane.

## Research (upstream, 2026-10-20)
- Latest release `1.4.4` (no `v`). Asset `umu-launcher-<tag>-zipapp.tar` (plain tar, ~430 KB): `umu/umu-run` (Python zipapp, `#!/usr/bin/env python3`, needs python3 >= 3.10, pure Python) and `umu/umu_run.py` (symlink). Other assets are .rpm/.deb. No checksum files; the releases API carries `"digest": "sha256:<hex>"`.
- No self-update. Heroic/Lutris download the same zipapp and prefer a system umu-run.
- First run downloads the Steam Linux Runtime (~195 MB) into `~/.local/share/umu`; needs network. GRID always sets PROTONPATH, so umu never fetches its own Proton.
- GPL-3.0; downloading the unmodified upstream asset at run time is fine.

## Design
- **Config:** `UmuLauncherInstall { path, release_tag }`, `Config.umu_launcher: Option<_>` (a TOML table, after the scalar keys).
- **Forge:** `ResolvedDownload.digest: String` (raw asset `digest`, `""` when absent); `library::download::verify_sha256_digest(path, digest) -> Result<DigestOutcome { Verified, Unavailable }, LibraryError>`.
- **`launch/umu.rs`** (pure, injectable): `UMU_SOURCE_ID = "Open-Wine-Components/umu-launcher"`, `UMU_TITLE`, `default_source()` (github-release, `latest`, `umu-launcher-*-zipapp.tar`), `install_dir`, `find_umu_run`, `managed_umu_run`, `locate(which, managed) -> Option<(PathBuf, UmuSource)>`, `status(...) -> UmuRunStatus { source: system|managed|missing, path, release_tag, source_id }`, `parse_python_version`, `python_requirement`, `probe_python` (looks up python3 in `clean_env()`'s PATH). `library/extract.rs` gains `which_in(path_var, name)`.
- **InstallService:** `EmulatorJob.compat_tool: bool` becomes `target: JobTarget { Emulator, CompatTool, UmuLauncher }`; `install_umu_launcher()` (Linux only) admits `JobKey::Emulator(UMU_SOURCE_ID)`, kind `compat_tool`; `set_umu_source` is the test seam. Finalize: digest check, extract, `find_umu_run`, `make_executable`, write the config record, delete the archive, fire `compat_tools_hook`. `install_compat_tool` auto-queues umu for Proton profiles when `locate` finds none. Retry goes back through `install_umu_launcher`.
- **Launch:** `NativeCommandError { Message(String), UmuMissing }` from `build_native_command`; `LaunchError::UmuMissing`; `LaunchService::launch` resolves umu-run via `locate` and probes python3 for the managed copy.
- **Tauri:** `install_umu_launcher` command (`ensure_library_idle`, on the library-move guard list); `CompatToolsDto.umu_run`; `launch_game` returns `LaunchCommandError { code, message }` (`"umu_missing"` or `""`).
- **Frontend:** `launchError.ts` (`LaunchFailure`, `toLaunchFailure`, `isUmuMissing`) wraps `api.launchGame`; `api.installUmuLauncher`; `compatTools.ts` gets `umuStatusLabel`, `umuActionLabel`, `umuInstallBusy`; the CompatTools.svelte status line (`compat-umu-status`, `compat-umu-install`); `UmuInstallOffer.svelte` under the launch errors in Library, Server and Details.

## Steps (tests first in each; gate; `git commit --only`)
1. Config record. 2. Digest plumbing. 3. `launch/umu.rs` + `which_in`. 4. Install pipeline (new integration test binary `tests/umu_launcher_install.rs`: install, bad digest, no digest, Proton auto-queue, no auto-queue with a managed copy, umu 404 does not fail Proton, retry). 5. Launch path. 6. Tauri layer. 7. Frontend. 8. E2E (mock-forge umu-launcher routes with a computed digest; the emulator-catalog group installs it from the pane; optionally a native-group check for the offer). 9. SPEC.md (Compat tools pane, native launch) and ARCHITECTURE.md (`launch::umu`). 10. Full gate + scoped e2e.

## Risks
- The first Proton launch looks slow while umu downloads the runtime (no progress shown). Follow-up: a "Preparing Proton runtime…" state.
- An e2e host that already has umu-run on PATH reports "system"; the install case skips with a log line.
- Uninstalling the managed copy is out of scope.

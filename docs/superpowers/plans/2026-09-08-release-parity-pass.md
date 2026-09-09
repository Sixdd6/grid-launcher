# Release Parity Pass — Implementation Plan (2026-09-08)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. One implementer per wave-slot; tasks marked as conflicting must never run concurrently.

**Goal:** Close the remaining behaviour gaps between the Python app and the Rust/Tauri/Svelte rewrite that the user ruled must ship now: Linux platforms as native, emulator "Update from Source", the debug-prints toggle, Eden advisory notes, archive-backed manual emulator entries, launch-time RetroArch sync, the standalone-emulator early-exit warning, toast a11y, picker failure feedback, and the Windows Documents resolver — then run the full gate and push.

**Architecture:** Pure rules live in `crates/grid-core` (platform predicates, launch command building, install pipeline, forge resolution, cloud path resolution). `app/src-tauri` owns Tauri commands, AppState services and hooks into grid-core (`set_game_finalized_hook`, `set_emulator_installed_hook`, `set_session_finished_hook` — copy that pattern for anything that needs keyring or UI access from inside a grid-core flow). `app/src` is Svelte 5 + TS; pure helpers sit in `app/src/lib/**/*.ts` with vitest tests, components get SSR render tests (`render` from `svelte/server`, see `app/src/lib/Icon.svelte.test.ts`). E2E specs live in `e2e/specs` and are grouped by `scripts/e2e.sh`.

**Tech Stack:** Rust (tokio, wiremock, tempfile, tracing-subscriber 0.3 `env-filter`), Tauri 2, Svelte 5, TypeScript, vitest, WebdriverIO.

**Reference:** Python app at the repo root is the behaviour oracle; `docs/porting/*.md` are the specs. TV mode is OUT of scope (deferred by ruling).

**User rulings (2026-09-08, final):**
- (a) Platforms whose display label starts with `linux` are native everywhere, exactly like `windows`: install block reason, launch, cloud save scope and block reasons all use ONE windows-or-linux predicate. A Linux-platform game launches its own executable directly, with no compat tool and no prefix; a Windows-platform game keeps the compat tool.
- (b) `.icon-btn:hover, .icon-btn:focus-visible` uses `background: var(--surface)` and the glyph turns `var(--primary)`.
- (c) The rating star stays `--primary` (no change).
- Emulator "Update from Source" ships now; the deferred parity items ship now.

## Global Constraints

- Token secrecy: nothing secret in logs, errors, IPC or console output. The debug-prints toggle (Task 4) raises the tracing level only; it must not add any log line that carries a header, token or URL with credentials.
- No `git checkout` / `git restore` / `git reset` / `git stash`. Commit from the repo root with `git commit --only -- <paths>`; subjects start with `rewrite: `. Commit after every completed, verified task.
- Rust gates, from `rewrite/`: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p app --all-targets --features e2e -- -D warnings`, `cargo test -p grid-core`, `cargo test -p app --lib`; repo root: `bash rewrite/scripts/check_secret_hygiene.sh`. Frontend, from `rewrite/app`: `npm test` (vitest), `npm run check` (svelte-check; baseline 3 warnings: Details.svelte ×2, DownloadsFooter.svelte ×1 — the count must not rise). E2E: `npm run typecheck` in `rewrite/e2e`; groups via `bash rewrite/scripts/e2e.sh <group…>` from the repo root; never `E2E_SKIP_BUILD=1` after a source change.
- TDD: write the failing test first in every step that says "test first".
- Every behaviour change updates the matching `docs/porting/*.md` section in the same task (user requirement). `rewrite/README.md` changes only if a command changes (none expected).
- "Verify in task" notes mark facts this plan could not confirm from the code; the implementer checks them before coding and adjusts the step, not the intent.
- All `rewrite/` paths below are relative to `rewrite/`; `docs/` paths are relative to the repo root.

## Waves (parallelism and conflicts)

| Wave | Tasks | May run concurrently? | Why |
|------|-------|-----------------------|-----|
| A | 1, 2, 9, 10, 11 | Yes, all five | Disjoint files. Task 1 must NOT touch `app/src-tauri/src/cloud_service.rs` or `crates/grid-core/src/cloud/ops/*` (it keeps the old predicate names as delegating wrappers); Task 11 owns those. |
| B | 4, 7 | Yes, both | Task 4 owns `config.rs`, `lib.rs`, a new `commands/logging.rs`, `api.ts`, `AppearancePage.svelte`. Task 7 owns `launch/mod.rs` and `autoconfig/`. Task 7 waits for Task 1 (both edit `launch/mod.rs`). |
| C | 5, then 6, then 8 | No — serial | All three edit `app/src-tauri/src/commands.rs` and `lib.rs` (5 and 8 also edit `Emulators.svelte` and `api.ts`; 6 and 8 both touch `launch/emu_install.rs`/`launch/spawn.rs` neighbours). |
| D | 3 | Alone | Touches `library/mod.rs`, `catalog.rs`, `forge.rs`, `commands.rs`, `lib.rs`, `Emulators.svelte`, `api.ts`, docs 04 and 10. |
| E | 12 | Alone, last | Full e2e, `cargo clean --profile dev`, push. |

Conflict map (never concurrent): {1,7} on `launch/mod.rs`; {1,11} only if Task 1 renames callers — it must not; {4,5,6,8,3} on `commands.rs`/`lib.rs`; {5,8,3} on `Emulators.svelte`; {7,8} on `launch/`; {4,3} on `api.ts` if Task 3 starts before Task 4 ends.

---

### Task 1: Linux platforms are native everywhere

**Spec:** `docs/porting/03-library-install.md` "Open decision — launch-target line vs. install block reason disagree on Linux platforms" (~line 1561) becomes a ruling; `docs/porting/04-emulator-launch.md` §9 (native launch); `docs/porting/06-cloud-saves.md` "Save scope" / "Block reasons".

**Files:**
- Modify: `crates/grid-core/src/library/platforms.rs` — `is_native_platform` becomes the ONE predicate: trimmed, lowercased, starts with `windows` OR `linux`. Add `is_windows_platform` (windows-only) for the compat-tool decision.
- Modify: `crates/grid-core/src/cloud/scope.rs` — `is_native_executable_platform` stays as a name (callers in `cloud/ops/{mod,upload,restore}.rs`, `app/src-tauri/src/cloud_service.rs`, `launch/selection.rs` keep compiling) but delegates to `library::platforms::is_native_platform`; doc comment records the ruling.
- Modify: `crates/grid-core/src/launch/native.rs` — `build_native_command`: when `!is_windows_platform(&row.platform)` the tool is forced blank (no wine, no umu-run, no `WINEPREFIX`, `tool_label == ""`) regardless of `row.native_compat_tool` and `default_compat_tool`. Windows-platform rows are unchanged.
- Modify: `crates/grid-core/src/launch/mod.rs` line ~192 — the native branch already keys on `is_native_platform`; only the doc comment ("for a `windows*` platform row") changes.
- Modify: `crates/grid-core/src/launch/selection.rs` `install_block_reason` — no logic change once scope's predicate widens; add a test.
- Modify: `app/src/lib/details/cloud.ts` — ONE TS predicate: `isNativePlatform(platform)` (windows-or-linux). `isNativeExecutablePlatform` and `isNativeLaunchPlatform` become aliases of it (keep the exports so `Details.svelte`, `details/header.ts`, `server/header.ts`, `emulators/defaults.ts` compile) with a comment that the split is gone; `app/src/lib/details/actions.ts` `isNativePlatform` re-exports the one from `cloud.ts` instead of its own windows-only copy. Update `cloud.test.ts` (delete the "is wider than…" test; add linux cases) and `actions.test.ts` if it pins windows-only.
- Modify: `app/src/lib/Details.svelte` — nothing functional; `isNativeInstall` and `isNative` now agree. Verify in task: `NativeSettings.svelte` shows a compat-tool select; for a Linux-platform game that select must be hidden or disabled with the reason "Linux games run directly" (pure helper + vitest; keep the wording in one TS helper).
- Modify: `docs/porting/03-library-install.md` (~1561: replace the open decision with the ruling and date), `docs/porting/04-emulator-launch.md` §9 (compat tool applies to windows-platform rows only), `docs/porting/06-cloud-saves.md` (native predicate covers linux).
- Verify in task: `app/src-tauri/src/commands/updates.rs:69` routes native updates through `install_native_update` — a Linux row now takes that path too; confirm `library/mod.rs:893/962` accept it (they key on the same predicate, so they should).

**Interfaces:**
- `pub fn is_native_platform(platform: &str) -> bool` (windows|linux), `pub fn is_windows_platform(platform: &str) -> bool` (windows only), both in `library::platforms`.
- `build_native_command` signature unchanged.

- [ ] **Step 1 (test first, grid-core):** `platforms.rs` tests: `"Linux"`, `" linux x86_64"` are native; `"Windows"` still native; `is_windows_platform("Linux") == false`. `scope.rs`: `cloud_save_block_reason("Linux", State, …)` returns the PC-game sentence; `("Linux", Save, …)` returns `""`. `selection.rs`: `install_block_reason("Linux", &[], &[], &no_cores) == ""`. `native.rs`: a Linux-platform row with `native_compat_tool = "wine"` and `default_compat_tool = "GE-Proton9"` yields `argv == [exe]`, empty env, `tool_label == ""`, and never calls `which`; a Windows-platform row keeps today's behaviour. `cargo test -p grid-core` → FAIL, implement → PASS.
- [ ] **Step 2 (TS):** `cloud.test.ts`/`actions.test.ts` first, then the predicates. `cd app && npm test && npm run check`.
- [ ] **Step 3 (docs):** the three porting docs.
- [ ] **Step 4 (gates):** full Rust gate list; `bash rewrite/scripts/e2e.sh native cloud-saves updates` (no spec change expected; a Linux fixture is not required — verify in task whether `fixtures-native` can gain one Linux-platform rom cheaply; if yes, add an assertion that Play spawns the executable with no `wine` stub in `wine-argv.log`).
- [ ] **Step 5:** commit `rewrite: treat linux platforms as native everywhere; linux games launch with no compat tool`.

---

### Task 2: Icon-button hover and focus colour

**Spec:** user ruling (b); `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` §4 (tokens) — add one sentence.

**Files:**
- Modify: `app/src/app.css` ~line 215: `.icon-btn:hover, .icon-btn:focus-visible { background: var(--surface); color: var(--primary); }`.
- Check (designer decides, list is complete per grep): components that set `color` on an `.icon-btn`: `app/src/lib/Details.svelte` `.close` (`var(--text)`) and `.dismiss` (`var(--danger)`); `app/src/lib/details/NativeSettings.svelte` `.close` (`var(--text)`); `app/src/lib/details/CloudPanel.svelte` `.remove` (`var(--danger)`); `app/src/lib/details/MediaViewer.svelte` `.icon` (`#fff` on a scrim, own hover rule — deliberately exempt, it floats over artwork). Scoped component rules win over the global hover rule at equal specificity, so `.close` and `.dismiss`/`.remove` will NOT turn `--primary` unless the component adds its own `:hover, :focus-visible { color: var(--primary) }` or drops the base colour. Ruling (b) says the glyph turns `--primary`; apply it to `.close` in both dialogs. For the two danger buttons (`.dismiss`, `.remove`) keep danger on hover — see Open Questions.
- Verify in task: no rule elsewhere sets `.icon-btn` `background` on hover (grep `icon-btn` in `app/src` — only the five files above use it).

- [ ] **Step 1:** SSR render test is not meaningful for CSS; instead add a vitest that reads `app.css` and asserts the `.icon-btn:hover` block contains `var(--surface)` and `var(--primary)` (pattern: plain `fs.readFileSync` in a `*.test.ts`; verify in task that a similar CSS-pinning test does not already exist to extend).
- [ ] **Step 2:** CSS edits; `npm test && npm run check`; hand-check in `npx tauri dev`: hover the details close button and the cloud "remove path" button.
- [ ] **Step 3:** spec sentence; commit `rewrite: icon buttons hover to the surface colour with a primary glyph`.

---

### Task 9: Error toasts are announced assertively

**Spec:** none in porting docs (a11y improvement); note it in `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` toast paragraph.

**Files:**
- Modify: `app/src/lib/Toast.svelte` — render two live regions: the existing `role="status" aria-live="polite"` container for non-error toasts, and a `role="alert"` container (implicitly assertive) for `level === 'error'` ones; keep `data-testid="toast-region"` on a wrapper so `e2e/specs/*` selectors (`toast-region`, `toast`) keep working (verify in task: grep `toast-region` in `e2e/specs`). Also prefix error text with a visually-hidden `Error:` span (class `sr-only`; verify in task whether `app.css` already has an `sr-only`/`visually-hidden` utility — add one if not).
- Create: `app/src/lib/Toast.svelte.test.ts` — SSR render tests (`// @vitest-environment node`, `render` from `svelte/server`). The store is module state (`stores/toasts.svelte.ts` `pushToast(text, level)`); push an error and a success, render, assert `role="alert"` wraps the error text, `role="status"` wraps the success text, and the `Error:` prefix appears once. Verify in task: the store exposes a reset (`dismissToast(id)` exists) so tests stay independent.

- [ ] **Step 1:** test first → FAIL; component change → PASS; `npm test && npm run check`.
- [ ] **Step 2:** spec sentence; commit `rewrite: announce error toasts as alerts`.

---

### Task 10: Picker failure is reported once, not swallowed

**Spec:** `docs/porting/02-config-and-secrets.md` has no picker section; add one line to the redesign spec §10 (Browse buttons are additive; failure feedback).

**Files:**
- Modify: `app/src/lib/pickers.ts` — separate "cancelled" (`open` resolved to `null`/non-string) from "failed" (`open` threw). On failure: return `null` AND, once per process (module-level flag), `pushToast('Could not open a file dialog. Enter the path by hand.', 'error')`. Under the E2E build stay silent: gate on `import.meta.env.VITE_E2E` (declared in `app/src/vite-env.d.ts:4-7`, used in `app/src/main.ts:8-11`; `scripts/e2e.sh` sets `VITE_E2E=1` for the build). Callers (`Connect.svelte:18-21`, `Server.svelte:437-440`, `details/CloudPanel.svelte:240-245`, `emulators/EmulatorForm.svelte:88-93`) already treat `null` as cancel and need no change. No e2e spec references a Browse button (grep confirmed); specs type paths into `emu-form-path`, `connect-*`, `library-path` inputs.
- Create/extend: `app/src/lib/pickers.test.ts` — `vi.mock('@tauri-apps/plugin-dialog')`: resolves `null` → returns null, no toast; rejects → returns null, toast pushed once across two calls; with the E2E flag set → no toast. Mock the toasts store or read `toasts.list`.
- Callers (`pickFolder`/`pickFile`) need no change — verify in task by grep; they already treat `null` as cancel.
- E2E: specs must never depend on the dialog (they type into the text inputs beside Browse). Verify in task: `grep -rn -i browse e2e/specs` shows no click on a Browse button; if one exists, replace it with the text-input path.

- [ ] **Step 1:** tests first → FAIL; implement → PASS; `npm test && npm run check`.
- [ ] **Step 2:** spec line; commit `rewrite: tell the user when no file dialog can open`.

---

### Task 11: Windows Documents-redirection resolver for native save paths

**Spec:** `docs/porting/06-cloud-saves.md` "Platform differences" (~line 1100: "Save paths containing `%USERPROFILE%\Documents` are rewritten to the Shell-resolved Documents folder when the two differ … No adjustment happens off Windows") and the test-oracle line (~1195). Python: `grid_launcher/library/cloud_transfer.py:484-541` (`resolve_native_save_dir`, returns early when `windows_documents is None or sys.platform != "win32"`); the value comes from `grid_launcher/emulator/pcsx2.py:10-48` (`SHGetKnownFolderPath(FOLDERID_Documents)`).

**Evidence:** `crates/grid-core/src/cloud/native.rs:209-213` `resolve_native_save_dir(raw, windows_documents: Option<&Path>, wine_prefix: Option<&Path>) -> PathBuf` branches on `windows_documents.is_some()` (no win32 gate). `crates/grid-core/src/cloud/dirs.rs:137-160` `ResolveContext` already has `windows_documents: Option<&'a Path>` ("`None` off Windows"). Every construction/call site passes `None` today: `app/src-tauri/src/cloud_service.rs:1019`, `:1160` (the two `ResolveContext` builders) and `:1686` (the Details tooltip's direct `resolve_native_save_dir(raw, None, wine_prefix)`); grid-core forwards `ctx.resolve_ctx.windows_documents` in `cloud/ops/native.rs:112,118,208,212,271,276`, `cloud/ops/mod.rs:234`, `cloud/native.rs:355,392`. A sibling stub `crates/grid-core/src/autoconfig/readers.rs:297-299` `fn windows_documents_folder() -> Option<PathBuf> { None }` (PCSX2 standard-config path, consumed at `:369`) has the same gap. `directories = "6.0.0"` is already a grid-core dependency (`crates/grid-core/Cargo.toml:11`); no `windows`/`dirs` crates anywhere.

**Files:**
- Modify: `crates/grid-core/src/platform.rs` (new; or `cloud/native.rs` if a new module is unwelcome) — `pub fn windows_documents_dir() -> Option<PathBuf>`: `#[cfg(windows)]` → `directories::UserDirs::new()?.document_dir().map(Path::to_path_buf)` (the `directories` crate resolves `FOLDERID_Documents` through the Known Folder API on Windows, which honours User Shell Folders redirection — verify in task against the 6.0 docs; if it does not, add `windows-sys` with `SHGetKnownFolderPath`); `#[cfg(not(windows))]` → `None`. Make `autoconfig/readers.rs::windows_documents_folder` delegate to it.
- Modify: `app/src-tauri/src/cloud_service.rs:1019`, `:1160`, `:1686` — pass `windows_documents_dir()` (computed once per operation, held in a local `Option<PathBuf>`, borrowed into the context).
- Modify: `docs/porting/06-cloud-saves.md` — the two sections: rewrite resolves Documents via the Known Folder API on Windows; `None` elsewhere; the win32 gate is implicit in the `None`.

- [ ] **Step 1 (test first):** `native.rs` already has resolver tests at `:516-528` with an injected Documents path — extend with the documented cases if any is missing (plain expansion with `None`; redirected Documents; a non-Documents path untouched). New test: `windows_documents_dir()` is `None` under `#[cfg(not(windows))]`. App layer: make the context-building helper in `cloud_service.rs` take the Documents value as a parameter so a `#[cfg(test)]` test (existing `cloud_service.rs` test style; verify) can inject `Some(tmp)` and assert a `%USERPROFILE%\Documents\...` raw path in the panel's path entries resolves under the injected directory.
- [ ] **Step 2:** implement; `cargo test -p grid-core`, `cargo test -p app --lib`, full Rust gate list; `bash rewrite/scripts/e2e.sh cloud-saves native`.
- [ ] **Step 3:** docs; commit `rewrite: resolve the Windows Documents folder for native save paths`.

---

### Task 4: Debug-prints toggle

**Spec:** `docs/porting/02-config-and-secrets.md` line 138 (`debug_prints`, bool, default `true`; grid-launcher.py:2150-2156 lenient parsing) and line 373 (settings collection); Python UI puts the checkbox "Enable debug prints" in the Appearance form under a "Debug" row (grid-launcher.py:1712-1714).

**Files:**
- Modify: `crates/grid-core/src/config.rs` — `Config.debug_prints: bool`, `#[serde(default = "default_true")]`, placed among the scalar keys BEFORE `ui` (TOML tables must follow scalars — see the `ui` comment). Round-trip test: a config without the key loads `true`; save writes it.
- Modify: `app/src-tauri/src/lib.rs` lines 44-49 — build the subscriber with `tracing_subscriber::reload::Layer` around the `EnvFilter`; keep the handle in a `static OnceLock<reload::Handle<EnvFilter, Registry>>` in a new `app/src-tauri/src/logging.rs` (verify in task: `tracing-subscriber` 0.3.23 with the `env-filter` feature includes `reload`; no new feature flag expected). Rule, as a pure function `pub fn filter_directive(rust_log: Option<&str>, debug_prints: bool) -> String`: `RUST_LOG` set and non-blank → its value verbatim (and the toggle never overrides it); else `"debug"` when `debug_prints` else `"info"`. At startup read `Config::load(&Config::default_path())` (a load error → treat as `true`, the default) and apply. Expose `pub fn apply_debug_prints(enabled: bool)` that recomputes the directive with the current `RUST_LOG` and reloads.
- Create: `app/src-tauri/src/commands/logging.rs` — `get_debug_prints() -> bool`, `set_debug_prints(enabled: bool)`: `modify_config` (config_write.rs) then `apply_debug_prints`. Register both in `lib.rs`'s `generate_handler!`.
- Modify: `app/src/lib/api.ts` — `getDebugPrints`, `setDebugPrints`.
- Modify: `app/src/lib/settings/AppearancePage.svelte` — a `field` row labelled `Debug` with a checkbox `Enable debug prints` (`data-testid="debug-prints-toggle"`), loaded on mount, saved on change (same pattern as the background-art toggle). Verify in task: whether a `uiSettings` store slot is the better home than a page-local `$state`; either is acceptable, page-local is simpler.
- Modify: `docs/porting/02-config-and-secrets.md` — line 138 row gains "rewrite: `debug_prints` switches the tracing filter between `info` and `debug` at runtime; `RUST_LOG` wins when set".

- [ ] **Step 1 (test first):** `config.rs` round-trip test; `logging.rs` tests for `filter_directive` (four cases: env set/unset × flag on/off). `cargo test -p grid-core`, `cargo test -p app --lib` → FAIL then PASS.
- [ ] **Step 2:** commands, `api.ts`, page; `npm test && npm run check`. Hand-check: toggle on, a `debug!` line from grid-core appears on the terminal; toggle off, it stops; `RUST_LOG=warn npx tauri dev` ignores the toggle.
- [ ] **Step 3:** e2e: the `updates` group asserts each Settings pane renders a line; add one assertion that `debug-prints-toggle` exists on Appearance (`bash rewrite/scripts/e2e.sh updates`).
- [ ] **Step 4:** docs; commit `rewrite: add the debug prints toggle and switch the log filter at runtime`.

---

### Task 7: RetroArch settings sync before every launch

**Spec:** `docs/porting/05-emulator-autoconfig.md:315-326` call-site table ("Before launching a game — details_view_mixin.py:1457"; "Before launching an emulator standalone — emulator_ui_mixin.py:1655"); `docs/porting/04-emulator-launch.md:528-536` records the rewrite deviation ("No `_ensure_emulator_sync_settings` pre-pass… the rewrite runs the autoconfig sync at add/install time"). Load the `emulator-autoconfig` skill (`.claude/skills/emulator-autoconfig/SKILL.md`): the RetroArch flat writer overwrites managed keys only; a second run must report no change.

**Evidence:** Python `_ensure_emulator_sync_settings(name, path)` (`emulator_ui_mixin.py:365-440`) is memoized per `"{name}::{path}"` for the process, and for RetroArch calls `ensure_retroarch_save_location_settings(path, enable_fullscreen=True, ra_username, ra_token, username=romm_username)`. The rewrite's equivalent is `autoconfig::sync_new_emulator(entry_name, &SyncContext)` (`crates/grid-core/src/autoconfig/mod.rs:524`), which dispatches to `retroarch::ensure_settings(path, true, &romm_username, ra)` (`autoconfig/retroarch.rs:365`) and is called today only from `app/src-tauri/src/commands.rs:873-891` (`save_emulator`, add only). `SyncContext` (`mod.rs:376-403`) needs `config_path`, `platforms`, `platform_slugs`, `ps3_library_path`, `ra`, `profiles` — `save_emulator` builds it from `state.install` (`install.ra_credentials()`, `known_platforms`, `platform_slugs`). `LaunchService` (`launch/mod.rs:83-117`) has none of that; the precedent for app-supplied behaviour is `set_session_finished_hook` (`:157`). The pre-spawn point in `launch()` is between `resolve_launch` (`:217`) and `spawn_child` (`:223`). Standalone: `commands.rs:925 launch_emulator` loads `Config` and calls `prepare_standalone_emulator_launch` + `spawn_standalone_emulator` inside one `spawn_blocking`.

**Files:**
- Modify: `crates/grid-core/src/launch/mod.rs` — `pub type PreLaunchHook = Arc<dyn Fn(&str /* emulator name */, &str /* path */) + Send + Sync>`; `set_pre_launch_hook`; in `launch()`, for the emulated branch only, when `entry_is_retroarch(entry, profiles)` (`launch/selection.rs:131`; `resolve_launch` already resolves the entry and loads profiles — expose them on the plan) call the hook before `spawn_child`. The hook cannot fail the launch (it returns `()`; it logs its own errors).
- Modify: `app/src-tauri/src/lib.rs` — install the hook: build a `SyncContext` exactly as `save_emulator` does (factor that construction into a shared `fn sync_context(state) -> SyncContext` in `commands.rs`), call `sync_new_emulator(name, &ctx)`, `tracing::warn!` on `Err` or on report warnings (no paths with secrets — the RA token is a `SecretString`).
- Modify: `app/src-tauri/src/commands.rs::launch_emulator` — same sync call before `spawn_standalone_emulator`, RetroArch entries only (`entry_is_retroarch` with `load_profiles()`), errors logged not returned. Update the doc comment on `spawn.rs:116-118` that records the old deviation.
- Memoization: Python's per-process `"{name}::{path}"` memo is NOT ported — the writer is idempotent, and a fresh sync per launch is what makes a changed RA credential or username reach the cfg. Record this in doc 05.
- Modify: `docs/porting/05-emulator-autoconfig.md:315-326` (rewrite column: hook + command), `docs/porting/04-emulator-launch.md:528-536` (delete the deviation, describe the hook).

- [ ] **Step 1 (test first, grid-core):** in `launch/mod.rs` tests (they seed a tempdir registry + config and a `#!/bin/sh` stub — verify the helper names): a hook recording `(name, path)` into an `Arc<Mutex<Vec<_>>>` fires once for an entry named `RetroArch` and never for `Dolphin`; a hook that panics does not prevent the session from registering (wrap the call in `std::panic::catch_unwind` or document that hooks must not panic — pick one and test it).
- [ ] **Step 2 (autoconfig idempotency):** `retroarch.rs` tests already include an idempotency case (verify — the skill requires it); if absent add: call `ensure_settings` twice on a tempdir RetroArch layout, second `EnsureResult` reports unchanged and the file bytes are identical.
- [ ] **Step 3:** implement; full Rust gate list; `bash rewrite/scripts/e2e.sh launch emulators` — `emulators.spec.ts:27-53` seeds a stub named `retroarch`; extend the standalone-launch case to assert a `retroarch.cfg` appears next to the stub after Launch (the writer creates it — verify the target path `retroarch::config_path_candidates` picks for a bare stub in a tempdir).
- [ ] **Step 4:** docs; commit `rewrite: sync RetroArch settings before every game and standalone launch`.

---

### Task 5: Eden advisory notes

**Spec:** `docs/porting/04-emulator-launch.md:676-679` ("Eden shows notes when `eden_keys_path` or `eden_has_firmware` is falsy").

**Evidence (Python, verbatim):** `emulator_ui_mixin.py:729-738` — when `not eden_keys_path(path_text)`: `Switch keys (prod.keys) must be placed in user/keys/ before playing games.`; `:740-748` — when `not eden_has_firmware(path_text)`: `Switch firmware must be installed via Emulation → Install Firmware before playing games.` Both use `path_text = entry["path"].strip()`. `grid_launcher/emulator/eden.py:372-381` `eden_keys_path`: blank → None; `emulator_dir` = the path if it is a directory else its parent; checks ONLY the portable `<emulator_dir>/user/keys/prod.keys` (`exists() and is_file()`). `:383-392` `eden_has_firmware`: `<emulator_dir>/user/nand/system/Contents/registered` is a directory AND has at least one entry (any name). The wider `eden_user_root_candidates` (XDG/AppData) is NOT used by these two probes.

**Evidence (rewrite):** `crates/grid-core/src/autoconfig/eden.rs` has `config_path_candidates` and `ensure_settings` plus private `maybe_create_user_dir` (dir-or-parent rule, `~` expansion) — no port of the two probes. `app/src-tauri/src/commands.rs:816 list_emulators` returns raw `EmulatorEntry`s; notes are frontend-only in `app/src/lib/emulators/notes.ts` (`emulatorNotes(name): EmulatorNote[]`, substring table; its header comment defers exactly these two notes). Consumed in `Emulators.svelte:564`. `notes.test.ts:15-19` asserts the Eden static note as an exact single-element array.

**Files:**
- Modify: `crates/grid-core/src/autoconfig/eden.rs` — `pub fn eden_keys_path(emulator_path: &str) -> Option<PathBuf>` and `pub fn eden_has_firmware(emulator_path: &str) -> bool`, portable-only rule above, reusing the private dir-or-parent helper.
- Create: command `emulator_facts(name: String) -> EmulatorFacts { eden_keys_present: bool, eden_firmware_present: bool }` in `app/src-tauri/src/commands.rs` (register in `lib.rs`); computed only when the entry matches the Eden autoprofile (`autoconfig::is_eden`-style predicate — verify the name used by `sync_new_emulator`'s dispatch), else both `true` (no notes).
- Modify: `app/src/lib/api.ts` (`emulatorFacts`), `app/src/lib/emulators/notes.ts` — a second pure helper `dynamicEmulatorNotes(name, facts): EmulatorNote[]` returning the two verbatim notes (keys `eden-keys`, `eden-firmware`) for an Eden-matching name when the fact is false; keep `emulatorNotes` unchanged so the exact-array tests hold.
- Modify: `app/src/lib/Emulators.svelte` — fetch facts per installed row when the pane comes forward (same refresh trigger as the catalog, `:278-280`/`:366-374`), render the dynamic notes after the static ones with the same `emulator-note-<key>-<name>` test id pattern.
- Modify: `docs/porting/04-emulator-launch.md:676-679` — rewrite sentence naming the command and helper.

- [ ] **Step 1 (test first, grid-core):** tempdir: `Eden/eden.exe` + `Eden/user/keys/prod.keys` → `Some`; missing → `None`; a directory path (not exe) works; `registered/` with one file → `true`; empty or absent → `false`.
- [ ] **Step 2 (test first, TS):** `notes.test.ts`: Eden + both facts false → both notes verbatim in that order; Eden + both true → `[]`; non-Eden + false facts → `[]`.
- [ ] **Step 3:** command, `api.ts`, component; gates: full Rust list, `npm test && npm run check`; `bash rewrite/scripts/e2e.sh emulators` (add a case if the group can seed an `eden` stub cheaply: add via the form, both notes visible; create `user/keys/prod.keys` via the seed — not required).
- [ ] **Step 4:** docs; commit `rewrite: show Eden keys and firmware notes on the emulator card`.

---

### Task 6: Manual emulator entry pointing at an archive

**Spec:** `docs/porting/04-emulator-launch.md:792-793` ("Manual archive adds mark the detected executable `0o755` on non-`win32`"). Python flow (verbatim strings): the config dialog (`grid_launcher/ui/dialogs.py:325`) treats suffixes `{.7z, .zip, .rar, .tar, .gz, .bz2, .xz}` as archives (a NEW entry only — the edit browser offers executables only, `:465-475`, though a typed archive path still routes); on save (`emulator_ui_mixin.py:1459-1470`) a missing file warns `Archive file was not found:\n{archive_path}`; `_extract_emulator_archive` (`:1371-1404`): no library path → `Set a Library Path in Settings before adding an emulator archive.`; destination `emulator_install_directory(library, emulator_name)` = `<library>/Emulators/<sanitize(ENTRY NAME)>` (the entry name, not the archive stem); extraction failure → `Failed to extract emulator archive: {error}`; detection via `select_emulator_executable_path`; none found → title `Archive Extracted`, body `Archive extraction finished, but no launchable executable was detected. Open Config to set the executable path manually.`; on success `os.chmod(path, 0o755)` off win32 (`:1399-1403`).

**Evidence (rewrite):** every building block exists: `library/extract.rs::is_extractable_archive` (`:70`; `EXTRACTABLE_SUFFIXES` `:30` = the same seven suffixes) and `extract_archive(archive, dest, progress)` (`:93`; wipes and recreates `dest`); `launch/emu_install.rs::emulator_install_dir(library, stem)` (`:24`), `launchable_emulator_file` (`:125`), `select_executable(title, install_dir, archive)` (`:253`, the port of Python's selector); `library/mod.rs::make_executable` (`:2841`, the `0o755` step) and `NO_EMULATOR_EXECUTABLE` (`:126`). `save_emulator` (`commands.rs:825-906`) stores `entry.path` verbatim; `EmulatorForm.svelte:85-93` browses with no filter, so an archive path reaches the backend today and is saved as the executable. Tests at `commands.rs:1899-2000` cover `apply_save_emulator` only.

**Files:**
- Create: `crates/grid-core/src/launch/emu_install.rs` function `pub fn install_manual_archive(library: &Path, entry_name: &str, archive: &Path) -> Result<PathBuf, String>`: `is_extractable_archive` gate is the caller's; missing file → `Archive file was not found:\n{path}`; dest `emulator_install_dir(library, entry_name)`; `extract_archive` (a `&mut |_, _| {}` progress) mapped to `Failed to extract emulator archive: {e}`; `select_executable(entry_name, &dest, archive)` else the verbatim "Archive extraction finished…" message; `make_executable` (move it to `emu_install.rs` or re-export — one copy); return the path.
- Modify: `app/src-tauri/src/commands.rs::save_emulator` — before `modify_config`: if `is_extractable_archive(Path::new(entry.path.trim()))`, require `config.library_path` (else the verbatim "Set a Library Path…" error), call the helper, replace `entry.path` with the result. Applies to add AND edit (the rewrite has one path field; Python's edit path also routes a typed archive). Autoconfig then runs on the executable as today.
- Modify: `app/src/lib/emulators/EmulatorForm.svelte` — no logic change; verify the sheet re-reads the saved entry so the path field shows the executable after save.
- Modify: `docs/porting/04-emulator-launch.md:792-793` — extend with the rewrite's helper, the entry-name directory rule, and that edits route too.

- [ ] **Step 1 (test first, grid-core):** tempdir: a zip with `bin/emu` (mode 0o644) and `readme.txt` → returns `<library>/Emulators/<name>/bin/emu` with `0o111` bits (unix); a zip with no launchable file → the verbatim message; a missing archive → the verbatim "not found" message; a `.tar.gz` variant.
- [ ] **Step 2 (test first, app):** factor the archive gate into a pure `fn archive_path_to_extract(entry: &EmulatorEntry) -> Option<PathBuf>` in `commands.rs` and test it (archive → Some; `.AppImage` → None; blank → None).
- [ ] **Step 3:** implement; full Rust gate list; `bash rewrite/scripts/e2e.sh emulators` (optional: zip the existing stub in the seed and add it by path; assert `config.toml` `path` ends in the extracted executable).
- [ ] **Step 4:** docs; commit `rewrite: extract an archive given as a manual emulator path and store its executable`.

---

### Task 8: 500 ms early-exit warning for standalone emulator launches

**Spec:** `docs/porting/04-emulator-launch.md` ~490 and §8b (~530-536, the recorded deferral). Python `grid_launcher/emulator/launch.py:321-323`, verbatim:
`Process exited immediately (code {exit_code}).\nCommand:\n{command_text}` (command space-joined). Timers: `details_view_mixin.py:1441/1471`, `emulator_ui_mixin.py:1662` (`QTimer.singleShot(500, …)`).

**Evidence (rewrite):** the GAME path already has the check: `launch/mod.rs:70 EARLY_EXIT_DELAY`, `early_exit_watch`, `schedule_early_exit_check` (`:347`), `early_exit_message` (`:433`) — but with its own wording `Game exited immediately (code N): <cmd>` (plus signal/unknown variants), surfaced through `SessionsSnapshot.warning` → Tauri event `sessions-changed` (`lib.rs:270-272`) → `stores/sessions.svelte.ts` (`lastWarning`, sticky, `dismissWarning`) → `Details.svelte:737-744` `details-warning` (`role="alert"`, `details-warning-dismiss`). `e2e/specs/launch.spec.ts:195-211` asserts `details-warning` contains "exited immediately" for the seeded `InstantExit` stub (exit code 3). STANDALONE: `launch/spawn.rs:171 spawn_standalone_emulator(argv, working_dir) -> Result<(), String>` moves the `Child` into a detached reaper thread and keeps no handle; `commands.rs:925 launch_emulator(name) -> Result<(), String>`; `Emulators.svelte:460-469` toasts errors via `pushToast(msg, 'error')`. Spawn tests (`spawn.rs:489-528`) use a `#!/bin/sh` stub that touches a marker, not `true`/`false`.

**Files:**
- Modify: `crates/grid-core/src/launch/spawn.rs` — `pub fn process_exited_early_message(status: Option<ExitStatus>, argv: &[String]) -> String` producing Python's verbatim text for a code (`Process exited immediately (code N).\nCommand:\n<joined>`), keeping the rewrite's signal/unknown variants in the same shape (`(signal: …)` / `(unknown)`). `spawn_standalone_emulator` returns `Result<std::process::Child, String>` (drop the reaper thread and the deferral comment). Add `pub fn wait_for_early_exit(mut child: Child, argv: &[String]) -> Option<String>` (blocking: sleep 500 ms, `try_wait`; exited → `Some(message)`; running → spawn the old reaper thread for it and return `None`).
- Modify: `crates/grid-core/src/launch/mod.rs::early_exit_message` — delegate to the shared builder so BOTH paths emit the same text (the game path's wording changes from "Game exited immediately (code N): cmd" to Python's; `launch.spec.ts:195-211` matches on "exited immediately" and keeps passing — verify the exact matcher).
- Modify: `app/src-tauri/src/commands.rs::launch_emulator` → `Result<Option<String>, String>` (already inside `spawn_blocking`, so the 500 ms hold is fine there).
- Modify: `app/src/lib/api.ts` (`launchEmulator(): Promise<string | null>`), `app/src/lib/Emulators.svelte:460-469` — on a string, `pushToast(message, 'error')`.
- Modify: `docs/porting/04-emulator-launch.md` §8b — remove the deferral; state both surfaces (Details strip for games, toast for standalone) share one message builder.

- [ ] **Step 1 (test first, grid-core):** `spawn.rs` (unix): a `#!/bin/sh` stub `exit 3` → `wait_for_early_exit` returns `Some` equal to `Process exited immediately (code 3).\nCommand:\n<path>`; a stub `sleep 5` → `None` within ~600 ms; missing executable still `Err("Failed to launch emulator:\n…")`. `mod.rs`: existing early-exit test's expected string updated to the verbatim form.
- [ ] **Step 2:** command + frontend; `npm test && npm run check`; full Rust gate list; `bash rewrite/scripts/e2e.sh launch emulators` — add to `launch.spec.ts`: Emulators › Launch on the `InstantExit` stub shows a `toast` containing `Process exited immediately (code 3).`
- [ ] **Step 3:** docs; commit `rewrite: warn when a standalone emulator exits within 500 ms`.

---

### Task 3: Emulator "Update from Source"

**Spec:** `docs/porting/04-emulator-launch.md` "Install directory" / "Extraction and post-install" / "Version check" (~757-810) and the dialog rows (~700-720); `docs/porting/10-identity-updates.md:490-495` and the Platform table row for `Emulators`.

**Evidence (Python):** there is no `source_emulator_update` function; it is an `_install_mode` value routed through the normal source install (`install_mixin.py:1162-1165`, `:1399-1499`). The install dir is recomputed as `<library>/Emulators/<stem of "<name>-<configured tag>.zip">`, so a `latest` pin lands in the SAME directory; extraction merges. Finalize (`:1690-1716`): register, autoconfig (always), firmware pass only when NOT an update, record the source install. Toast: `Updated emulator '{name}' from source.` vs `Installed emulator '{name}' from source.` The recorded tag is the CONFIGURED one (`emulator_ui_mixin.py:140-171`), so a `latest` pin records `latest`. The Update button (`:844-858`, tooltip `Update from Source`) exists for every row with a source entry; the check runs ON CLICK, not on dialog open, uncached (`:1235-1293`, `SourceVersionCheckWorker`, `workers.py:439-510`): `direct` → `available_tag == "direct"`, no network; `github` → `/releases/tags/{tag}` for a pinned tag else `/releases/latest`; `gitea` → `{base}/api/v1/repos/{o}/{r}/releases/latest`; unknown provider → `Unsupported provider: {p}`; bad payload → `Source release API returned an unsupported payload shape.` / `Source release API response did not include tag_name.`. Result dialogs (`:1323-1370`, verbatim): error → title `Version Check Failed`, body `Could not check for updates:\n{error}`; `installed_display` = tag unless blank/`latest` → `unknown`; `available_display` = `Unknown (direct source)` for `direct`; up to date only when installed is a real pin equal to available → title `No Updates Available`, `Already up to date ({available}).`; otherwise a Yes/No `Update Available` question `Update {name}?\n\nInstalled: {installed}\nAvailable: {available}` (default No) → start the update. Missing source on click → title `No Source`, `No source download configured for this emulator.`

**Evidence (rewrite — most of the pipeline already exists):** `InstallService::install_emulator(source_id)` (`library/mod.rs:993-1029`) admits an `EmulatorJob`; `finalize_emulator` (~`:2045-2150`) extracts into `emulator_install_dir(library, stem of archive_file_name(profile, configured_tag))` (same-dir property as Python), MERGES over an existing tree (`merge_tree_into`), selects the executable, `make_executable`, `write_emulator_entry` (`:2190-2232`: replaces an existing entry by exact name IN PLACE → `fresh = false`; records `source_release_tag = configured_tag`), `sync_autoconfig` (always), fires `emulator_installed_hook` with `EmulatorInstalled { name, fresh, compat_tool }`; `lib.rs:246-258` runs the firmware pass only when `fresh`. So "update" = re-running `install_emulator` for the entry's `source_id`; no new `InstallMode` is needed. Forge: `ForgeClient::get(url, github_headers)` (`forge.rs:127`), `release_endpoint(api_base, tag)` (`:447`, the exact endpoint rule), `ResolvedDownload.release_tag` is the RESOLVED tag (`:36-45`); wiremock tests at `:521+`. Service pattern to copy: `app/src-tauri/src/app_update.rs` (`CheckOutcome { UpToDate, Newer, Failed }`, capped body, host-only logging) and `update_service.rs`'s `PassGate`. Catalog: `mark_installed` (`catalog.rs:215`). UI: `Emulators.svelte:538-590` rows with `emulator-launch-*`/`emulator-edit-*`/`emulator-delete-*`; drawer rows come from the shared downloads store (`kindLabel 'emulator' → 'Emulator'`). E2E: `emulator-catalog.spec.ts` installs PCSX2 + Redream against `mock-forge.mjs`; nothing covers an update.

**Decisions (this plan):**
- Trigger: on click of the row's `Update` button, like Python (no view-open check, no cache) — the button is always shown for a source-backed row (`source_id` non-blank); the check runs, then the confirm dialog. This avoids GitHub rate-limit exposure entirely and matches the reference. (Supersedes the "recommend one" question: on-click.)
- Preserved settings: `write_emulator_entry` replaces the entry in place — verify in task that it keeps `args`, `save_*`, `ignore_*`, `state_paths` from the EXISTING entry (if it rebuilds from the profile, merge: keep user fields, replace `path` and `source_*`). `default_emulators`/`retroarch_cores` are keyed by name and untouched.
- Installed tag: add `EmulatorEntry.source_installed_tag` (resolved tag from `ResolvedDownload.release_tag`, serde default/skip-if-empty like its siblings) so a `latest` pin can still show a real installed version. Deviation from Python (which shows `unknown` for `latest`): record it in doc 04. Comparison stays string equality (Python's).
- `direct` provider: available `Unknown (direct source)`, never "up to date", the confirm dialog offers the reinstall (Python behaviour).
- Toast on completion: `Updated emulator '{name}' from source.` when `fresh == false`, else the existing install wording (verify the rewrite already toasts `Installed emulator …`; if not, add both).

**Files:**
- Modify: `crates/grid-core/src/config.rs` — `source_installed_tag`. `library/mod.rs::write_emulator_entry` — set it from `job.resolved.release_tag`; preserve user fields on replace (test).
- Modify: `crates/grid-core/src/launch/forge.rs` — `pub async fn check_release_tag(&self, provider, owner, repo, base_url: Option<&str>, configured_tag) -> Result<String, SourceError>`: `direct` → `Ok("direct")` with no request; github/gitea via `release_endpoint` + `get`; payload rules and the three verbatim error strings above. Pure `pub fn version_check_outcome(installed: &str, available: &str) -> VersionCheck { installed_display, available_display, up_to_date }` mirroring `:1334-1349`.
- Modify: `app/src-tauri/src/commands.rs` + `lib.rs` — `check_emulator_update(name) -> VersionCheck` (entry lookup; no `source_id` → the verbatim `No source download configured for this emulator.`; check error → `Could not check for updates:\n{e}`), `update_emulator(name) -> ()` (`install_emulator(entry.source_id)`; the drawer row shows it). Pass the `fresh` flag through to the completion toast (verify how the install toast is emitted today — `set_emulator_installed_hook` in `lib.rs:246-258` is the place).
- Modify: `app/src/lib/api.ts`; new `app/src/lib/emulators/update.ts` — pure `updateConfirmText(name, check)` (`Update {name}?\n\nInstalled: …\nAvailable: …`) and `upToDateText(check)` (`Already up to date ({available}).`), vitest.
- Modify: `app/src/lib/Emulators.svelte` — `Update` button (`emulator-update-<name>`, title `Update from Source`) on rows with `source_id`; click → pending state → `checkEmulatorUpdate` → up to date: toast `Already up to date (…)` / else the app's existing confirm pattern (verify: a two-click confirm like Delete, or a dialog component) → `updateEmulator`; errors → error toast with the verbatim text.
- Modify: `e2e/mock-romm/mock-forge.mjs` — a control route (pattern: the RomM mock's `POST /__e2e__/offline`) that bumps the PCSX2 `releases/latest` tag; `e2e/specs/emulator-catalog.spec.ts` — after the existing PCSX2 install: bump, click `emulator-update-pcsx2`, confirm, drawer row reaches Completed, `config.toml`: `source_installed_tag` is the new tag, `args` unchanged, PS2 default still PCSX2, no second firmware row (`download-kind-*` count), `portable.ini` still present, forge request log shows `/releases/latest` exactly once for the check plus the asset download.
- Modify: `docs/porting/04-emulator-launch.md` (~700-720, ~757-810), `docs/porting/10-identity-updates.md` (~493 and the table row) — rewrite paragraphs: on-click check, same-dir merge, preserved fields, `fresh=false` firmware suppression, `source_installed_tag` deviation.

- [ ] **Step 1 (test first, grid-core):** `version_check_outcome` cases (blank/`latest` installed → `unknown`; `direct` → `Unknown (direct source)` and never up to date; equal pin → up to date; differing → not); `check_release_tag` wiremock: github latest 200 → tag; pinned tag → `/releases/tags/<tag>` hit; 404 → `Err`; non-object payload → verbatim; missing `tag_name` → verbatim; `direct` → no request recorded (`MockServer::received_requests` empty).
- [ ] **Step 2 (test first, grid-core service):** existing emulator-install test style (wiremock forge + tempdir library): install once, edit the entry's `args` and `save_paths`, install again → same directory, `path` updated, `source_installed_tag` = the mock's tag, `args`/`save_paths` preserved, hook fired with `fresh == false`, autoconfig marker present.
- [ ] **Step 3:** commands, `api.ts`, `update.ts` (vitest first), `Emulators.svelte`; `npm test && npm run check`; full Rust gate list including `--features e2e` clippy.
- [ ] **Step 4:** mock forge + spec; `bash rewrite/scripts/e2e.sh emulator-catalog emulators`.
- [ ] **Step 5:** docs 04 and 10; commit `rewrite: check a source-installed emulator for a newer release and update it in place`.

---

### Task 12: Housekeeping and release

**Files:** `docs/porting/*.md` (final consistency pass: every task above touched its section — grep each task's commit for `docs/porting`), `rewrite/README.md` only if a command changed (none expected; the test counts in "## Test" are stale and may be refreshed).

- [ ] **Step 1:** from `rewrite/`: `cargo fmt --check`, both clippy commands, `cargo test --workspace`; `bash rewrite/scripts/check_secret_hygiene.sh`; `cd rewrite/app && npm test && npm run check`; `cd rewrite/e2e && npm run typecheck`.
- [ ] **Step 2:** the FULL suite: `bash rewrite/scripts/e2e.sh` (no group filter; run detached, it takes long — see memory note "run the full gate detached").
- [ ] **Step 3:** `cd rewrite && cargo clean --profile dev` (keeps `release`).
- [ ] **Step 4:** `git status` clean, then `git push origin main`.

---

## Testing Impact (summary)

| Test file | Change |
|-----------|--------|
| `crates/grid-core/src/library/platforms.rs`, `cloud/scope.rs`, `launch/selection.rs`, `launch/native.rs` (inline tests) | Task 1 linux cases |
| `app/src/lib/details/cloud.test.ts`, `actions.test.ts` | Task 1 one predicate |
| new `app/src/lib/app-css.test.ts` (or existing CSS pin) | Task 2 |
| new `app/src/lib/Toast.svelte.test.ts` | Task 9 |
| `app/src/lib/pickers.test.ts` | Task 10 |
| `crates/grid-core/src/cloud/native.rs`, `cloud/ops/tests.rs` | Task 11 |
| `crates/grid-core/src/config.rs`, new `app/src-tauri/src/logging.rs` | Task 4 |
| `crates/grid-core/src/launch/mod.rs` tests, autoconfig retroarch tests | Task 7 |
| `crates/grid-core/src/autoconfig/eden.rs`, `app/src/lib/emulators/notes.test.ts` | Task 5 |
| new grid-core emulator-archive tests, `commands.rs` pure-fn test | Task 6 |
| `crates/grid-core/src/launch/spawn.rs` tests, `e2e/specs/launch.spec.ts` | Task 8 |
| `forge.rs`/`catalog.rs`/`library/mod.rs` tests, new TS helper tests, `e2e/specs/emulator-catalog.spec.ts`, `e2e/mock-romm/mock-forge.mjs` | Task 3 |

## Risks and Edge Cases

- Task 1: a Linux-platform game already installed with `native_compat_tool` set keeps the value in the registry but it is ignored; the Game Settings sheet must not show a stale compat-tool selection as active. Multi-file native installs, `install_native_update` and uninstall removal steps all key on the same predicate — one change moves all of them; run the `native` and `updates` e2e groups.
- Task 2: scoped component colour rules beat the global hover rule; the plan lists every override. Do not touch `MediaViewer.svelte`.
- Task 4: default `true` means a fresh install logs at `debug`. Debug lines must stay secret-free (hygiene script runs in the gate). `reload::Layer` type parameters are fiddly; keep the handle type behind a type alias in `logging.rs`.
- Task 7: the hook runs on the launch path; a slow disk makes launch slower by one config write. Never let a sync failure block the launch. Idempotency: the RetroArch writer overwrites managed keys only.
- Task 6: extraction of a hostile archive — reuse the existing extractor (it already guards path traversal; verify in task). Requires a library path; without one, return the existing verbatim error.
- Task 8: holding the command for 500 ms delays the UI's "launched" feedback by that much; acceptable (Python does the same). Dropping a `Child` without `wait` leaves a zombie until the process exits on unix — verify whether the existing standalone path already accepts that (it returned `()` and kept no handle, so yes).
- Task 3: GitHub rate limits (cache + manual button); an update while the emulator is running (refuse with a clear error if a session uses that entry — verify `LaunchService` exposes the running emulator names; if not, skip and note); the install-dir name keeps the old tag; a changed asset layout may move the executable — detection must re-run, not assume the old relative path. Never delete `portable/`, `user/`, `saves/`, `memstick/`, `bios/`, `cores/` content on update.
- Task 11: `directories::UserDirs::document_dir()` can be `None` on a broken profile; treat as no override.
- Task 12: `cargo clean --profile dev` after the e2e run, not before (the e2e build is a debug build).

## Open Questions

1. Task 2 — should the two danger icon buttons (`Details.svelte .dismiss`, `CloudPanel.svelte .remove`) also turn `--primary` on hover, or keep `--danger`? Plan default: keep `--danger`; the ruling's "glyph turns primary" is applied to the neutral close buttons. Designer confirms.
2. Task 3 — the install directory is `Emulators/<name>-<configured tag>/`; for a `latest` pin it is stable across updates (same as Python). For a PINNED tag, changing the pin creates a sibling directory (Python does the same). Accepted; no rename.
3. Task 8 — should the game-launch early-exit warning ALSO toast (in addition to the Details strip) for symmetry with the standalone toast? Plan default: no; one surface each, one shared message builder.
4. Task 4 — the toggle lives on the Appearance page to mirror Python (grid-launcher.py:1714). A separate "Advanced" page would need a new rail entry in `settings/pages.ts`; not planned.
5. Task 3 — Python checks on click with no cache; the plan follows that (no view-open check). Confirm the confirm-dialog surface: the app has no modal Yes/No component (verify) — the plan uses the two-click confirm pattern the Delete button uses.

## Verification after all tasks

1. Full gate list (Rust, hygiene, frontend, e2e typecheck), then the full `scripts/e2e.sh`.
2. Hand-test notes: a Linux-platform game's Play runs its executable directly; hover an icon button; force an error toast and hear it announced; unset `xdg-desktop-portal` (or run under the e2e build) and press Browse once; toggle debug prints; open Emulators with a source-installed emulator whose forge has a newer tag and update it; add an emulator by pointing the form at a zip; launch a RetroArch entry and diff `retroarch.cfg` before/after (managed keys only); launch a stub that exits at once from the Emulators pane and read the toast.

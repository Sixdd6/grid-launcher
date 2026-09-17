# Vulkan renderer seed — implementation plan (2026-09-17)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal**

A fresh emulator install or manual add from GRID starts Cemu, Dolphin, DuckStation, PPSSPP, Azahar and xemu on their Vulkan renderer. The seed never overwrites a value already in the file, never runs for an entry that existed before the pass, and never runs on macOS. Cemu's `settings.xml` template stops pinning OpenGL.

**Architecture**

`autoconfig::SyncContext` (crates/grid-core/src/autoconfig/mod.rs:392-418) gains `fresh_install: bool`. The two `sync_new_emulator` callers already know the answer: `InstallService::finalize_emulator` (crates/grid-core/src/library/mod.rs:2317 `let fresh = self.write_emulator_entry(...)`) and `save_emulator` (app/src-tauri/src/commands.rs:1069 `if is_add { run_emulator_sync(...) }`). `sync_new_emulator`'s flat dispatch (mod.rs:617-723) hands the flag to six writers as one trailing `bool` parameter on their full writer; each writer adds the renderer key inside its existing section pass, per-key preserve, gated by `fresh_install && cfg!(not(target_os = "macos"))`. Narrow writers (`ensure_ra_credentials`, `ensure_skip_ipl`, `ensure_gcpad_config`, `ensure_controller_config`) do not change. Dependency direction stays `app/src-tauri` -> `grid-core`; grid-core never depends on Tauri.

**Tech Stack**

Rust (grid-core writers, `regex` crate already a dependency of cemu.rs), Tauri commands (app/src-tauri), WebdriverIO e2e (TypeScript), Markdown docs.

**Spec**

`/home/six/Documents/Programming/grid-launcher/docs/superpowers/specs/2026-09-17-vulkan-renderer-seed-design.md`

**Global Constraints**

- Fresh-install only: `fresh_install == false` never adds a renderer key. A catalog update or a reinstall into an existing entry is `fresh == false`.
- Per-key preserve: the key is written only when absent from the section in the file as it was BEFORE this call wrote. A present value, whatever it is, survives.
- Never on macOS: every seed is gated by `cfg!(not(target_os = "macos"))`.
- Exact literals: Cemu `<Graphic><api>1</api>` (the template constant changes to `1`; the merge branch adds `<api>1</api>` inside an existing `<Graphic>` lacking `<api>`; when `<Graphic>` is absent it creates `<Graphic><api>1</api></Graphic>` before `</content>`, not the full template block); Dolphin `[Core] GFXBackend = Vulkan` in `Dolphin.ini`; DuckStation `[GPU] Renderer = Vulkan`; PPSSPP `[Graphics] GraphicsBackend = 3 (VULKAN)`; Azahar `[Renderer]` `graphics_api\default=false` and `graphics_api=2`; xemu `[display] renderer = 'VULKAN'`.
- Idempotent: a second `fresh_install == true` call over a file that already carries the key reports `changed == false`.
- Secrets never in logs, errors, IPC, fixtures or console output (`scripts/check_secret_hygiene.sh` is part of the gate). Nothing in this plan touches a credential; keep it that way.
- Run `cargo fmt` before every commit. Commit with `git commit --only -- <paths>` and the trailer `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Never run `git checkout`, `git restore`, `git reset`, or `git stash` on tracked files.
- Test commands: `cargo test -p grid-core --lib <filter>` for grid-core; `cargo test -p app` for the Tauri side; at the end `cargo clippy --workspace --all-targets -- -D warnings` and `cargo clippy -p app --all-targets --features e2e -- -D warnings`.

**Verified current state (read before planning)**

- `SyncContext` has six fields: `config_path`, `platforms`, `platform_slugs`, `ps3_library_path`, `ra`, `profiles` (mod.rs:392-418).
- `SyncContext { ... }` literals exist at: app/src-tauri/src/commands.rs:266 (backfill in `list_platforms`) and :939 (`run_emulator_sync`); crates/grid-core/src/library/mod.rs:2357 (`sync_autoconfig`); crates/grid-core/src/autoconfig/mod.rs test module lines 940, 980, 1023, 1073, 1258, 1289, 1323, 1366, 1405, 1442, 1480, 1523, 1558, 1600, 1644, 1680, 1947, 1998 (18 literals). No other crate constructs one.
- Dispatch calls today (mod.rs): `duckstation::ensure_memory_card_settings(path, true)` :630; `xemu::ensure_settings(path)` :634; `dolphin::ensure_settings(path)` :659; `azahar::ensure_settings(path)` :662; `ppsspp::ensure_settings(path, ra)` :680; `cemu::ensure_settings(path)` :684.
- Writer signatures today: `cemu.rs:435 pub fn ensure_settings(emulator_path: &str) -> EnsureResult`; `dolphin.rs:199 pub fn ensure_settings(emulator_path: &str) -> EnsureResult`; `duckstation.rs:210 pub fn ensure_memory_card_settings(emulator_path: &str, enable_fullscreen: bool) -> EnsureResult`; `ppsspp.rs:185 pub fn ensure_settings(emulator_path: &str, ra: Option<&RaCredentials>) -> EnsureResult`; `azahar.rs:127 pub fn ensure_settings(emulator_path: &str) -> EnsureResult`; `xemu.rs:170 pub fn ensure_settings(emulator_path: &str) -> EnsureResult`.
- `writers::section_has_key(raw, section, key) -> bool` (writers.rs:561) probes with `NARROW_KEY_RE = ^\s*([A-Za-z0-9_]+)\s*=` (writers.rs:53-54), so Azahar's `graphics_api\default=false` companion line does NOT satisfy a probe for `graphics_api`; only a real `graphics_api=...` line does.
- `ini_overwrite_section`, `azahar_section`, `toml_add_only_section` all have signature `(raw: &str, section: &str, desired: &Desired) -> (String, bool)`; `Desired = Vec<(String, String)>`; `crate::desired![("K","v"), ...]` builds one.
- ARCHITECTURE.md mentions `autoconfig` only at line 28 ("the `ensure_*` writers that seed an emulator's own settings files") and never names `SyncContext`, so the spec's conditional ARCHITECTURE.md sentence does not apply.
- e2e: no stage installs one of the six seeded emulators from the catalog (`emulator-catalog` installs PCSX2 and Redream; `emulators` adds RetroArch by hand and a DuckStation row at `/nonexistent/duckstation`, which no writer can reach). Task 9 turns that DuckStation case into the assertion by pointing it at a real stub in its own directory; `delete_emulator` leaves hand-configured paths on disk (commands.rs:1100-1102), so the stub survives the case's cleanup.

---

## Task 1: `SyncContext.fresh_install` plumbing (no behavior change)

**Files**
- Modify: `crates/grid-core/src/autoconfig/mod.rs` (struct :392-418; 18 test literals at :940, :980, :1023, :1073, :1258, :1289, :1323, :1366, :1405, :1442, :1480, :1523, :1558, :1600, :1644, :1680, :1947, :1998)
- Modify: `crates/grid-core/src/library/mod.rs` (:2317-2318 call, :2351-2364 `sync_autoconfig`)
- Modify: `app/src-tauri/src/commands.rs` (:266-273, :932-946, :974-979, :1070-1075)
- Test: `crates/grid-core/src/autoconfig/mod.rs` test module

**Interfaces**
- Produces: `pub struct SyncContext<'a> { ..., pub fresh_install: bool }`
- Produces: `fn sync_autoconfig(&self, entry_name: &str, fresh_install: bool, warning: &mut String)` (library/mod.rs)
- Produces: `pub fn run_emulator_sync(entry_name: &str, library_path: &str, inputs: SyncInputs, fresh_install: bool, pass: SyncPass)` (commands.rs)
- Consumes: nothing new.

**Steps**

- [ ] 1. Write the failing test. Append inside `mod tests` in `crates/grid-core/src/autoconfig/mod.rs`, right after the `config_with` fixture (after line 919):

```rust
    /// The renderer seed's gate. Every caller states it explicitly; there is
    /// no default, so a new call site cannot forget it.
    #[test]
    fn a_sync_context_carries_the_fresh_install_flag() {
        let profiles: Vec<EmulatorProfile> = Vec::new();
        let config_path = PathBuf::from("unused.toml");
        let ctx = SyncContext {
            config_path: &config_path,
            platforms: &[],
            platform_slugs: &no_slugs(),
            ps3_library_path: String::new(),
            ra: None,
            profiles: &profiles,
            fresh_install: true,
        };
        assert!(ctx.fresh_install);
    }
```

- [ ] 2. Run `cargo test -p grid-core --lib a_sync_context_carries_the_fresh_install_flag`. Expected: compile error `E0560: struct SyncContext<'_> has no field named fresh_install`.

- [ ] 3. Add the field. In `crates/grid-core/src/autoconfig/mod.rs`, after the `profiles` field (line 417) inside `pub struct SyncContext<'a>`:

```rust
    /// `true` only for an entry that did not exist before this pass: a
    /// catalog install that created it, or a manual add. A reinstall, a
    /// catalog update and a defaults backfill are `false`. The renderer seed
    /// (Vulkan on Cemu, Dolphin, DuckStation, PPSSPP, Azahar, xemu) runs only
    /// when this is `true`; nothing else reads it.
    pub fresh_install: bool,
```

- [ ] 4. Update the 18 test literals in the same file. In each `SyncContext { ... }` at lines 940, 980, 1023, 1073, 1258, 1289, 1323, 1366, 1405, 1442, 1480, 1523, 1558, 1600, 1644, 1680, 1947 and 1998, add the line `fresh_install: false,` directly after the `profiles: ...,` line. (`false` keeps every existing assertion exactly as it is; the tests that need `true` come in Task 8.) The `sync` helper at :1997-2007 becomes:

```rust
    fn sync(config_path: &Path, entry_name: &str) -> SyncReport {
        let ctx = SyncContext {
            config_path,
            platforms: &[],
            platform_slugs: &no_slugs(),
            ps3_library_path: String::new(),
            ra: None,
            profiles: crate::launch::profiles::load_profiles(),
            fresh_install: false,
        };
        sync_new_emulator(entry_name, &ctx).unwrap()
    }
```

- [ ] 5. Thread the flag through the install service. In `crates/grid-core/src/library/mod.rs` replace lines 2317-2318:

```rust
        let fresh = self.write_emulator_entry(job, &paths.resolved, &exe)?;
        self.sync_autoconfig(&job.profile_name, fresh, warning);
```

and replace the `sync_autoconfig` signature and context literal (:2351-2364):

```rust
    /// D1 call site A: runs [`autoconfig::sync_new_emulator`] for the entry
    /// the install just wrote, before the archive cleanup.
    ///
    /// `fresh_install` is [`Self::write_emulator_entry`]'s answer: `true`
    /// when this install CREATED the entry, `false` for an update or a
    /// reinstall — the renderer seed runs only on the former.
    ///
    /// Autoconfig NEVER fails an install. A config error, or any writer that
    /// reached nothing, appends ONE line to the finalize warning — exactly
    /// like a failed archive delete — and the install still reports
    /// `Completed`. No credential can appear in that line: the report names
    /// emulators and writers only.
    fn sync_autoconfig(&self, entry_name: &str, fresh_install: bool, warning: &mut String) {
        let library_path = Config::load(&self.config_path)
            .map(|config| config.library_path)
            .unwrap_or_default();
        let platforms = self.known_platforms();
        let platform_slugs = self.platform_slugs();
        let ctx = autoconfig::SyncContext {
            config_path: &self.config_path,
            platforms: &platforms,
            platform_slugs: &platform_slugs,
            ps3_library_path: autoconfig::ps3_library_path(&library_path),
            ra: self.ra_credentials(),
            profiles: &self.profiles,
            fresh_install,
        };
```

(The rest of the function body — the `match autoconfig::sync_new_emulator(entry_name, &ctx)` — is unchanged.)

- [ ] 6. Thread the flag through the Tauri commands. In `app/src-tauri/src/commands.rs`:

  a. The backfill context at :266-273 — add `fresh_install: false,` after `profiles,`:

```rust
                let ctx = autoconfig::SyncContext {
                    config_path: &config_path,
                    platforms: &assignable,
                    platform_slugs: &slugs,
                    ps3_library_path: String::new(),
                    ra,
                    profiles,
                    fresh_install: false,
                };
```

  b. `run_emulator_sync` at :929-946 becomes:

```rust
/// Runs `pass` for one entry and logs its outcome — a sync never fails its
/// caller. `library_path` feeds RPCS3's PS3 library path only.
/// `fresh_install` is the renderer seed's gate: `true` only for a manual ADD
/// (`save_emulator`); the launch-time RetroArch-only pass ignores it and
/// passes `false`. Blocking: the writers touch emulator config files.
pub fn run_emulator_sync(
    entry_name: &str,
    library_path: &str,
    inputs: SyncInputs,
    fresh_install: bool,
    pass: SyncPass,
) {
    let SyncInputs {
        platforms,
        platform_slugs,
        ra,
    } = inputs;
    let config_path = Config::default_path();
    let ctx = autoconfig::SyncContext {
        config_path: &config_path,
        platforms: &platforms,
        platform_slugs: &platform_slugs,
        ps3_library_path: autoconfig::ps3_library_path(library_path),
        ra,
        profiles: load_profiles(),
        fresh_install,
    };
```

  c. The launch-time call in `sync_emulator_settings` (:974-979):

```rust
    run_emulator_sync(
        entry_name,
        &library_path,
        SyncInputs::from_install(install),
        false,
        autoconfig::sync_retroarch_settings_only,
    );
```

  d. The manual-add call in `save_emulator` (:1069-1076):

```rust
            if is_add {
                run_emulator_sync(
                    &saved_name,
                    &library_path,
                    inputs,
                    true,
                    autoconfig::sync_new_emulator,
                );
            }
```

- [ ] 7. Run `cargo test -p grid-core --lib autoconfig` and `cargo test -p app`. Expected: everything compiles; all existing tests pass; the new test passes.

- [ ] 8. `cargo fmt`, then commit:

```
git commit --only -- crates/grid-core/src/autoconfig/mod.rs crates/grid-core/src/library/mod.rs app/src-tauri/src/commands.rs -m "$(cat <<'EOF'
rewrite: SyncContext carries fresh_install; both sync_new_emulator callers set it

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Cemu — template `<api>1</api>` and the `<Graphic>` seed

**Files**
- Modify: `crates/grid-core/src/autoconfig/cemu.rs` (:131 `DEFAULT_CEMU_SETTINGS_XML`, :188-189 `<Graphic>\n<api>0</api>`, :264 `CONTENT_CLOSE`, :358-377 `apply_forced_elements`, :422-477 `ensure_settings`, test module :529-828)
- Modify: `crates/grid-core/src/autoconfig/mod.rs:684` dispatch
- Test: `crates/grid-core/src/autoconfig/cemu.rs` test module

**Interfaces**
- Produces: `pub fn ensure_settings(emulator_path: &str, fresh_install: bool) -> EnsureResult`
- Produces (private): `fn apply_forced_elements(content: &str, seed_renderer: bool) -> Option<(String, bool)>`, `fn seed_graphic_api(root_span: &mut String) -> bool`
- Consumes: `CONTENT_CLOSE`, `find_root_span`, `regex::Regex`, `std::sync::LazyLock` (all already in the module).

**Steps**

- [ ] 1. Write the failing tests. Append inside `mod tests` in `cemu.rs`, after `cemu_settings_xml_empty_after_trim_yields_unchanged` (after line 639):

```rust
    // --- renderer seed (spec 2026-09-17-vulkan-renderer-seed-design) --------

    /// Every forced element already at its value, so the ONLY thing a fresh
    /// call can still change is the renderer seed.
    const SIX_FORCED: &str = "<use_discord_presence>false</use_discord_presence>\n<check_update>false</check_update>\n<receive_untested_updates>false</receive_untested_updates>\n<gp_download>true</gp_download>\n<fullscreen>false</fullscreen>\n<window_maximized>true</window_maximized>\n";

    fn settled_body(extra: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<content>\n{SIX_FORCED}{extra}</content>\n"
        )
    }

    #[test]
    fn cemu_template_pins_the_vulkan_renderer() {
        assert!(DEFAULT_CEMU_SETTINGS_XML.contains("<api>1</api>"));
        assert!(!DEFAULT_CEMU_SETTINGS_XML.contains("<api>0</api>"));
        assert!(
            DEFAULT_CEMU_SETTINGS_XML.contains("<api>3</api>"),
            "the Audio api is a different element and stays 3"
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn cemu_fresh_install_seeds_vulkan_inside_an_existing_graphic_element() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        let target = write_settings(
            &dir,
            &settled_body(
                "<Graphic>\n<device>abc</device>\n</Graphic>\n<Audio><api>3</api></Audio>\n",
            ),
        );

        let result = ensure_settings(exe.to_str().unwrap(), true);

        assert!(result.changed);
        let text = std::fs::read_to_string(&target).unwrap();
        assert!(
            text.contains("<Graphic><api>1</api>\n<device>abc</device>"),
            "the api child goes first inside <Graphic>: {text}"
        );
        assert_eq!(text.matches("<api>1</api>").count(), 1, "{text}");
        assert!(
            text.contains("<Audio><api>3</api></Audio>"),
            "the Audio api is not the Graphic api: {text}"
        );
    }

    #[test]
    fn cemu_fresh_install_keeps_an_existing_graphic_api_value() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        let body = settled_body("<Graphic><api>0</api></Graphic>\n");
        let target = write_settings(&dir, &body);

        let result = ensure_settings(exe.to_str().unwrap(), true);

        assert!(!result.changed);
        let text = std::fs::read_to_string(&target).unwrap();
        assert!(text.contains("<api>0</api>"));
        assert!(!text.contains("<api>1</api>"));
        assert_eq!(text, body, "no write at all");
    }

    #[test]
    fn cemu_non_fresh_call_never_adds_the_graphic_api() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        let body = settled_body("<Graphic><device>abc</device></Graphic>\n");
        let target = write_settings(&dir, &body);

        let result = ensure_settings(exe.to_str().unwrap(), false);

        assert!(!result.changed);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), body);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn cemu_second_fresh_call_is_a_no_op() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        write_settings(&dir, &settled_body("<Graphic><device>abc</device></Graphic>\n"));

        let first = ensure_settings(exe.to_str().unwrap(), true);
        let second = ensure_settings(exe.to_str().unwrap(), true);

        assert!(first.changed);
        assert!(!second.changed);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn cemu_fresh_install_creates_a_graphic_element_with_only_the_api_when_absent() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        let target = write_settings(&dir, &settled_body(""));

        let result = ensure_settings(exe.to_str().unwrap(), true);

        assert!(result.changed);
        let text = std::fs::read_to_string(&target).unwrap();
        assert!(
            text.contains("<Graphic><api>1</api></Graphic></content>"),
            "created right before the root close, api child only: {text}"
        );
        assert_eq!(text.matches("<Graphic>").count(), 1, "{text}");
        assert!(!text.contains("<device>"), "not the full template block: {text}");
    }
```

- [ ] 2. Run `cargo test -p grid-core --lib cemu`. Expected: compile error `E0061: this function takes 1 argument but 2 arguments were supplied` on the new tests.

- [ ] 3. Change the template. In `DEFAULT_CEMU_SETTINGS_XML` (cemu.rs:188-189) replace

```
    <Graphic>
        <api>0</api>
```
with
```
    <Graphic>
        <api>1</api>
```

Update the constant's doc comment (cemu.rs:127-130) to:

```rust
/// cemu.py:115-237, transcribed verbatim EXCEPT `<Graphic><api>`, which is
/// `1` (Vulkan) here where the reference pinned `0` (OpenGL): Cemu's own
/// default has been Vulkan since 1.22.10, and the OpenGL pin booted Breath
/// of the Wild to a black screen. First line the UPPERCASE-`UTF-8` XML
/// declaration, last line `</content>`, exactly one trailing newline. The
/// six forced elements already carry their desired values here, so the
/// create-from-template branch never needs to touch this text.
```

- [ ] 4. Add the seed helper. Insert after `apply_forced_elements` (after cemu.rs:377):

```rust
/// The `<Graphic>...</Graphic>` element, matched on the literal open tag WITH
/// its `>` so the sibling `<GraphicPack/>` can never match. Group 1 is the
/// inner text; its start is where the `<api>` child goes.
static GRAPHIC_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<Graphic>(.*?)</Graphic>").expect("static regex is valid"));

/// An `<api>` child, probed ONLY inside `<Graphic>`'s inner text — `<Audio>`
/// has its own `<api>` and must not satisfy the probe.
static GRAPHIC_API_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<api>[^<]*</api>").expect("static regex is valid"));

/// Cemu's renderer enum: `1` is Vulkan.
const GRAPHIC_API_VULKAN: &str = "<api>1</api>";

/// The renderer seed for the MERGE branch (spec
/// `2026-09-17-vulkan-renderer-seed-design.md`): add `<api>1</api>` inside
/// `<Graphic>` when that element has no `<api>` child; when `<Graphic>` is
/// absent (or self-closing) create it with the `<api>` child ONLY, right
/// before `</content>` — never the full template block. Per-key preserve:
/// any existing `<api>` value, `0` included, is left alone. Returns whether
/// `root_span` changed.
fn seed_graphic_api(root_span: &mut String) -> bool {
    if let Some(caps) = GRAPHIC_RE.captures(root_span.as_str()) {
        let inner = caps.get(1).unwrap();
        if GRAPHIC_API_RE.is_match(inner.as_str()) {
            return false;
        }
        let insert_at = inner.start();
        root_span.insert_str(insert_at, GRAPHIC_API_VULKAN);
        return true;
    }
    if let Some(pos) = root_span.find("<Graphic/>") {
        root_span.replace_range(
            pos..pos + "<Graphic/>".len(),
            "<Graphic><api>1</api></Graphic>",
        );
        return true;
    }
    let insert_at = root_span.len() - CONTENT_CLOSE.len();
    root_span.insert_str(insert_at, "<Graphic><api>1</api></Graphic>");
    true
}
```

- [ ] 5. Thread the seed through `apply_forced_elements`. Replace cemu.rs:358-377 with:

```rust
/// D11: locate the `<content>...</content>` root via [`find_root_span`],
/// apply the six forced elements in order, then — only when `seed_renderer`
/// — the `<Graphic><api>` seed ([`seed_graphic_api`]), and return the
/// (possibly edited) root span plus whether anything changed. `None` on any
/// parse failure: empty (after trim) content, no un-commented `<content>`
/// tag, or a close tag that doesn't follow the open tag — cemu.py:308-312's
/// `root is None` / `ET.ParseError` branch.
fn apply_forced_elements(content: &str, seed_renderer: bool) -> Option<(String, bool)> {
    if content.trim().is_empty() {
        return None;
    }
    let (open_pos, close_end) = find_root_span(content)?;

    let mut root_span = content[open_pos..close_end].to_string();
    let mut changed = false;
    for ((tag, value), re) in FORCED_ELEMENTS.iter().zip(FORCED_ELEMENT_REGEXES.iter()) {
        changed |= set_or_insert_element(&mut root_span, tag, value, re);
    }
    if seed_renderer {
        changed |= seed_graphic_api(&mut root_span);
    }

    Some((root_span, changed))
}
```

- [ ] 6. Change `ensure_settings`. Replace the doc comment and signature at cemu.rs:422-435 with:

```rust
/// `ensure_cemu_settings` (cemu.py:286-329). Blank path (after `.trim()`) or
/// a resolved `emulator_dir` with no path text at all returns
/// [`EnsureResult::unchanged`].
///
/// Creates `<emulator_dir>/portable/` unconditionally before any file
/// check, targeting `<emulator_dir>/portable/settings.xml`. When that file
/// is missing, [`DEFAULT_CEMU_SETTINGS_XML`] is written and `changed = true`
/// is reported immediately. Otherwise the file is parsed via
/// [`apply_forced_elements`] (D11) and rewritten only when something
/// changed, as `<?xml version="1.0" encoding="utf-8"?>\n` (lowercase here,
/// unlike the template) followed by the edited root, with no added trailing
/// newline. **Every** failure — parse error and I/O alike — yields
/// [`EnsureResult::unchanged`] (cemu.py:326-327's bare `except Exception`).
///
/// `fresh_install` gates the renderer seed on the merge branch
/// ([`seed_graphic_api`]): only a fresh install, and never on macOS, adds
/// `<Graphic><api>1</api>` to an existing file lacking it. The
/// create-from-template branch is unaffected — the template already says
/// `1`.
pub fn ensure_settings(emulator_path: &str, fresh_install: bool) -> EnsureResult {
```

and replace line 465:

```rust
    let seed_renderer = fresh_install && cfg!(not(target_os = "macos"));
    let Some((new_root_span, changed)) = apply_forced_elements(&content, seed_renderer) else {
```

- [ ] 7. Update the existing test call sites in `cemu.rs` — every `ensure_settings(exe.to_str().unwrap())` at lines 555, 573, 595, 611, 623, 635, 676, 697 becomes `ensure_settings(exe.to_str().unwrap(), false)`.

- [ ] 8. Update the dispatch. In `crates/grid-core/src/autoconfig/mod.rs:684` replace

```rust
        record(&mut report, name, "cemu", cemu::ensure_settings(path));
```
with
```rust
        record(
            &mut report,
            name,
            "cemu",
            cemu::ensure_settings(path, ctx.fresh_install),
        );
```

- [ ] 9. Run `cargo test -p grid-core --lib cemu` and `cargo test -p grid-core --lib autoconfig`. Expected: all pass, including `cemu_creates_the_default_settings_xml_when_missing` (byte-for-byte against the updated template).

- [ ] 10. `cargo fmt`, then commit:

```
git commit --only -- crates/grid-core/src/autoconfig/cemu.rs crates/grid-core/src/autoconfig/mod.rs -m "$(cat <<'EOF'
rewrite: Cemu template defaults to Vulkan; fresh installs seed <Graphic><api>1</api> with per-key preserve

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Dolphin — `[Core] GFXBackend = Vulkan`

**Files**
- Modify: `crates/grid-core/src/autoconfig/dolphin.rs` (:180-255 `ensure_settings`, test module :327-622)
- Modify: `crates/grid-core/src/autoconfig/mod.rs:659` dispatch
- Test: `crates/grid-core/src/autoconfig/dolphin.rs` test module

**Interfaces**
- Produces: `pub fn ensure_settings(emulator_path: &str, fresh_install: bool) -> EnsureResult`
- Consumes: `writers::section_has_key(raw: &str, section: &str, key: &str) -> bool`, `writers::ini_overwrite_section(raw: &str, section: &str, desired: &Desired) -> (String, bool)`, `crate::desired!`.

**Steps**

- [ ] 1. Write the failing tests. Append inside `mod tests` in `dolphin.rs`, after `dolphin_uses_candidate_zero_when_a_path_is_given` (after line 445):

```rust
    // --- renderer seed (spec 2026-09-17-vulkan-renderer-seed-design) --------

    fn dolphin_ini(dir: &Path) -> PathBuf {
        dir.join("User").join("Config").join("Dolphin.ini")
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn dolphin_fresh_install_seeds_the_vulkan_backend() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());

        let result = ensure_settings(exe.to_str().unwrap(), true);

        assert!(result.changed);
        let text = std::fs::read_to_string(dolphin_ini(&dir)).unwrap();
        assert!(text.contains("[Core]"), "{text}");
        assert!(text.contains("GFXBackend = Vulkan"), "{text}");
    }

    #[test]
    fn dolphin_fresh_install_keeps_an_existing_backend() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());
        let ini = dolphin_ini(&dir);
        std::fs::create_dir_all(ini.parent().unwrap()).unwrap();
        std::fs::write(&ini, "[Core]\nGFXBackend = OGL\n").unwrap();

        ensure_settings(exe.to_str().unwrap(), true);

        let text = std::fs::read_to_string(&ini).unwrap();
        assert!(text.contains("GFXBackend = OGL"), "{text}");
        assert!(!text.contains("Vulkan"), "{text}");
    }

    #[test]
    fn dolphin_non_fresh_call_never_adds_the_backend() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());

        ensure_settings(exe.to_str().unwrap(), false);

        let text = std::fs::read_to_string(dolphin_ini(&dir)).unwrap();
        assert!(!text.contains("GFXBackend"), "{text}");
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn dolphin_second_fresh_call_is_a_no_op() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, _dir) = make_exe(temp.path());

        let first = ensure_settings(exe.to_str().unwrap(), true);
        let second = ensure_settings(exe.to_str().unwrap(), true);

        assert!(first.changed);
        assert!(!second.changed);
    }
```

- [ ] 2. Run `cargo test -p grid-core --lib dolphin`. Expected: compile error `E0061` (1 argument expected, 2 supplied).

- [ ] 3. Implement. Replace `ensure_settings` in `dolphin.rs:180-255` with:

```rust
/// `ensure_dolphin_settings` (dolphin.py:253-315): writes `Dolphin.ini` and
/// `GFX.ini` as two independently fallible scopes.
///
/// Creates `portable.txt` first (see [`maybe_create_portable_txt`]).
/// **Selection rule:** when `emulator_path.trim()` is non-blank,
/// `candidates[0]` is used UNCONDITIONALLY for both files — the portable
/// `User/Config` path; otherwise the first EXISTING candidate, falling back
/// to `candidates[0]`.
///
/// `Dolphin.ini`'s forced overwrites, in order: `[Analytics] Enabled` =
/// `False`, `PermissionAsked` = `True`; `[Display] Fullscreen` = `True`,
/// `RenderToMain` = `True`; `[General] ShowLaunchWarning` = `False`;
/// `[DSP] Volume` = `70`. `GFX.ini`: `[Settings] UseVerticalSync` = `True`.
/// Capitalized `True`/`False` is Dolphin's own convention.
///
/// Renderer seed (spec `2026-09-17-vulkan-renderer-seed-design.md`): on a
/// `fresh_install`, off macOS, `[Core] GFXBackend = Vulkan` is added to
/// `Dolphin.ini` ONLY when the file as read has no `GFXBackend` under
/// `[Core]` — per-key preserve, the way PCSX2's `[EmuCore/GS]` block works.
///
/// A read/write failure on one file sets its own path to `None` in the
/// result WITHOUT aborting the other. `config_path` is the `Dolphin.ini`
/// path; `extras["gfx_ini_path"]` is the `GFX.ini` path; `changed` is the OR
/// of both files' writes, true only when a write actually happened.
pub fn ensure_settings(emulator_path: &str, fresh_install: bool) -> EnsureResult {
    maybe_create_portable_txt(emulator_path);

    let force_first = !emulator_path.trim().is_empty();
    let seed_renderer = fresh_install && cfg!(not(target_os = "macos"));
    let mut changed = false;
    let mut config_path: Option<PathBuf> = None;
    let mut extras = std::collections::BTreeMap::new();

    let dolphin_candidates = ini_path_candidates(emulator_path, "Dolphin.ini");
    if !dolphin_candidates.is_empty() {
        let selected = select_candidate(&dolphin_candidates, force_first);
        if let Ok(file_changed) = write_if_changed(&selected, |original| {
            let (content, c1) = writers::ini_overwrite_section(
                original,
                "Analytics",
                &crate::desired![("Enabled", "False"), ("PermissionAsked", "True")],
            );
            let (content, c2) = writers::ini_overwrite_section(
                &content,
                "Display",
                &crate::desired![("Fullscreen", "True"), ("RenderToMain", "True")],
            );
            let (content, c3) = writers::ini_overwrite_section(
                &content,
                "General",
                &crate::desired![("ShowLaunchWarning", "False")],
            );
            let (content, c4) =
                writers::ini_overwrite_section(&content, "DSP", &crate::desired![("Volume", "70")]);
            // The probe reads `original` — the file BEFORE this call's own
            // edits — like every preserve probe in this crate.
            let (content, c5) =
                if seed_renderer && !writers::section_has_key(original, "Core", "GFXBackend") {
                    writers::ini_overwrite_section(
                        &content,
                        "Core",
                        &crate::desired![("GFXBackend", "Vulkan")],
                    )
                } else {
                    (content, false)
                };
            (content, c1 || c2 || c3 || c4 || c5)
        }) {
            changed = changed || file_changed;
            config_path = Some(selected);
        }
    }

    let gfx_candidates = ini_path_candidates(emulator_path, "GFX.ini");
    if !gfx_candidates.is_empty() {
        let selected = select_candidate(&gfx_candidates, force_first);
        if let Ok(file_changed) = write_if_changed(&selected, |content| {
            writers::ini_overwrite_section(
                content,
                "Settings",
                &crate::desired![("UseVerticalSync", "True")],
            )
        }) {
            changed = changed || file_changed;
            extras.insert("gfx_ini_path".to_string(), selected);
        }
    }

    EnsureResult {
        changed,
        config_path,
        extras,
    }
}
```

- [ ] 4. Update the existing test call sites in `dolphin.rs`: `ensure_settings(exe.to_str().unwrap())` at lines 360, 394, 407, 433, 493 becomes `ensure_settings(exe.to_str().unwrap(), false)`.

- [ ] 5. Update the dispatch. In `mod.rs:659` replace

```rust
        record(&mut report, name, "dolphin", dolphin::ensure_settings(path));
```
with
```rust
        record(
            &mut report,
            name,
            "dolphin",
            dolphin::ensure_settings(path, ctx.fresh_install),
        );
```

- [ ] 6. Run `cargo test -p grid-core --lib dolphin` and `cargo test -p grid-core --lib autoconfig`. Expected: all pass.

- [ ] 7. `cargo fmt`, then commit:

```
git commit --only -- crates/grid-core/src/autoconfig/dolphin.rs crates/grid-core/src/autoconfig/mod.rs -m "$(cat <<'EOF'
rewrite: Dolphin fresh installs seed [Core] GFXBackend = Vulkan with per-key preserve

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: DuckStation — `[GPU] Renderer = Vulkan`

**Files**
- Modify: `crates/grid-core/src/autoconfig/duckstation.rs` (:197-210 doc + signature, :325-343 `[GPU]` loop, test module :444-854)
- Modify: `crates/grid-core/src/autoconfig/mod.rs:625-632` dispatch
- Test: `crates/grid-core/src/autoconfig/duckstation.rs` test module

**Interfaces**
- Produces: `pub fn ensure_memory_card_settings(emulator_path: &str, enable_fullscreen: bool, fresh_install: bool) -> EnsureResult`
- Consumes: `writers::section_has_key`, the existing `gpu_desired` / `apply_section` pattern.

**Steps**

- [ ] 1. Write the failing tests. Append inside `mod tests` in `duckstation.rs`, after `duckstation_is_idempotent` (after line 853):

```rust
    // --- renderer seed (spec 2026-09-17-vulkan-renderer-seed-design) --------

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn duckstation_fresh_install_seeds_the_vulkan_renderer() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, _, config_path) = setup_emulator(temp.path());

        let result = ensure_memory_card_settings(&emulator_path, false, true);

        assert!(result.changed);
        let text = std::fs::read_to_string(&config_path).unwrap();
        assert!(text.contains("[GPU]"), "{text}");
        assert!(text.contains("Renderer = Vulkan"), "{text}");
    }

    #[test]
    fn duckstation_fresh_install_keeps_an_existing_renderer() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, _, config_path) = setup_emulator(temp.path());
        std::fs::write(&config_path, "[GPU]\nRenderer = OpenGL\n").unwrap();

        ensure_memory_card_settings(&emulator_path, false, true);

        let text = std::fs::read_to_string(&config_path).unwrap();
        assert!(text.contains("Renderer = OpenGL"), "{text}");
        assert!(!text.contains("Renderer = Vulkan"), "{text}");
        assert!(
            text.contains("ResolutionScale = 4"),
            "the other GPU defaults still land: {text}"
        );
    }

    #[test]
    fn duckstation_non_fresh_call_never_adds_the_renderer() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, _, config_path) = setup_emulator(temp.path());

        ensure_memory_card_settings(&emulator_path, false, false);

        let text = std::fs::read_to_string(&config_path).unwrap();
        assert!(text.contains("[GPU]"), "{text}");
        assert!(!text.contains("Renderer ="), "{text}");
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn duckstation_second_fresh_call_is_a_no_op() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (emulator_path, _, _) = setup_emulator(temp.path());

        let first = ensure_memory_card_settings(&emulator_path, true, true);
        let second = ensure_memory_card_settings(&emulator_path, true, true);

        assert!(first.changed);
        assert!(!second.changed);
    }
```

- [ ] 2. Run `cargo test -p grid-core --lib duckstation`. Expected: compile error `E0061` (2 arguments expected, 3 supplied).

- [ ] 3. Implement. Replace the doc comment and signature at `duckstation.rs:197-210`:

```rust
/// `ensure_duckstation_memory_card_settings` (duckstation.py:198-386).
///
/// Every preserve-if-present probe below reads the ORIGINAL, pre-write
/// `raw_content` — frozen before any of this call's own writes — the
/// opposite of PCSX2's progressively-rewritten probe (see
/// `pcsx2::ensure_settings`).
///
/// `emulator_dir` (and therefore the write target, always
/// `<emulator_dir>/settings.ini`) is the trimmed, expanded path itself when
/// it is a directory, else its parent — computed independently of
/// [`config_path_candidates`]'s own (untrimmed, file-or-suffix) root rule,
/// exactly as the Python reference keeps them as two separate helpers
/// (duckstation.py:206 vs. duckstation.py:16-19).
///
/// Renderer seed (spec `2026-09-17-vulkan-renderer-seed-design.md`): on a
/// `fresh_install`, off macOS, `[GPU] Renderer = Vulkan` joins the per-key
/// preserve loop in step 5 — written only when `raw_content` has no
/// `Renderer` under `[GPU]`.
pub fn ensure_memory_card_settings(
    emulator_path: &str,
    enable_fullscreen: bool,
    fresh_install: bool,
) -> EnsureResult {
```

and replace step 5 (`duckstation.rs:325-343`) with:

```rust
    // 5: [GPU] the 9 keys, per-key preserve, probed against raw_content
    // (duckstation.py:284-301), plus the renderer seed on a fresh install.
    let mut gpu_desired: writers::Desired = Vec::new();
    for (key, value) in [
        ("ResolutionScale", "4"),
        ("PGXPEnable", "true"),
        ("PGXPColorCorrection", "true"),
        ("TextureFilter", "Scale2x"),
        ("SpriteTextureFilter", "Scale2x"),
        ("DitheringMode", "TrueColorFull"),
        ("LineDetectMode", "BasicTriangles"),
        ("DownsampleMode", "Box"),
        ("DownsampleScale", "2"),
    ] {
        if !writers::section_has_key(&raw_content, "GPU", key) {
            gpu_desired.push((key.to_string(), value.to_string()));
        }
    }
    if fresh_install
        && cfg!(not(target_os = "macos"))
        && !writers::section_has_key(&raw_content, "GPU", "Renderer")
    {
        gpu_desired.push(("Renderer".to_string(), "Vulkan".to_string()));
    }
    apply_section(&mut content, &mut changed, "GPU", &gpu_desired);
```

- [ ] 4. Update the existing test call sites in `duckstation.rs`: every `ensure_memory_card_settings(&emulator_path, false)` (lines 483, 496, 516, 538, 578, 596, 616, 629, 641, 657, 674, 694, 716, 791) becomes `ensure_memory_card_settings(&emulator_path, false, false)`; every `ensure_memory_card_settings(&emulator_path, true)` (lines 559, 848, 851) becomes `ensure_memory_card_settings(&emulator_path, true, false)`.

- [ ] 5. Update the dispatch. In `mod.rs:625-632` replace

```rust
            duckstation::ensure_memory_card_settings(path, true),
```
with
```rust
            duckstation::ensure_memory_card_settings(path, true, ctx.fresh_install),
```

- [ ] 6. Run `cargo test -p grid-core --lib duckstation` and `cargo test -p grid-core --lib autoconfig`. Expected: all pass.

- [ ] 7. `cargo fmt`, then commit:

```
git commit --only -- crates/grid-core/src/autoconfig/duckstation.rs crates/grid-core/src/autoconfig/mod.rs -m "$(cat <<'EOF'
rewrite: DuckStation fresh installs seed [GPU] Renderer = Vulkan in the per-key preserve loop

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: PPSSPP — `[Graphics] GraphicsBackend = 3 (VULKAN)`

**Files**
- Modify: `crates/grid-core/src/autoconfig/ppsspp.rs` (:171-232 `ensure_settings`, test module :280-572)
- Modify: `crates/grid-core/src/autoconfig/mod.rs:675-682` dispatch
- Test: `crates/grid-core/src/autoconfig/ppsspp.rs` test module

**Interfaces**
- Produces: `pub fn ensure_settings(emulator_path: &str, ra: Option<&RaCredentials>, fresh_install: bool) -> EnsureResult`
- Consumes: `read_guarded(path: &Path) -> Option<String>`, `base_sections() -> Vec<(&'static str, writers::Desired)>`, `writers::section_has_key`. `ensure_ra_credentials` is unchanged and still never touches `[Graphics]` (module doc lines 15-19).

**Steps**

- [ ] 1. Write the failing tests. Append inside `mod tests` in `ppsspp.rs`, after `ppsspp_no_change_for_empty_path` (after line 571):

```rust
    // --- renderer seed (spec 2026-09-17-vulkan-renderer-seed-design) --------

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn ppsspp_fresh_install_seeds_the_vulkan_backend() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());

        let result = ensure_settings(exe.to_str().unwrap(), None, true);

        assert!(result.changed);
        let text = std::fs::read_to_string(ini_path_for(&dir)).unwrap();
        assert!(text.contains("[Graphics]"), "{text}");
        assert!(text.contains("GraphicsBackend = 3 (VULKAN)"), "{text}");
        assert_eq!(text.matches("[Graphics]").count(), 1, "one section: {text}");
    }

    #[test]
    fn ppsspp_fresh_install_keeps_an_existing_backend() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        let ini = ini_path_for(&dir);
        std::fs::create_dir_all(ini.parent().unwrap()).unwrap();
        std::fs::write(&ini, "[Graphics]\nGraphicsBackend = 0 (OPENGL)\n").unwrap();

        ensure_settings(exe.to_str().unwrap(), None, true);

        let text = std::fs::read_to_string(&ini).unwrap();
        assert!(text.contains("GraphicsBackend = 0 (OPENGL)"), "{text}");
        assert!(!text.contains("VULKAN"), "{text}");
        assert!(
            text.contains("InternalResolution = 4"),
            "the other Graphics keys still land: {text}"
        );
    }

    #[test]
    fn ppsspp_non_fresh_call_never_adds_the_backend() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());

        ensure_settings(exe.to_str().unwrap(), None, false);

        let text = std::fs::read_to_string(ini_path_for(&dir)).unwrap();
        assert!(!text.contains("GraphicsBackend"), "{text}");
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn ppsspp_second_fresh_call_is_a_no_op() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, _dir) = make_exe(temp.path());

        let first = ensure_settings(exe.to_str().unwrap(), None, true);
        let second = ensure_settings(exe.to_str().unwrap(), None, true);

        assert!(first.changed);
        assert!(!second.changed);
    }

    #[test]
    fn ppsspp_ensure_ra_credentials_still_never_touches_graphics() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        let ra = RaCredentials::new("psp_user", "psp_tok");

        ensure_ra_credentials(exe.to_str().unwrap(), &ra);

        let text = std::fs::read_to_string(ini_path_for(&dir)).unwrap();
        assert!(!text.contains("[Graphics]"), "{text}");
        assert!(!text.contains("GraphicsBackend"), "{text}");
    }
```

- [ ] 2. Run `cargo test -p grid-core --lib ppsspp`. Expected: compile error `E0061` (2 arguments expected, 3 supplied).

- [ ] 3. Implement. Replace the doc comment, signature and the section assembly in `ppsspp.rs:171-206` with:

```rust
/// `ensure_ppsspp_settings` (ppsspp.py:75-166). Blank path (after `.trim()`)
/// returns [`EnsureResult::unchanged`].
///
/// Order of operations: delete `installed.txt` (ppsspp.py:87), then
/// overwrite the four base sections plus, only when BOTH `ra` fields are
/// non-blank after trimming, an `[Achievements]` section, then — again only
/// with both RA fields non-blank — the `ppsspp_retroachievements.dat` token
/// file. `config_path` is the INI path ONLY when [`write_ini_sections`]
/// fully succeeded (D5's unreadable-INI guard and a genuine write failure
/// both leave it `None`, matching [`EnsureResult`]'s documented contract);
/// `extras["ra_token_path"]` is set ONLY when [`write_ra_token_dat`] fully
/// succeeded for a present RA pair. `changed` still reflects every write
/// that actually happened (e.g. `installed.txt`'s deletion), independent of
/// whether the INI or dat portions failed.
///
/// Renderer seed (spec `2026-09-17-vulkan-renderer-seed-design.md`): on a
/// `fresh_install`, off macOS, `GraphicsBackend = 3 (VULKAN)` joins the
/// `[Graphics]` desired list ONLY when the INI as it is before this call
/// has no `GraphicsBackend` under `[Graphics]`. [`ensure_ra_credentials`]
/// never takes the flag and never touches `[Graphics]`.
pub fn ensure_settings(
    emulator_path: &str,
    ra: Option<&RaCredentials>,
    fresh_install: bool,
) -> EnsureResult {
    let Some(emulator_dir) = resolve_emulator_dir(emulator_path) else {
        return EnsureResult::unchanged();
    };

    let mut changed = delete_installed_txt(&emulator_dir);
    let ini_path = ini_path(&emulator_dir);

    let (ra_user, ra_token) = ra
        .map(|creds| {
            (
                creds.username().trim().to_string(),
                creds.token().trim().to_string(),
            )
        })
        .unwrap_or_default();
    let has_ra = !ra_user.is_empty() && !ra_token.is_empty();

    let mut sections = base_sections();
    // The probe reads the INI BEFORE this call writes, like every preserve
    // probe in this crate. An unreadable INI probes as empty; the write below
    // then fails exactly as it always did (D5), so nothing new leaks out.
    if fresh_install && cfg!(not(target_os = "macos")) {
        let existing = read_guarded(&ini_path).unwrap_or_default();
        if !writers::section_has_key(&existing, "Graphics", "GraphicsBackend") {
            for (name, desired) in sections.iter_mut() {
                if *name == "Graphics" {
                    desired.push(("GraphicsBackend".to_string(), "3 (VULKAN)".to_string()));
                }
            }
        }
    }
    if has_ra {
        sections.push(("Achievements", achievements_section(&ra_user, &ra_token)));
    }
```

(The remainder of the function — `let mut config_path = None; if let Some(ini_changed) = write_ini_sections(...)` through the final `EnsureResult { ... }` — is unchanged.)

- [ ] 4. Update the existing test call sites in `ppsspp.rs`: every `ensure_settings(<path>, None)` / `ensure_settings(<path>, Some(&...))` at lines 312, 323, 334, 363, 370, 381, 410, 415, 434, 462, 487, 498, 518, 519, 569 gains a trailing `, false` (for example `ensure_settings(exe.to_str().unwrap(), None, false)`, `ensure_settings(exe.to_str().unwrap(), Some(&ra), false)`, `ensure_settings("", None, false)`; the three `catch_unwind` closures wrap the same call).

- [ ] 5. Update the dispatch. In `mod.rs:675-682` replace

```rust
            ppsspp::ensure_settings(path, ra),
```
with
```rust
            ppsspp::ensure_settings(path, ra, ctx.fresh_install),
```

- [ ] 6. Run `cargo test -p grid-core --lib ppsspp` and `cargo test -p grid-core --lib autoconfig`. Expected: all pass.

- [ ] 7. `cargo fmt`, then commit:

```
git commit --only -- crates/grid-core/src/autoconfig/ppsspp.rs crates/grid-core/src/autoconfig/mod.rs -m "$(cat <<'EOF'
rewrite: PPSSPP fresh installs seed [Graphics] GraphicsBackend = 3 (VULKAN) with per-key preserve

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: Azahar — `[Renderer] graphics_api\default=false`, `graphics_api=2`

**Files**
- Modify: `crates/grid-core/src/autoconfig/azahar.rs` (:104-187 `ensure_settings`, test module :189-334)
- Modify: `crates/grid-core/src/autoconfig/mod.rs:662` dispatch
- Test: `crates/grid-core/src/autoconfig/azahar.rs` test module

**Interfaces**
- Produces: `pub fn ensure_settings(emulator_path: &str, fresh_install: bool) -> EnsureResult`
- Consumes: `writers::azahar_section(raw, section, desired) -> (String, bool)`, `writers::section_has_key`.

**Steps**

- [ ] 1. Write the failing tests. Append inside `mod tests` in `azahar.rs`, after `azahar_blank_path_is_unchanged` (after line 297):

```rust
    // --- renderer seed (spec 2026-09-17-vulkan-renderer-seed-design) --------

    fn qt_config(dir: &Path) -> PathBuf {
        dir.join("user").join("config").join("qt-config.ini")
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn azahar_fresh_install_seeds_the_vulkan_api_with_its_companion() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());

        let result = ensure_settings(exe.to_str().unwrap(), true);

        assert!(result.changed);
        let text = std::fs::read_to_string(qt_config(&dir)).unwrap();
        assert!(text.contains(r"graphics_api\default = false"), "{text}");
        assert!(text.contains("graphics_api = 2"), "{text}");
        assert_eq!(text.matches(r"graphics_api\default").count(), 1, "{text}");
    }

    #[test]
    fn azahar_fresh_install_keeps_an_existing_api() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());
        let ini = qt_config(&dir);
        std::fs::create_dir_all(ini.parent().unwrap()).unwrap();
        std::fs::write(
            &ini,
            "[Renderer]\ngraphics_api\\default=false\ngraphics_api=1\n",
        )
        .unwrap();

        ensure_settings(exe.to_str().unwrap(), true);

        let text = std::fs::read_to_string(&ini).unwrap();
        assert!(text.contains("graphics_api=1"), "verbatim, unmanaged: {text}");
        assert!(!text.contains("graphics_api = 2"), "{text}");
        assert!(
            text.contains("resolution_factor = 4"),
            "the other Renderer keys still land: {text}"
        );
    }

    #[test]
    fn azahar_non_fresh_call_never_adds_the_api() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path());

        ensure_settings(exe.to_str().unwrap(), false);

        let text = std::fs::read_to_string(qt_config(&dir)).unwrap();
        assert!(!text.contains("graphics_api"), "{text}");
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn azahar_second_fresh_call_is_a_no_op() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, _dir) = make_exe(temp.path());

        let first = ensure_settings(exe.to_str().unwrap(), true);
        let second = ensure_settings(exe.to_str().unwrap(), true);

        assert!(first.changed);
        assert!(!second.changed);
    }
```

- [ ] 2. Run `cargo test -p grid-core --lib azahar`. Expected: compile error `E0061` (1 argument expected, 2 supplied).

- [ ] 3. Implement. Replace `ensure_settings` in `azahar.rs:104-187` with:

```rust
/// `ensure_azahar_settings` (azahar.py:144-216).
///
/// Creates `<emulator_dir>/user/` first (see [`maybe_create_user_dir`]).
/// [`EnsureResult::unchanged`] when [`config_path_candidates`] is empty
/// (blank path). Otherwise selects the first EXISTING candidate, falling
/// back to the first, and overwrites every key below unconditionally via
/// [`writers::azahar_section`] — every real key preceded by its
/// `<key>\default` companion, written as an ORDINARY key (which is exactly
/// why the widened key charset exists):
///
/// - `[Renderer]`: `resolution_factor\default`=`false`,
///   `resolution_factor`=`4`, `use_vsync\default`=`false`, `use_vsync`=`true`.
/// - `[Audio]`: `volume\default`=`false`, `volume`=`0.4`.
/// - `[UI]`: `enable_discord_presence\default`=`false`,
///   `enable_discord_presence`=`false`, `confirmClose\default`=`false`,
///   `confirmClose`=`false`, `fullscreen\default`=`false`,
///   `fullscreen`=`true`, `pauseWhenInBackground\default`=`false`,
///   `pauseWhenInBackground`=`true`, `hideInactiveMouse\default`=`false`,
///   `hideInactiveMouse`=`true`, the Fullscreen shortcut
///   (`Shortcuts\Main%20Window\Fullscreen\KeySeq`, default `false`/value
///   `F1`), the Stop Emulation shortcut (same pattern, value `Escape`).
///
/// Renderer seed (spec `2026-09-17-vulkan-renderer-seed-design.md`): on a
/// `fresh_install`, off macOS, `[Renderer]` also gets
/// `graphics_api\default`=`false` and `graphics_api`=`2` (Vulkan) — ONLY
/// when the file as read has no `graphics_api` under `[Renderer]`. The
/// probe is [`writers::section_has_key`], whose narrow key charset stops at
/// the `\` of the companion line, so only a real `graphics_api=` line
/// counts as present.
///
/// Any I/O error along the way reports [`EnsureResult::unchanged`].
pub fn ensure_settings(emulator_path: &str, fresh_install: bool) -> EnsureResult {
    maybe_create_user_dir(emulator_path);

    let candidates = config_path_candidates(emulator_path);
    if candidates.is_empty() {
        return EnsureResult::unchanged();
    }
    let selected = candidates
        .iter()
        .find(|c| c.exists())
        .cloned()
        .unwrap_or_else(|| candidates[0].clone());
    let seed_renderer = fresh_install && cfg!(not(target_os = "macos"));

    match write_if_changed(&selected, |original| {
        let mut renderer_desired = crate::desired![
            (r"resolution_factor\default", "false"),
            ("resolution_factor", "4"),
            (r"use_vsync\default", "false"),
            ("use_vsync", "true"),
        ];
        if seed_renderer && !writers::section_has_key(original, "Renderer", "graphics_api") {
            renderer_desired.push((r"graphics_api\default".to_string(), "false".to_string()));
            renderer_desired.push(("graphics_api".to_string(), "2".to_string()));
        }
        let (content, c1) = writers::azahar_section(original, "Renderer", &renderer_desired);
        let (content, c2) = writers::azahar_section(
            &content,
            "Audio",
            &crate::desired![(r"volume\default", "false"), ("volume", "0.4")],
        );
        let (content, c3) = writers::azahar_section(
            &content,
            "UI",
            &crate::desired![
                (r"enable_discord_presence\default", "false"),
                ("enable_discord_presence", "false"),
                (r"confirmClose\default", "false"),
                ("confirmClose", "false"),
                (r"fullscreen\default", "false"),
                ("fullscreen", "true"),
                (r"pauseWhenInBackground\default", "false"),
                ("pauseWhenInBackground", "true"),
                (r"hideInactiveMouse\default", "false"),
                ("hideInactiveMouse", "true"),
                (
                    r"Shortcuts\Main%20Window\Fullscreen\KeySeq\default",
                    "false"
                ),
                (r"Shortcuts\Main%20Window\Fullscreen\KeySeq", "F1"),
                (
                    r"Shortcuts\Main%20Window\Stop%20Emulation\KeySeq\default",
                    "false"
                ),
                (r"Shortcuts\Main%20Window\Stop%20Emulation\KeySeq", "Escape"),
            ],
        );
        (content, c1 || c2 || c3)
    }) {
        Ok(changed) => EnsureResult::at(selected, changed),
        Err(_) => EnsureResult::unchanged(),
    }
}
```

- [ ] 4. Update the existing test call sites in `azahar.rs`: `ensure_settings(exe.to_str().unwrap())` at lines 216, 231, 243, 244, 264, 285, 286 becomes `ensure_settings(exe.to_str().unwrap(), false)`; lines 295-296 become `assert_eq!(ensure_settings("", false), EnsureResult::unchanged());` and `assert_eq!(ensure_settings("   ", false), EnsureResult::unchanged());`.

- [ ] 5. Update the dispatch. In `mod.rs:662` replace

```rust
        record(&mut report, name, "azahar", azahar::ensure_settings(path));
```
with
```rust
        record(
            &mut report,
            name,
            "azahar",
            azahar::ensure_settings(path, ctx.fresh_install),
        );
```

- [ ] 6. Run `cargo test -p grid-core --lib azahar` and `cargo test -p grid-core --lib autoconfig`. Expected: all pass.

- [ ] 7. `cargo fmt`, then commit:

```
git commit --only -- crates/grid-core/src/autoconfig/azahar.rs crates/grid-core/src/autoconfig/mod.rs -m "$(cat <<'EOF'
rewrite: Azahar fresh installs seed [Renderer] graphics_api=2 with its \default companion, per-key preserve

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: xemu — `[display] renderer = 'VULKAN'`

**Files**
- Modify: `crates/grid-core/src/autoconfig/xemu.rs` (:115-156 `sections`, :158-208 `ensure_settings`, test module :297-576)
- Modify: `crates/grid-core/src/autoconfig/mod.rs:633-635` dispatch
- Test: `crates/grid-core/src/autoconfig/xemu.rs` test module

**Interfaces**
- Produces: `pub fn ensure_settings(emulator_path: &str, fresh_install: bool) -> EnsureResult`
- Produces (private): `fn sections(base_dir: &std::path::Path, seed_renderer: bool) -> Vec<(&'static str, writers::Desired)>`
- Consumes: `writers::toml_add_only_section` (add-only by construction, so no probe is needed).

**Steps**

- [ ] 1. Write the failing tests. Append inside `mod tests` in `xemu.rs`, after `xemu_blank_path_targets_default_base_root` (after line 441):

```rust
    // --- renderer seed (spec 2026-09-17-vulkan-renderer-seed-design) --------

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn xemu_fresh_install_seeds_the_vulkan_renderer() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());

        let result = ensure_settings(exe.to_str().unwrap(), true);

        assert!(result.changed);
        let text = std::fs::read_to_string(dir.join("xemu.toml")).unwrap();
        assert!(text.contains("[display]"), "{text}");
        assert!(text.contains("renderer = 'VULKAN'"), "{text}");
        assert_eq!(text.matches("[display]").count(), 1, "one section: {text}");
    }

    #[test]
    fn xemu_fresh_install_keeps_an_existing_renderer() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        std::fs::write(dir.join("xemu.toml"), "[display]\nrenderer = 'OPENGL'\n").unwrap();

        ensure_settings(exe.to_str().unwrap(), true);

        let text = std::fs::read_to_string(dir.join("xemu.toml")).unwrap();
        assert!(text.contains("renderer = 'OPENGL'"), "{text}");
        assert!(!text.contains("VULKAN"), "{text}");
        assert!(text.contains("vsync = true"), "the other display key still lands: {text}");
    }

    #[test]
    fn xemu_non_fresh_call_never_adds_the_renderer() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());

        ensure_settings(exe.to_str().unwrap(), false);

        let text = std::fs::read_to_string(dir.join("xemu.toml")).unwrap();
        assert!(!text.contains("renderer"), "{text}");
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn xemu_second_fresh_call_is_a_no_op() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, _dir) = make_exe(temp.path());

        let first = ensure_settings(exe.to_str().unwrap(), true);
        let second = ensure_settings(exe.to_str().unwrap(), true);

        assert!(first.changed);
        assert!(!second.changed);
    }
```

- [ ] 2. Run `cargo test -p grid-core --lib xemu`. Expected: compile error `E0061` (1 argument expected, 2 supplied).

- [ ] 3. Implement. Replace `sections` (`xemu.rs:115-156`) with:

```rust
/// The eight `(section, desired)` pairs, in the pinned order
/// (xemu.py:257-315). `base_dir` backs the four `[sys.files]` paths, each
/// wrapped in single quotes with no escaping. `hdd_path`'s default is
/// [`hdd_default_path`] (D3).
///
/// `seed_renderer` (spec `2026-09-17-vulkan-renderer-seed-design.md`) adds
/// `renderer = 'VULKAN'` to `[display]`. Add-only like everything else here,
/// so an existing `renderer` survives whatever it says.
fn sections(
    base_dir: &std::path::Path,
    seed_renderer: bool,
) -> Vec<(&'static str, writers::Desired)> {
    let mut display = crate::desired![("vsync", "true")];
    if seed_renderer {
        display.push(("renderer".to_string(), "'VULKAN'".to_string()));
    }
    vec![
        ("general", crate::desired![("show_welcome", "false")]),
        ("misc", crate::desired![("check_for_updates", "false")]),
        ("display", display),
        (
            "display.window",
            crate::desired![("fullscreen_on_startup", "true")],
        ),
        ("display.quality", crate::desired![("surface_scale", "2")]),
        ("audio", crate::desired![("volume_limit", "0.4")]),
        (
            "input.bindings",
            crate::desired![("port1_driver", "\"usb-xbox-gamepad\"")],
        ),
        (
            "sys.files",
            crate::desired![
                (
                    "bootrom_path",
                    format!("'{}'", base_dir.join("mcpx_1.0.bin").display())
                ),
                (
                    "flashrom_path",
                    format!("'{}'", base_dir.join("complex_4627.bin").display())
                ),
                (
                    "hdd_path",
                    format!("'{}'", hdd_default_path(base_dir).display())
                ),
                (
                    "eeprom_path",
                    format!("'{}'", base_dir.join("eeprom.bin").display())
                ),
            ],
        ),
    ]
}
```

and replace the doc comment, signature and loop in `ensure_settings` (`xemu.rs:158-194`) with:

```rust
/// `ensure_xemu_settings` (xemu.py:243-297). Target:
/// `<emulator_dir>/xemu.toml` when a non-blank path is given (trimmed,
/// expanded, dir-or-parent; **no existence check on the file itself**),
/// else `<default_base_root()>/xemu.toml`.
///
/// Every one of the eight sections is written add-only via
/// [`writers::toml_add_only_section`], unconditionally, chaining the
/// content through each call; `changed` is the OR of all eight. The file is
/// written back only when something changed, with the parent directory
/// created lazily. Any I/O error — reading the existing file, creating the
/// parent, or writing — yields [`EnsureResult::unchanged`]
/// (xemu.py:296-297's bare `except OSError`).
///
/// `fresh_install`, off macOS, turns on the `[display] renderer` seed in
/// [`sections`]; add-only semantics make it per-key preserve for free.
pub fn ensure_settings(emulator_path: &str, fresh_install: bool) -> EnsureResult {
    let emulator_dir = resolve_emulator_dir(emulator_path);
    let config_path = match &emulator_dir {
        Some(dir) => dir.join("xemu.toml"),
        None => default_base_root().join("xemu.toml"),
    };

    let content = if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(_) => return EnsureResult::unchanged(),
        }
    } else {
        String::new()
    };

    let base_dir = emulator_dir.clone().unwrap_or_else(default_base_root);
    let seed_renderer = fresh_install && cfg!(not(target_os = "macos"));
    let mut content = content;
    let mut changed = false;
    for (section, desired) in sections(&base_dir, seed_renderer) {
        let (new_content, section_changed) =
            writers::toml_add_only_section(&content, section, &desired);
        content = new_content;
        changed |= section_changed;
    }
```

(The trailing `if changed { ... }` and `EnsureResult::at(config_path, changed)` are unchanged.)

- [ ] 4. Update the existing test call sites in `xemu.rs`: `ensure_settings(exe.to_str().unwrap())` at lines 317, 348, 359, 391, 406, 409, 497, 511, 531 becomes `ensure_settings(exe.to_str().unwrap(), false)`; line 427 becomes `let result = ensure_settings("", false);`.

- [ ] 5. Update the dispatch. In `mod.rs:633-635` replace

```rust
        record(&mut report, name, "xemu", xemu::ensure_settings(path));
```
with
```rust
        record(
            &mut report,
            name,
            "xemu",
            xemu::ensure_settings(path, ctx.fresh_install),
        );
```

- [ ] 6. Run `cargo test -p grid-core --lib xemu` and `cargo test -p grid-core --lib autoconfig`. Expected: all pass, including `xemu_add_only_per_key` and `existing_hdd_path_key_is_never_touched`.

- [ ] 7. `cargo fmt`, then commit:

```
git commit --only -- crates/grid-core/src/autoconfig/xemu.rs crates/grid-core/src/autoconfig/mod.rs -m "$(cat <<'EOF'
rewrite: xemu fresh installs seed [display] renderer = 'VULKAN' through the add-only writer

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: `sync_new_emulator` forwards `fresh_install`

**Files**
- Test: `crates/grid-core/src/autoconfig/mod.rs` test module (after `sync_writes_no_pcsx2_bios_directory_without_a_profile`, line 1617)

**Interfaces**
- Consumes: `sync_new_emulator(entry_name: &str, ctx: &SyncContext) -> Result<SyncReport, ConfigError>`; the test fixtures `lock`, `isolated`, `touch`, `entry`, `config_with`, `write_config`, `no_slugs` already in the module; Dolphin's `[Core] GFXBackend` seed from Task 3 as the observable.

**Steps**

- [ ] 1. Write the tests. Append after line 1617 in `mod tests`:

```rust
    // --- renderer seed forwarding (spec 2026-09-17-vulkan-renderer-seed) ----

    /// The renderer seed is gated on `SyncContext::fresh_install`, so the
    /// orchestrator must hand the flag to the writers. Dolphin is the probe:
    /// `[Core] GFXBackend` appears in `User/Config/Dolphin.ini` only on a
    /// fresh pass. `profiles: &[]` — `is_dolphin` matches the entry NAME.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn sync_forwards_fresh_install_to_the_writers() {
        let _lock = lock();
        let temp = tempfile::tempdir().unwrap();
        let _env = isolated(temp.path());

        let exe = temp.path().join("Dolphin").join("dolphin-emu");
        touch(&exe);
        let config = config_with(temp.path(), vec![entry("Dolphin", exe.to_str().unwrap())]);
        let config_path = write_config(temp.path(), &config);

        let ctx = SyncContext {
            config_path: &config_path,
            platforms: &[],
            platform_slugs: &no_slugs(),
            ps3_library_path: String::new(),
            ra: None,
            profiles: &[],
            fresh_install: true,
        };
        let report = sync_new_emulator("Dolphin", &ctx).unwrap();
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);

        let ini = std::fs::read_to_string(
            exe.parent()
                .unwrap()
                .join("User")
                .join("Config")
                .join("Dolphin.ini"),
        )
        .unwrap();
        assert!(ini.contains("GFXBackend = Vulkan"), "{ini}");
    }

    /// The contrast: the same entry through a non-fresh pass (a reinstall
    /// or a catalog update) gets every other Dolphin key and no renderer.
    #[test]
    fn sync_without_fresh_install_leaves_the_renderer_alone() {
        let _lock = lock();
        let temp = tempfile::tempdir().unwrap();
        let _env = isolated(temp.path());

        let exe = temp.path().join("Dolphin").join("dolphin-emu");
        touch(&exe);
        let config = config_with(temp.path(), vec![entry("Dolphin", exe.to_str().unwrap())]);
        let config_path = write_config(temp.path(), &config);

        let ctx = SyncContext {
            config_path: &config_path,
            platforms: &[],
            platform_slugs: &no_slugs(),
            ps3_library_path: String::new(),
            ra: None,
            profiles: &[],
            fresh_install: false,
        };
        let report = sync_new_emulator("Dolphin", &ctx).unwrap();
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);

        let ini = std::fs::read_to_string(
            exe.parent()
                .unwrap()
                .join("User")
                .join("Config")
                .join("Dolphin.ini"),
        )
        .unwrap();
        assert!(ini.contains("Fullscreen = True"), "the writer still ran: {ini}");
        assert!(!ini.contains("GFXBackend"), "{ini}");
    }
```

- [ ] 2. Run `cargo test -p grid-core --lib sync_forwards_fresh_install_to_the_writers` and `cargo test -p grid-core --lib sync_without_fresh_install_leaves_the_renderer_alone`. Expected: both PASS on the first run — Tasks 2-7 already forward `ctx.fresh_install` at every dispatch site. If either fails, the dispatch line for Dolphin (mod.rs, Task 3 step 5) was not updated; fix that, not the test.

- [ ] 3. Run the full module: `cargo test -p grid-core --lib autoconfig`. Expected: all pass.

- [ ] 4. `cargo fmt`, then commit:

```
git commit --only -- crates/grid-core/src/autoconfig/mod.rs -m "$(cat <<'EOF'
test: sync_new_emulator forwards fresh_install to the renderer seed

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
EOF
)"
```

---

## Task 9: e2e — the `emulators` stage asserts DuckStation's Vulkan key

**Files**
- Modify: `e2e/specs/emulators.spec.ts` (:361-387, the DuckStation note case)

**Interfaces**
- Consumes: `dataDir()`, `TRANSITION_TIMEOUT` from `../helpers/env.js`; `mkdirSync`, `chmodSync`, `writeFileSync`, `existsSync`, `readFileSync` (all already imported at :1-10); `save_emulator` -> `run_emulator_sync(..., true, autoconfig::sync_new_emulator)` -> `duckstation::ensure_memory_card_settings(path, true, true)` writes `<stub dir>/settings.ini`.
- Why here and not `emulator-catalog`: no e2e stage installs one of the six seeded emulators from the catalog (`emulator-catalog` installs PCSX2 and Redream, neither seeded). The `emulators` stage already adds a DuckStation row by hand at `/nonexistent/duckstation`, which no writer can reach. Pointing that row at a real stub in its own directory makes the add-time sync write `settings.ini` beside it. `delete_emulator` leaves hand-configured paths on disk (commands.rs:1100-1102), so the case's existing cleanup does not remove the stub or the other stubs. No new stage.

**Steps**

- [ ] 1. Replace the case at `e2e/specs/emulators.spec.ts:361-387` with:

```ts
  it('shows the DuckStation controller note and none for RetroArch, and seeds its Vulkan renderer', async () => {
    // A real stub in its OWN directory: a manual add is a fresh install, so
    // the add-time autoconfig sync writes `settings.ini` beside the
    // executable and the renderer seed
    // (docs/superpowers/specs/2026-09-17-vulkan-renderer-seed-design.md)
    // must land in it. Its own directory keeps that file away from the
    // RetroArch stub. `delete_emulator` leaves hand-configured paths on
    // disk, so the cleanup below removes only the row.
    const duckDir = path.join(dataDir(), 'stubs', 'duckstation');
    mkdirSync(duckDir, { recursive: true });
    const duckPath = path.join(duckDir, 'duckstation');
    writeFileSync(duckPath, '#!/bin/sh\nexit 0\n');
    chmodSync(duckPath, 0o755);

    await $(testId('emulator-add')).click();
    await $(testId('emu-add-tab-manual')).click();
    await $(testId('emu-form-name')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('emu-form-name')).setValue('DuckStation');
    await $(testId('emu-form-path')).setValue(duckPath);
    await $(testId('emu-form-save')).click();
    await $(testId(`emulator-row-${sanitize('DuckStation')}`)).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the DuckStation row never appeared',
    });

    await expect($(testId('emulator-note-duckstation-duckstation'))).toHaveText(
      'RetroAchievements: Configure login via Emulator Settings → Achievements (tokens are machine-encrypted)',
    );
    await expect($(testId('emulator-note-azahar-duckstation'))).not.toExist();

    // The sync runs after the config save, on the blocking pool; poll for
    // the file rather than reading it right away.
    const settingsIni = path.join(duckDir, 'settings.ini');
    await browser.waitUntil(() => existsSync(settingsIni), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `the add-time autoconfig never wrote ${settingsIni}`,
    });
    const ini = readFileSync(settingsIni, 'utf-8');
    expect(ini).toContain('[GPU]');
    expect(ini).toContain('Renderer = Vulkan');

    // Clean up so the defaults cases below still see the single RetroArch row.
    const deleteBtn = $(testId(`emulator-delete-${sanitize('DuckStation')}`));
    await deleteBtn.click();
    await deleteBtn.click();
    await $(testId(`emulator-row-${sanitize('DuckStation')}`)).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the DuckStation row was not removed',
    });
  });
```

- [ ] 2. Run the stage with a rebuild (Rust changed since the last e2e build, so `E2E_SKIP_BUILD=1` is NOT safe): `scripts/e2e.sh emulators`. Expected: the `emulators` stage passes, including the renamed case. If `settings.ini` never appears, check the app log for an `emulator autoconfig:` warning line (it names the emulator and writer only).

- [ ] 3. Commit:

```
git commit --only -- e2e/specs/emulators.spec.ts -m "$(cat <<'EOF'
test(e2e): a manually added DuckStation gets [GPU] Renderer = Vulkan

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
EOF
)"
```

---

## Task 10: Docs — SKILL.md "Renderer seed" subsection

**Files**
- Modify: `.claude/skills/emulator-autoconfig/SKILL.md` (:20 module-map line for `mod.rs`; insert a new section before `## RetroAchievements Credential Wiring` at :176; :225-235 checklist)
- ARCHITECTURE.md: NO change. It describes `autoconfig` at line 28 only as "the `ensure_*` writers that seed an emulator's own settings files" and never names `SyncContext`; the spec's sentence is conditional on that and the condition is not met.

**Interfaces**
- None (documentation).

**Steps**

- [ ] 1. In `SKILL.md:20`, replace the `mod.rs` bullet's text with:

```
  - `RaCredentials` (RetroAchievements creds, `.usable()` gate), `SyncContext` (including `fresh_install: bool`, the renderer seed's gate), `SyncReport`, and the two orchestration entry points: `sync_new_emulator` (runs a full per-emulator sync plus a RetroArch backfill) and `sync_retroarch_settings_only` (RetroArch-only pass, never applies a profile).
```

- [ ] 2. Insert this section immediately before `## RetroAchievements Credential Wiring` (SKILL.md:176):

```markdown
## Renderer Seed (Vulkan on a fresh install)

Spec: `docs/superpowers/specs/2026-09-17-vulkan-renderer-seed-design.md`.

- The gate is `SyncContext::fresh_install`. It is `true` only for an entry that did not exist before the pass: a catalog install that CREATED the entry (`InstallService::finalize_emulator` passes `write_emulator_entry`'s answer) or a manual add (`save_emulator` passes `true`). A reinstall, a catalog update, the defaults backfill and the launch-time RetroArch pass are `false`.
- The six full writers take the flag as a trailing `bool`: `cemu::ensure_settings`, `dolphin::ensure_settings`, `duckstation::ensure_memory_card_settings`, `ppsspp::ensure_settings`, `azahar::ensure_settings`, `xemu::ensure_settings`. The narrow writers (`ensure_ra_credentials`, `ensure_skip_ipl`, `ensure_gcpad_config`, `ensure_controller_config`) do not.
- Per-key preserve: the key is written only when the file AS READ (before the call's own edits) has no such key in that section. Overwrite-policy writers probe with `writers::section_has_key` and omit the key when present; xemu's add-only writer preserves by construction. A present value — even a non-Vulkan one — always survives. A second fresh call reports `changed == false`.
- macOS: every seed is gated by `cfg!(not(target_os = "macos"))`. Nothing is seeded there. (Cemu's template constant carries `<api>1</api>` on every host; only the merge-branch seed is gated.)
- `fresh_install == false` never adds the key. Emulators installed before this change are not re-seeded.

| Emulator | File | Seed |
|---|---|---|
| Cemu | `portable/settings.xml` | `<Graphic><api>1</api>`; the template says `1`; the merge branch adds the child inside an existing `<Graphic>` lacking `<api>`, or creates `<Graphic><api>1</api></Graphic>` before `</content>` when `<Graphic>` is absent |
| Dolphin | `User/Config/Dolphin.ini` | `[Core] GFXBackend = Vulkan` |
| DuckStation | `settings.ini` | `[GPU] Renderer = Vulkan` |
| PPSSPP | `memstick/PSP/SYSTEM/PPSSPP.INI` | `[Graphics] GraphicsBackend = 3 (VULKAN)` |
| Azahar | `qt-config.ini` | `[Renderer] graphics_api\default=false` and `graphics_api=2` |
| xemu | `xemu.toml` | `[display] renderer = 'VULKAN'` |

Not seeded: PCSX2 (already seeds `[EmuCore/GS] Renderer = 14` with per-key preserve), RPCS3 / Eden / Vita3K (default to Vulkan), ShadPS4 / KytyPS5 (Vulkan-only), Redream / Supermodel (no Vulkan renderer), Xenia (no writer), RetroArch (one global `video_driver` serves every core and some cores misbehave under `vulkan`).
```

- [ ] 3. Add one line to the Implementation Checklist (SKILL.md:225-235), after "Function is idempotent on repeated calls.":

```
- A renderer key is written only behind `fresh_install && cfg!(not(target_os = "macos"))`, with per-key preserve, and the writer's tests cover fresh+absent, fresh+present, not-fresh+absent, and a second fresh call.
```

- [ ] 4. Run the full gate from the repo root, in order:

```
scripts/check_secret_hygiene.sh
cargo fmt --check
cd app && npm ci && npx svelte-check && npm run build && cd ..
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p app --all-targets --features e2e -- -D warnings
cargo test --workspace
cd app && npm test && cd ..
```

Expected: all pass. If clippy flags `collapsible_if` in ppsspp.rs's seed block, the outer `if` has a `let existing = ...;` statement so it is not collapsible; if it flags the inner `if !section_has_key { for ... }`, that is a `for`, not an `if`, so it is not collapsible either. Report any other clippy finding rather than silencing it with an attribute.

- [ ] 5. Commit:

```
git commit --only -- .claude/skills/emulator-autoconfig/SKILL.md -m "$(cat <<'EOF'
docs: renderer seed subsection in the emulator-autoconfig skill

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
EOF
)"
```

---

## Edge cases to handle (already encoded in the tasks; listed so the reviewer can check)

- Cemu `<Audio><api>3</api>` must not satisfy the `<Graphic>` probe: the probe regex runs only over `<Graphic>`'s inner text (Task 2 test asserts `<api>3</api>` survives and exactly one `<api>1</api>` appears).
- Cemu `<GraphicPack/>` must not match `<Graphic>`: the regex is the literal open tag with its `>`.
- Cemu self-closing `<Graphic/>`: replaced by `<Graphic><api>1</api></Graphic>` rather than creating a second `<Graphic>` element.
- Cemu create-from-template branch is NOT gated by `fresh_install` — the file is missing, so the whole template goes in as today, and the template now says `1`. The "not fresh + absent" Cemu test therefore has to use the merge branch (an existing file).
- Cemu template on macOS: the constant is one string for every host and now says `<api>1</api>`. The spec gates the SEED on macOS, and the template is not a seed; Cemu on macOS renders only through Vulkan/MoltenVK anyway. Flagged as an open question below.
- Azahar companion line `graphics_api\default=false` must not count as `graphics_api` present: `NARROW_KEY_RE` stops at `\`, so only a real `graphics_api=` line satisfies the probe (Task 6 test seeds `graphics_api=1` and checks it survives verbatim).
- Dolphin probe reads the closure's original `&str` parameter, not the progressively rewritten `content` — same rule as DuckStation's `raw_content`.
- PPSSPP probes a second read of the INI (`read_guarded`) before `write_ini_sections` does its own read. An unreadable INI probes as empty; the seed key joins `[Graphics]`, and the write then fails exactly as before (D5) with `config_path = None`. No new behavior on the failure path.
- PPSSPP `ensure_ra_credentials` keeps its "never touches `[Graphics]`" contract (Task 5 adds a test pinning it).
- xemu add-only: an existing `renderer` of any value survives; the `[display]` section is still one section (test asserts one `[display]` header).
- Tests that assert the seed WAS written carry `#[cfg(not(target_os = "macos"))]`; tests that assert preserve / not-fresh / RA isolation run on every host.
- All existing writer tests pass `false`, so their assertions are unchanged; every `SyncContext` literal in existing tests gets `fresh_install: false`.
- e2e: `E2E_SKIP_BUILD=1` is unsafe for Task 9 because the Rust crates changed; run `scripts/e2e.sh emulators` with the build.

## Open questions

1. Cemu template on macOS. The spec says "macOS: nothing is seeded on any emulator" and separately "the template itself changes to `1`". These two statements meet on a macOS create-from-template branch, which will now write `<api>1</api>`. The plan follows the spec literally (template unconditional, merge-branch seed gated) because Cemu's macOS build has no non-Vulkan renderer. If the user wants the template's `<api>` to stay `0` on macOS, that needs a `cfg!`-selected template pair; say so before Task 2.
2. The spec's e2e sentence names the `emulators` stage as the one that "installs a catalog emulator against the mock forge"; it is `emulator-catalog` that does, and it installs PCSX2 and Redream (neither seeded). Task 9 uses the `emulators` stage's manual DuckStation add instead, which exercises the `save_emulator` -> `fresh_install: true` path. The catalog-install path (`finalize_emulator` -> `write_emulator_entry` -> `fresh`) is covered only by the Rust unit tests (Task 8) and not end to end. Adding a seeded emulator to the mock forge fixture would be a larger change than the spec's "no new stage" allows; flag if end-to-end coverage of the catalog path is wanted.
3. Commit prefix. Recent history uses `rewrite:`, `fix:`, `docs:`; the plan uses `rewrite:` for the feature commits, `test:` / `test(e2e):` for the test-only ones and `docs:` for the skill. Adjust if the repo prefers a different prefix for test-only commits.

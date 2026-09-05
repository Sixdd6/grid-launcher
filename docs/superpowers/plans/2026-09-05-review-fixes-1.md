# Review fixes 1 — details popup, cards, RetroArch AppImage launch

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the six defects the user found while reviewing the merged UI redesign: three in the details popup, two on the game cards, and the RetroArch AppImage launch failure (with its config-file twin).

**Architecture:** Each fix is small and local. The RetroArch fix introduces one shared helper for the AppImage portable-home layout (`<exe>.home/.config/retroarch`) so the core installer, the launch-time core resolver and the config-file writer all agree on where an AppImage keeps its files. The release-date fix normalises RomM's millisecond stamps in the backend so the frontend's "epoch seconds" contract stays true. The card fixes are CSS plus one optional backdrop image in `Image.svelte`.

**Tech Stack:** Rust (grid-core), Svelte 5 runes + TypeScript + vitest, WebdriverIO E2E against the mock RomM server.

**Spec:** `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` for the UI items (§5 cards, §7 details popup, D-UI-9). The user's review decisions (recorded below) amend that spec where they differ: the user chose "option B" for card covers. `docs/porting/04-emulator-launch.md` and `docs/porting/05-emulator-autoconfig.md` are the behaviour specs for the RetroArch items and must be updated by the tasks that change that behaviour.

All paths below are relative to `rewrite/` unless they start with `docs/`.

## The user's review decisions (binding)

1. Details popup: the right column overflows the dialog. Fix by letting the grid column shrink (`min-width: 0`).
2. Details popup: RomM sends `metadatum.first_release_date` in **milliseconds** (the header showed year "56322" and the Overview row showed "+056322-12"). Values are to be normalised to seconds.
3. Details popup: genres and rating were rendered three times (header line, header chip, genres paragraph, Overview grid). Remove the header **chip** (`details-rating`) and the genres **paragraph** (`details-genres`). Keep the header line and the Overview grid's Genres row.
4. (Not in this round — the user has not confirmed hiding "No default emulator" for native games. Do NOT touch it.)
5. Cards: the focus ring is clipped on the top, left and right because `.card` uses `content-visibility: auto` (paint containment) and the ring is drawn outside `.cover`. Draw the ring inside the cover.
6. Cards: covers of other shapes (square PS1 jewel cases, wide Genesis boxes, tall Switch boxes) are cropped. The user chose **option B**: keep the fixed 3:4 frame, fit the whole cover inside it (`object-fit: contain`), and fill the letterbox with a blurred, dimmed copy of the same cover.
7. RetroArch AppImage: pressing Play on a SNES game exits with `--libretro argument "cores/bsnes_libretro.so" is not a file … Ignoring` then `Frontend is built for dynamic libretro cores, but path is not set. Cannot continue.` The cores live in the AppImage portable home `<AppImage>.home/.config/retroarch/cores/` (where the platform-cores installer put them), but the launch resolver only looks in `<emulator dir>/cores`. Twin defect: the autoconfig writes `<emulator dir>/retroarch.cfg`, but RetroArch in portable-home mode reads `<AppImage>.home/.config/retroarch/retroarch.cfg`, so nothing we write is applied.

## Global Constraints

- **Token secrecy (hard):** tokens live only in the OS keyring and the redacting in-memory type; never in files, logs, errors, IPC, or console output. The RetroArch config on the dev machine contains RetroAchievements credentials: **never print, cat or quote a real `retroarch.cfg`**; tests use temp directories with fake content only.
- **Only `app.css` tokens for colours**; `--m-*` motion tokens. Literal `rgba()` scrims inside a card cover are allowed (the file already uses them).
- **Every test id E2E asserts today stays**: `details-header-line`, `details-verification`, `details-flags`, `details-version`, `details-playing-chip`, every `card-*`/`installed-badge-*`/`library-update-badge-*` id. The `details-rating` and `details-genres` ids are removed by decision 3 (no E2E spec reads them — Task 3 greps to prove it).
- **Every task ends with**, from `rewrite/`: `cargo fmt`; `cargo clippy --workspace --all-targets -- -D warnings` and `cargo clippy -p app --all-targets --features e2e -- -D warnings` clean; `cargo test --workspace` green **when Rust changed**; and from `rewrite/app`: `npm run check` (baseline: 3 pre-existing warnings — two in `Details.svelte`, one in `DownloadsFooter.svelte` — no new ones) and `npx vitest run` green. Then a commit whose subject starts `rewrite: `.
- **Never** run `git checkout`, `git restore`, `git reset`, or `git stash`. Commit with explicit pathspecs.
- **No component test harness exists** (no `@testing-library/svelte`, no jsdom). `.svelte` changes are verified by `npm run check` and E2E, never by a fabricated component test.
- The final task runs the E2E groups `images`, `library`, `launch`, `install` (`rewrite/scripts/e2e.sh images library launch install`, detached, log to a file) and they must be green.

---

## File map

| File | Responsibility |
|---|---|
| `crates/grid-core/src/autoconfig/paths.rs` | new `retroarch_portable_home(executable)` helper |
| `crates/grid-core/src/autoconfig/cores.rs` | `installed_core_ids_with_extension` uses the helper |
| `crates/grid-core/src/launch/template.rs` | `normalized_retroarch_core_args` takes the executable and searches the portable home first |
| `crates/grid-core/src/launch/spawn.rs` | passes the executable path |
| `crates/grid-core/src/autoconfig/retroarch.rs` | `config_path_candidates` puts the portable-home cfg first when the portable home exists |
| `crates/grid-core/src/romm/mod.rs` | `into_detail` normalises millisecond `first_release_date` |
| `crates/grid-core/tests/romm_detail.rs` | test for the ms case |
| `e2e/fixtures/rom-details.json`, `e2e/fixtures-emulator-catalog/rom-details.json` | fixtures switched to milliseconds (what RomM really sends) |
| `app/src/lib/Details.svelte` | `min-width: 0`; header chip + genres paragraph removed |
| `app/src/lib/GameCard.svelte`, `app/src/lib/Image.svelte`, `app/src/lib/cards/size.ts` | focus ring inside the cover; contain + blurred backdrop |
| `docs/porting/04-emulator-launch.md`, `docs/porting/05-emulator-autoconfig.md`, `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` | behaviour docs updated |

---

### Task 1: RetroArch portable-home helper + launch-time core resolution

**Files:**
- Modify: `crates/grid-core/src/autoconfig/paths.rs`
- Modify: `crates/grid-core/src/autoconfig/cores.rs:511-549`
- Modify: `crates/grid-core/src/launch/template.rs:292-325` and its tests at `:515-559`
- Modify: `crates/grid-core/src/launch/spawn.rs:80-94`
- Modify: `docs/porting/04-emulator-launch.md` (the `normalized_retroarch_core_args` paragraph, ~line 440)

**Interfaces:**
- Produces: `pub fn retroarch_portable_home(executable: &Path) -> Option<PathBuf>` in `autoconfig::paths` — returns `<parent>/<file name>.home/.config/retroarch` when that path exists and is a directory, else `None`. `executable` is the emulator file path (may not exist; then `None`).
- Produces: `pub fn normalized_retroarch_core_args(executable: &Path, args: Vec<String>) -> Vec<String>` (signature change: the executable, not its directory).

- [ ] **Step 1: Write the failing helper tests** in `paths.rs`'s test module:

```rust
#[test]
fn portable_home_is_found_next_to_an_appimage() {
    let temp = tempfile::tempdir().unwrap();
    let exe = temp.path().join("RetroArch-Linux-x86_64.AppImage");
    std::fs::write(&exe, b"").unwrap();
    let home = temp
        .path()
        .join("RetroArch-Linux-x86_64.AppImage.home")
        .join(".config")
        .join("retroarch");
    std::fs::create_dir_all(&home).unwrap();
    assert_eq!(retroarch_portable_home(&exe), Some(home));
}

#[test]
fn portable_home_is_none_without_the_home_dir() {
    let temp = tempfile::tempdir().unwrap();
    let exe = temp.path().join("retroarch");
    std::fs::write(&exe, b"").unwrap();
    assert_eq!(retroarch_portable_home(&exe), None);
}

#[test]
fn portable_home_is_none_when_home_is_a_file() {
    let temp = tempfile::tempdir().unwrap();
    let exe = temp.path().join("retroarch");
    std::fs::write(&exe, b"").unwrap();
    std::fs::write(temp.path().join("retroarch.home"), b"").unwrap();
    assert_eq!(retroarch_portable_home(&exe), None);
}
```

- [ ] **Step 2: Run** `cargo test -p grid-core paths::` — expect compile failure (function missing).

- [ ] **Step 3: Implement the helper** in `paths.rs`:

```rust
/// The RetroArch AppImage's portable home, `<parent>/<file name>.home/
/// .config/retroarch`, when that directory exists. The AppImage runtime
/// sets `$HOME` to `<AppImage>.home` whenever that directory exists next
/// to the file, so RetroArch then reads its `retroarch.cfg` and its
/// `cores/` from here rather than from the emulator directory. Both the
/// core installer (cores.rs) and the launch-time core resolver
/// (launch/template.rs) and the config writer (retroarch.rs) consult this
/// one rule so they can never disagree about the layout.
pub fn retroarch_portable_home(executable: &Path) -> Option<PathBuf> {
    let parent = executable.parent()?;
    let file_name = executable.file_name()?.to_string_lossy();
    let home = parent
        .join(format!("{file_name}.home"))
        .join(".config")
        .join("retroarch");
    home.is_dir().then_some(home)
}
```

- [ ] **Step 4: Use it in `cores.rs`** — replace the inline `appimage_home_cores` block (lines ~531-549) with:

```rust
        let parent = expanded.parent().unwrap_or_else(|| Path::new(""));
        match crate::autoconfig::paths::retroarch_portable_home(&expanded) {
            Some(home) if home.join("cores").is_dir() => home.join("cores"),
            _ => parent.join("cores"),
        }
```

Keep the doc comment's description accurate (it already describes this preference). Run `cargo test -p grid-core cores::` — must stay green.

- [ ] **Step 5: Write the failing launch tests** in `template.rs`. Change the four existing `normalize_*` tests so the first argument is an executable path inside the temp dir (create an empty `retroarch` file with `fs::write`), and add:

```rust
    #[test]
    fn normalize_prefers_the_appimage_portable_home_cores() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("RetroArch-Linux-x86_64.AppImage");
        fs::write(&exe, b"").unwrap();
        let home_cores = dir
            .path()
            .join("RetroArch-Linux-x86_64.AppImage.home")
            .join(".config")
            .join("retroarch")
            .join("cores");
        fs::create_dir_all(&home_cores).unwrap();
        let core_file = home_cores.join("bsnes_libretro.so");
        fs::write(&core_file, b"core bytes").unwrap();
        // No <emulator dir>/cores at all — the layout the user has.

        let args = vec![
            "-L".to_string(),
            "cores/bsnes_libretro.so".to_string(),
            "/roms/game.sfc".to_string(),
        ];
        let result = normalized_retroarch_core_args(&exe, args);
        let expected = fs::canonicalize(&core_file).unwrap();
        assert_eq!(result[1], expected.to_string_lossy());
    }

    #[test]
    fn normalize_falls_back_to_the_emulator_dir_when_the_home_lacks_the_core() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("RetroArch-Linux-x86_64.AppImage");
        fs::write(&exe, b"").unwrap();
        fs::create_dir_all(
            dir.path()
                .join("RetroArch-Linux-x86_64.AppImage.home")
                .join(".config")
                .join("retroarch"),
        )
        .unwrap();
        let cores_dir = dir.path().join("cores");
        fs::create_dir_all(&cores_dir).unwrap();
        let core_file = cores_dir.join("snes9x_libretro.so");
        fs::write(&core_file, b"core bytes").unwrap();

        let args = vec!["-L".to_string(), "cores/snes9x_libretro.so".to_string()];
        let result = normalized_retroarch_core_args(&exe, args);
        let expected = fs::canonicalize(&core_file).unwrap();
        assert_eq!(result[1], expected.to_string_lossy());
    }
```

- [ ] **Step 6: Run** `cargo test -p grid-core template::` — expect failures (signature / missing behaviour).

- [ ] **Step 7: Implement.** Change `normalized_retroarch_core_args` to take `executable: &Path`; compute `let emulator_dir = executable.parent().map(Path::to_path_buf).unwrap_or_default();` and a search list `[portable_home, emulator_dir]` (portable home only when `retroarch_portable_home(executable)` is `Some`). For each relative core token, take the first search dir where `dir.join(&core_path).is_file()`, canonicalise it, and substitute. Absolute tokens and tokens found in no dir are left untouched (unchanged behaviour). Update the function's doc comment to say why the portable home comes first. In `spawn.rs:86` pass `&executable` instead of `&working_dir` (the working directory stays the executable's parent).

- [ ] **Step 8: Run** `cargo test -p grid-core` — all green. Run both clippy commands and `cargo fmt`.

- [ ] **Step 9: Update `docs/porting/04-emulator-launch.md`** — the paragraph starting "RetroArch-only post-pass, `normalized_retroarch_core_args`" (~line 440): state that the rewrite searches `<exe>.home/.config/retroarch/<token>` first when that portable home exists, then `<emulator dir>/<token>`; mark it as a deliberate deviation from the Python behaviour (which only knew the emulator directory) with the reason (the AppImage runtime moves `$HOME` to `<AppImage>.home`, so the cores the platform-cores installer places there are the ones RetroArch can load).

- [ ] **Step 10: Commit**

```bash
git add crates/grid-core/src/autoconfig/paths.rs crates/grid-core/src/autoconfig/cores.rs crates/grid-core/src/launch/template.rs crates/grid-core/src/launch/spawn.rs ../docs/porting/04-emulator-launch.md
git commit -m "rewrite: resolve RetroArch cores in the AppImage portable home at launch"
```

---

### Task 2: RetroArch config target follows the portable home

**Files:**
- Modify: `crates/grid-core/src/autoconfig/retroarch.rs:31-76` (`config_path_candidates`) and its tests (~`:425-495`)
- Modify: `docs/porting/05-emulator-autoconfig.md` ("Config discovery", ~line 359)

**Interfaces:**
- Consumes: `crate::autoconfig::paths::retroarch_portable_home` from Task 1.

- [ ] **Step 1: Write the failing test** next to the other `candidates_*` tests (same `test_env::lock()` + `isolated_env` pattern as `candidates_use_parent_for_a_file_path_and_self_for_a_directory`):

```rust
    #[test]
    fn candidates_put_the_appimage_portable_home_cfg_first() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let dir = temp.path().join("RetroArch");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("RetroArch-Linux-x86_64.AppImage");
        std::fs::write(&exe, b"").unwrap();
        let home = dir
            .join("RetroArch-Linux-x86_64.AppImage.home")
            .join(".config")
            .join("retroarch");
        std::fs::create_dir_all(&home).unwrap();

        let candidates = config_path_candidates(exe.to_str().unwrap());
        assert_eq!(candidates[0], home.join("retroarch.cfg"));
        assert_eq!(candidates[1], dir.join("retroarch.cfg"));
    }
```

- [ ] **Step 2: Run** `cargo test -p grid-core candidates_` — the new test fails on `candidates[0]`.

- [ ] **Step 3: Implement.** In `config_path_candidates`, after computing `root`, build the list starting with `retroarch_portable_home(&expanded).map(|home| home.join("retroarch.cfg"))` when present, then the existing four entries. Update the doc comment: the portable-home entry is the one candidate that IS existence-gated (on the `.home/.config/retroarch` directory, not on the cfg file), because the AppImage runtime only redirects `$HOME` when that directory exists, and writing a cfg into a `.home` that RetroArch will not use would be wrong for every non-AppImage install.

- [ ] **Step 4: Run** `cargo test -p grid-core` — green (the deduplication test creates nothing on disk, so it is unaffected). Both clippy commands and `cargo fmt`.

- [ ] **Step 5: Update `docs/porting/05-emulator-autoconfig.md`** "Config discovery": add the new item 0 (`<exe>.home/.config/retroarch/retroarch.cfg`, only when that directory exists) and note it as a rewrite deviation with the same reason as Task 1. Add one line under "Target file" that on an AppImage install with a portable home the written file is therefore the portable-home cfg.

- [ ] **Step 6: Commit**

```bash
git add crates/grid-core/src/autoconfig/retroarch.rs ../docs/porting/05-emulator-autoconfig.md
git commit -m "rewrite: write retroarch.cfg into the AppImage portable home when it exists"
```

---

### Task 3: Release date in milliseconds

**Files:**
- Modify: `crates/grid-core/src/romm/mod.rs:578-581`
- Modify: `crates/grid-core/tests/romm_detail.rs`
- Modify: `e2e/fixtures/rom-details.json:31,117`, `e2e/fixtures-emulator-catalog/rom-details.json:28`

**Interfaces:**
- Produces: `RomDetail.first_release_date` is always epoch **seconds** as a string (unchanged contract for the frontend). Add a private `fn release_date_seconds(raw: i64) -> i64` in `romm/mod.rs`.

- [ ] **Step 1: Write the failing test** in `tests/romm_detail.rs`, modelled on `rom_detail_maps_full_payload` (same mock-server setup): a payload with `"first_release_date": 653529600000` must map to `"653529600"`. Add a second assertion in the same test or a sibling: `"first_release_date": 631152000` still maps to `"631152000"` (seconds pass through).

- [ ] **Step 2: Run** `cargo test -p grid-core --test romm_detail` — the ms case fails.

- [ ] **Step 3: Implement** in `romm/mod.rs`:

```rust
/// RomM stores IGDB's `first_release_date` in **milliseconds** (the value
/// the user's server sends renders as year 56322 when read as seconds),
/// while older payloads and the IGDB source use seconds. Anything above
/// 100_000_000_000 (year 5138 in seconds) can only be milliseconds, so it
/// is divided down; the frontend then always receives seconds.
fn release_date_seconds(raw: i64) -> i64 {
    if raw > 100_000_000_000 { raw / 1000 } else { raw }
}
```

and use it: `.map(|d| release_date_seconds(d).to_string())`. Add a `#[cfg(test)]` unit test in `mod.rs` for `release_date_seconds(653529600000) == 653529600`, `release_date_seconds(631152000) == 631152000`, `release_date_seconds(0) == 0`.

- [ ] **Step 4: Switch the E2E fixtures to milliseconds** (what the real server sends): `653529600` → `653529600000`, `315532800` → `315532800000`, `991353600` → `991353600000`. `images-a.spec.ts:107` asserts the header line contains `1990`; it must keep passing (653529600000 ms = 1990-09-17).

- [ ] **Step 5: Run** `cargo test --workspace`, both clippy commands, `cargo fmt`.

- [ ] **Step 6: Commit**

```bash
git add crates/grid-core/src/romm/mod.rs crates/grid-core/tests/romm_detail.rs e2e/fixtures/rom-details.json e2e/fixtures-emulator-catalog/rom-details.json
git commit -m "rewrite: normalise RomM's millisecond release dates to seconds"
```

---

### Task 4: Details popup — column overflow and duplicated metadata

**Files:**
- Modify: `app/src/lib/Details.svelte` (header markup ~`:599-626`, `.right` ~`:755`, `.tabpanel` ~`:865`, `.genres` CSS ~`:803`)

- [ ] **Step 1: Prove nothing reads the removed ids:** `grep -rn "details-rating\|details-genres" e2e/specs app/src` — expected: only `Details.svelte`. If a spec reads them, stop and report NEEDS_CONTEXT.

- [ ] **Step 2: Remove** the `{#if rating} <span class="chip" data-testid="details-rating">…` block and the `<p class="genres" data-testid="details-genres">{genres}</p>` line from the header. Keep the `rating` and `genres` derived values — `headerLine` still consumes both. Delete `.genres` from the `.header-line, .genres` CSS selector (leave `.header-line`).

- [ ] **Step 3: Fix the overflow.** Add `min-width: 0;` to `.right` and to `.tabpanel`. Also `overflow-wrap: anywhere;` on `.header-line` so a long title/companies line wraps instead of widening the column.

- [ ] **Step 4: Run** from `app/`: `npm run check` (no new warnings beyond the 3 baseline; if removing the chip or paragraph clears one of the two `Details.svelte` baseline warnings, that is fine — report the new count) and `npx vitest run`.

- [ ] **Step 5: Commit**

```bash
git add app/src/lib/Details.svelte
git commit -m "rewrite: keep the details popup's right column inside the dialog and show genres/rating once"
```

---

### Task 5: Cards — focus ring inside the cover, covers fitted with a blurred backdrop

**Files:**
- Modify: `app/src/lib/GameCard.svelte:139-143, 158-171`
- Modify: `app/src/lib/Image.svelte`
- Modify: `app/src/lib/cards/size.ts:56-61` (comment only)
- Modify: `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md:79`

**Interfaces:**
- Produces: `Image.svelte` gains an optional prop `backdrop?: boolean` (default `false`). When true and the image has loaded, it renders `<img class="backdrop" src={src} alt="" aria-hidden="true" loading="lazy" draggable="false" />` **before** the main `<img>`. The backdrop img gets no `{...rest}` spread. When `src` is null (placeholder) no backdrop is rendered.

- [ ] **Step 1: Prove no E2E spec counts `img` elements inside cards:** `grep -rn "img" e2e/specs/*.ts` — expected: no selector that would now match two images per card. If one exists, stop and report.

- [ ] **Step 2: Focus ring.** In `GameCard.svelte` change `.card.focused .cover` to:

```css
  /* Drawn INSIDE the cover: `.card` uses `content-visibility: auto`, whose
     paint containment clips anything painted outside the card box, so a
     ring with a positive offset lost its top, left and right edges. */
  .card.focused .cover {
    outline: 2px solid var(--primary);
    outline-offset: -2px;
  }
```

- [ ] **Step 3: Backdrop prop** in `Image.svelte` (see Interfaces). Keep the placeholder branch unchanged.

- [ ] **Step 4: Fit the cover.** In `GameCard.svelte`, pass `backdrop` to `<Image url={coverUrl} alt={title} placeholder="No cover" backdrop />` and replace the `.cover :global(img)` rule with:

```css
  /* The user's review choice ("option B"): the frame stays 3:4 so rows
     stay even, the whole cover fits inside it, and a blurred, dimmed copy
     of the same cover fills the letterbox for square (PS1) and wide
     (Genesis) art instead of cropping their sides. */
  .cover :global(img) {
    position: relative;
    width: 100%;
    height: 100%;
    object-fit: contain;
    display: block;
  }

  .cover :global(img.backdrop) {
    position: absolute;
    inset: -12px;
    width: calc(100% + 24px);
    height: calc(100% + 24px);
    object-fit: cover;
    filter: blur(10px) brightness(0.45);
    pointer-events: none;
  }
```

- [ ] **Step 5: Comment fix** in `cards/size.ts` on `CARD_COVER_RATIO`: "a loaded image with a different intrinsic ratio is fitted inside it (`object-fit: contain`) over a blurred copy of itself" — replace the "fills it with `object-fit: cover`" sentence. `size.test.ts:111` still expects `'3 / 4'`.

- [ ] **Step 6: Spec line 79** of the design doc: change "cover ratio from the image with a 3:4 fallback" to "fixed 3:4 frame; the cover is fitted inside it over a blurred, dimmed copy of itself (user decision 2026-09-05, replaces the image-ratio rule)".

- [ ] **Step 7: Run** from `app/`: `npm run check` and `npx vitest run` — green, no new warnings.

- [ ] **Step 8: Commit**

```bash
git add app/src/lib/GameCard.svelte app/src/lib/Image.svelte app/src/lib/cards/size.ts ../docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md
git commit -m "rewrite: fit every cover inside the 3:4 card over a blurred backdrop and keep the focus ring inside it"
```

---

### Task 6: E2E gate

**Files:** none modified unless a group fails.

- [ ] **Step 1:** From `rewrite/`, run detached and log: `nohup scripts/e2e.sh images library launch install > /tmp/claude-1000/-home-six-Documents-Programming-grid-launcher/d527a4be-8a2d-487c-bc02-e067fbdcf4ce/scratchpad/e2e-fixes1.log 2>&1 &` then poll the log until the summary line appears. The `images` group reads the release-date fixture and the details header; `library` exercises the cards; `launch` exercises RetroArch argv building; `install` opens the popup.
- [ ] **Step 2:** All four groups green. If one fails, read the failure, fix the cause within this plan's scope, re-run that group, commit the fix with a `rewrite: ` subject.
- [ ] **Step 3:** Report the per-group result lines verbatim.

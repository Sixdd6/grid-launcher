# Plan: first-run emulator setup

## Why
A new user connects, lands in an empty Server view, and has to find Emulators › Add from
catalog, install emulators one by one, then check Platform defaults. The first open should
end with the user's platforms ready to play. The design system's "First open" section
(GRID Launcher design system) sets the visual rules for these screens.

## User decisions (2026-10-28)
- **Emulator per platform:** GRID recommends one per platform, pre-selected, with a
  dropdown of the other compatible, installable emulators.
- **Platform list:** the server's platforms that need an emulator, with ROM counts, sorted
  by count, none selected.
- **While installing:** the user continues into the app; installs run in the background
  with a progress summary and the usual Downloads rows.
- **RetroArch:** used only for chosen platforms with no standalone emulator.
- **RetroArch needs a core (user requirement):** RetroArch is offered for a platform only
  when a core is known for it (`romm-platform-cores.json` by slug, else the core list by
  name), and is made that platform's default only when that core is on disk after the
  install. Otherwise the platform stays unassigned and the summary says so.

## Flow
1. **Connect** (existing screen, restyled to the design system):
   server URL, token (or username and password), library folder. The library folder
   becomes **required**, because nothing can install without it (`LibraryPathUnset`). It is
   pre-filled with a suggested folder and has Browse…. One primary button, disabled until
   complete.
2. **Choose your platforms** (new). Shown once, after the first successful connect, when
   setup has never completed:
   - One row per server platform that needs an emulator. Native PC platforms and the
     "Emulators" platform are excluded. Rows show the name and ROM count, sorted by count,
     with a checkbox, none ticked.
   - A ticked row shows the emulator select, with the recommendation pre-selected.
   - "Installed" shows when a compatible emulator is already configured.
   - "No emulator available" (unselectable) shows when nothing installable serves it.
   - Footer: primary **Install N emulators** (N = distinct emulators, not platforms) and
     secondary **Skip for now**.
3. **Into the app.** Installs are queued, and the Shell opens on the Server view as today.
   - A progress card ("Setting up 4 emulators · 1 done") links to Downloads.
   - It ends with a summary:
     - assigned platforms
     - failures (with Retry)
     - any platform left unassigned because its RetroArch core was missing
4. **Later.** Emulators › Platform defaults gets **Set up platforms…**, which opens the same
   picker. This is the way back for users who skipped.

Existing users never see step 2. Setup counts as done when
`[setup] completed = true`, **or** when the config already has any emulator or any
installed game (an upgrade from an older build).

## Backend (grid-core + Tauri)
1. **Fix the defaults key mismatch first** (a separate commit). Auto-assignment writes
   `[default_emulators]` under the raw server `name`. The UI and `set_default_emulator`
   use the display label (`defaults.ts:16-18`, `commands.rs:222-231`). Settle on the
   label everywhere, read the old key as a fallback, and migrate it on write. Tests cover
   a custom-named platform.
2. **Recommendation data:** `platform-emulators.json` at the repo root, embedded like the
   other data files. It maps a RomM slug to an ordered list of profile names, for example:
   - `ps2` → PCSX2
   - `xbox360` → Xenia Canary, then Xenia Edge
   - `dc` → Redream

   The rules:
   - The first entry that is installable on this OS is the recommendation.
   - A platform missing from the table falls back to keyword matching, standalone first.
   - RetroArch is used only when no standalone emulator matches **and**
     `retroarch_core_for_platform(slug, name)` finds a core in the core list.
   - A unit test checks that every table entry names a real, sourced profile.
3. **`setup_platform_options(platforms)`** → for each platform: its label, slug, ROM
   count, `recommended: source_id | null`, `options: [{source_id, name, kind:
   standalone|retroarch, installed}]`, and `core` (the core RetroArch would use). It is
   pure logic in grid-core (`launch/setup.rs`) with unit tests, and gated by host OS,
   including `WINDOWS_ONLY_SLUGS`, which the catalog path skips today.
4. **`start_platform_setup(choices: [{platform_label, source_id}])`**:
   - Validates each choice against `setup_platform_options`.
   - Admits one `install_emulator` per distinct `source_id`. The existing queue
     de-duplicates and adds companions.
   - Records the wanted defaults in `[setup.pending]` (platform label → emulator name).
   - Sets `[setup] completed = true`.
   - Returns the queued job ids.

   Applying the defaults, in the emulator-installed hook (`lib.rs:439-461`):
   - For each pending platform whose emulator just installed, call the same logic as
     `set_default_emulator`. That overrides an auto-assigned default, but never a user's
     explicit "(none)".
   - For a RetroArch choice, assign it and record the core only if
     `installed_compatible_cores` finds the core on disk. Otherwise leave the platform
     unassigned and emit `setup-platform-unassigned {platform, reason: "core missing"}`.
   - Clear the pending entry.
   - A failed install keeps its entries pending, so Retry still applies them.
5. **`setup_status()`** → `{needed, pending: [...]}`, for the app to decide whether to show
   step 2 and to drive the summary.

No new RomM endpoints: the platform list is the existing `list_platforms`. Installs talk
only to the forge, so this needs no api-tester run.

## Frontend
- `app/src/lib/setup/choices.ts` (pure, vitest):
  - the selection model (ticked platforms, emulator per platform)
  - "Install N emulators", counting distinct emulators
  - rows sorted by ROM count
  - which rows show "Installed" or "No emulator available"
- `app/src/lib/setup/PlatformSetup.svelte`: the step-2 screen, built from the design
  system's TextField, Button and Notice rules.
- `app/src/lib/stores/session.svelte.ts` / `App.svelte`: a `setup` phase between connect
  and `shell`, shown when `setup_status().needed`.
- `app/src/lib/setup/SetupProgress.svelte`: the in-shell progress and summary card, fed by
  the downloads store and the `emulator-installed` / `setup-platform-unassigned` events.
- `Connect.svelte`: the library folder is required, with a suggested default; restyled
  to the design system.
- Emulators › Platform defaults: a **Set up platforms…** button.

## Tests
- **Rust unit tests:**
  - recommendation order and OS gating
  - the RetroArch fallback only when a core exists
  - no RetroArch default when the core is missing on disk
  - pending defaults applied on install, kept on failure, never overriding "(none)"
  - the defaults-key migration
- **vitest:** the selection model and its counts.
- **e2e:** a new stage `first-run-setup`, using fixtures with PS2, Dreamcast and a
  platform with no emulator. It runs connect, picks PS2 and Dreamcast, lands in the app
  and asserts both installed and assigned, using the mock forge's existing PCSX2 and
  Redream routes. RetroArch's two-archive install has no mock-forge route, so its path
  is covered by unit tests only.
- The gate, then `E2E_SKIP_BUILD=1 scripts/e2e.sh first-run-setup connect emulator-catalog`.

## Docs
- SPEC.md "First Run Setup": the new step, the required library folder, the
  Set up platforms… re-entry, and the RetroArch core rule.
- The design system's "First open" section: the platform picker.

## Order of work (each step gated and committed on its own)
1. Defaults key fix.
2. Recommendation table and `setup_platform_options`, with tests.
3. `start_platform_setup`, pending defaults and the RetroArch core check, with tests.
4. Connect: required library folder and restyle.
5. The platform picker and the setup phase.
6. In-shell progress and summary, and the Set up platforms… re-entry.
7. The e2e stage, the SPEC and the design system.

## Open points (defaults chosen; say if any is wrong)
- Suggested library folder: `~/Games/GRID` on Linux, `%USERPROFILE%\Games\GRID` on
  Windows.
- Xbox 360 recommends Xenia Canary.
- KytyPS5 is not recommended for PS5, being experimental: PS5 shows "No emulator
  available".
- Arcade uses RetroArch, because FBNeo has no download source. The core comes from the
  slug map (`arcade`), and the summary says so if it's missing.

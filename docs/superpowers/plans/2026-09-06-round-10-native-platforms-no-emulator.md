# Round 10 — Native Platforms Need No Emulator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Windows 9x (and every other native Windows/Linux platform) is never presented as needing an emulator: no "No default emulator" chip in the Server view and no emulator selector in the Emulators view, while install and launch keep using the native paths they already use.

**Architecture:** The native predicate already exists on both sides (`grid_core::library::platforms::is_native_platform` and the TS `isNativeLaunchPlatform` in `details/cloud.ts`, both "starts with windows" / "windows or linux"). The Server header and the Emulators defaults pane simply never consulted it. Two rendering guards, one shared TS helper, and an E2E case on the "Windows 9x" fixture platform (id 3, added in round 9).

**Tech Stack:** Svelte 5 + TypeScript, vitest, WebdriverIO E2E.

**Spec:** `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` §6 (Server header chips) and §9 (Emulators › Defaults). User ruling 2026-09-05 (parity round): "the native games 'No default emulator' can be hidden for linux and windows-based platforms"; user ruling 2026-09-06: Windows 9x is native like any other Windows platform.

**Evidence (2026-09-06):** `Server.svelte:517-519` renders `server-emulator-chip` with `emulatorChipLabel(defaultEmulator)` for every platform; `Emulators.svelte:697-703` renders a default-emulator row for every platform in `platforms`. The rom's `platform_display_name` for the live Windows 9x platform is "Windows 9x", which `is_native_platform`/`isNativePlatform` accept (prefix match), so `install`, `launch`, the Details install label and the launch-target line already take the native path.

## Global Constraints

- Colours only via `app.css` tokens; no component test harness except SSR `render` from `svelte/server`.
- No `git checkout` / `git restore` / `git reset` / `git stash`. Commit from the repo root with `git commit --only -- <paths>`; subjects start with `rewrite: `.
- Frontend gates, from `rewrite/app`: `npm run check` (baseline 3 warnings: Details.svelte ×2, DownloadsFooter.svelte ×1 — no new ones), `npx vitest run`. E2E: `npm run typecheck` in `rewrite/e2e`; groups via `bash rewrite/scripts/e2e.sh <group…>` from the repo root; never `E2E_SKIP_BUILD=1` after a source change.
- All `rewrite/` paths below are relative to `rewrite/`.

---

### Task 1: Hide emulator UI for native platforms

**Files:**
- Modify: `app/src/lib/server/header.ts`, `app/src/lib/server/header.test.ts` (`showsEmulatorChip(platformLabel: string): boolean` = `!isNativeLaunchPlatform(platformLabel)`)
- Modify: `app/src/lib/Server.svelte` (~line 517: wrap the chip in `{#if showsEmulatorChip(activePlatformName)}`; the firmware chip stays as it is)
- Modify: `app/src/lib/emulators/defaults.ts`, `app/src/lib/emulators/defaults.test.ts` (`needsEmulator(p): boolean` = `!isNativeLaunchPlatform(platformLabel(p))`)
- Modify: `app/src/lib/Emulators.svelte` (~lines 697–703: a native platform's row shows the label and the muted text "Native — runs without an emulator" (test id `emulator-default-native-<id>`) instead of the select; the `defaults` count in the pane header (~line 170) counts only platforms that need one)
- Modify: `e2e/specs/library.spec.ts` (one case) and `e2e/specs/emulators.spec.ts` (one case)
- Modify: `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` (§6 and §9 one sentence each)

**Interfaces:**
- `isNativeLaunchPlatform` is imported from `details/cloud.ts` (existing; "windows" or "linux" prefix, case-folded, trimmed) — do not duplicate the rule.
- `platformLabel` from `server/platformLabel.ts` (round 9) is the identity used everywhere a platform is keyed.

- [ ] **Step 1: Tests first** — `header.test.ts`: `showsEmulatorChip('Windows 9x')` false, `'Windows'` false, `'Linux'` false, `'Super Nintendo Entertainment System'` true, `''` true. `defaults.test.ts`: `needsEmulator` for a platform whose `display_name` is "Windows 9x" and `name` "Windows" → false; SNES → true. Run `npx vitest run header defaults` → FAIL.
- [ ] **Step 2: Implement** the helpers and the two guards. `npm run check`, `npx vitest run` → PASS.
- [ ] **Step 3: E2E** — `library.spec.ts`: after connecting, click `platform-btn-3` ("Windows 9x"), assert `server-emulator-chip` does NOT exist, then click `platform-btn-1` and assert it exists again with text "No default emulator". `emulators.spec.ts`: open the Emulators view's Defaults pane (find how the existing cases reach it), assert `emulator-default-native-3` exists with the native text and that no default select exists for platform 3 (`grep -n 'emulator-default-' e2e/specs/emulators.spec.ts` for the select's test-id pattern). Run `bash rewrite/scripts/e2e.sh library emulators` (full build).
- [ ] **Step 4: Spec** — §6: "the emulator chip is omitted for native platforms (Windows, Linux — prefix match on the display label)"; §9: "native platforms list as 'Native — runs without an emulator' and offer no selector".
- [ ] **Step 5: Commit** — `git commit --only -- <paths> -m "rewrite: native platforms never ask for an emulator"`.

---

## Verification after the task

1. Frontend gates; the `library` and `emulators` E2E groups, then the full suite.
2. Hand-test note: the Windows 9x platform shows no emulator chip, installs a game natively, and launches it through the native launcher like the Windows platform.

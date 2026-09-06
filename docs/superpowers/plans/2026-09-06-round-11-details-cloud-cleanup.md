# Round 11 — Details Cloud Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the redundant cloud button from the details popup's left column, and stop telling PC-game players that cloud saves are emulator-only.

**Architecture:** The left-column button (`details-cloud-status`, label "Cloud saves" / "Not configured") only routes to the Saves tab, which is already a tab. It goes, with its label helper and tests. The false sentence comes from `grid_core::cloud::scope::cloud_save_block_reason`, which answers it for every native platform regardless of save type; native games DO sync their save folders (the Saves tab's native panel), and only save states are unavailable for them. The sentence is replaced by one that says exactly that.

**Tech Stack:** Rust (grid-core `cloud::scope`), Svelte 5 + TypeScript, vitest, WebdriverIO E2E.

**Spec:** `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` §7 (details popup left column and Saves tab).

**Evidence (2026-09-06):** `Details.svelte:655-657` renders the button with `cloudStatusLabel` (`details/header.ts:128-130`); no E2E spec references `details-cloud-status`. `cloud/scope.rs:167-169` returns "Cloud save management is only available for emulator-based games." whenever `is_native_executable_platform(platform)`; `CloudPanel.svelte:161` shows it as the block reason for a native game's STATE panel, and `SavesTab.svelte:58` shows it whenever the save panel is unsupported. Tests pin the old string at `scope.rs:333`, `cloud/ops/tests.rs:1776`, `e2e/specs/cloud-saves.spec.ts:326`.

## Global Constraints

- Token secrecy: nothing secret in logs, errors, IPC or console output.
- No `git checkout` / `git restore` / `git reset` / `git stash`. Commit from the repo root with `git commit --only -- <paths>`; subjects start with `rewrite: `.
- Rust gates, from `rewrite/`: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p app --all-targets --features e2e -- -D warnings`, `cargo test --workspace`; repo root: `bash rewrite/scripts/check_secret_hygiene.sh`. Frontend, from `rewrite/app`: `npm run check` (baseline 3 warnings: Details.svelte ×2, DownloadsFooter.svelte ×1 — the count may DROP if the removed markup carried one; it must not rise), `npx vitest run`. E2E: `npm run typecheck` in `rewrite/e2e`; groups via `bash rewrite/scripts/e2e.sh <group…>` from the repo root; never `E2E_SKIP_BUILD=1` after a source change.
- All `rewrite/` paths below are relative to `rewrite/`.

---

### Task 1: Drop the left-column cloud button; fix the native block reason

**Files:**
- Modify: `app/src/lib/Details.svelte` (~lines 655–657: delete the button; delete the `cloudStatusLabel` import ~line 41 and the `openCloud` handler if nothing else uses it — `grep -n openCloud`)
- Modify: `app/src/lib/details/header.ts` (delete `cloudStatusLabel` and its doc comment), `app/src/lib/details/header.test.ts` (delete its `describe`)
- Modify: `crates/grid-core/src/cloud/scope.rs` (~line 168 string and its test ~line 333), `crates/grid-core/src/cloud/ops/tests.rs` (~line 1776), `e2e/specs/cloud-saves.spec.ts` (~line 326)
- Modify: `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` (§7: remove the left-column cloud button from the description; note the native wording)
- Modify: `docs/porting/*.md` only if one quotes the old sentence (`grep -rn 'emulator-based games' docs/porting`)

**Interfaces:**
- New string, exact: `Save states are not available for PC games; their save folders sync from the Saves tab instead.`
- `cloud_save_block_reason`'s signature and every other branch are unchanged.

- [ ] **Step 1: Rust tests first** — update the two Rust assertions to the new string; `cargo test -p grid-core cloud` → FAIL; change the string → PASS; the Rust gate list.
- [ ] **Step 2: Frontend** — delete the button, helper, import, handler and the helper's tests; `npm run check`, `npx vitest run` → PASS. Confirm with `grep -rn 'details-cloud-status\|cloudStatusLabel' app/src e2e` → nothing.
- [ ] **Step 3: E2E** — update the `cloud-saves.spec.ts` expectation; `bash rewrite/scripts/e2e.sh cloud-saves native` (full build).
- [ ] **Step 4: Spec** — §7 edits.
- [ ] **Step 5: Commit** — `git commit --only -- <paths> -m "rewrite: drop the details cloud button and stop calling PC saves emulator-only"`.

---

## Verification after the task

1. Full gate list; the `cloud-saves` and `native` E2E groups, then the full suite.
2. Hand-test note: a PC game's details popup shows no cloud button in the left column; its Saves tab explains that only save states are unavailable.

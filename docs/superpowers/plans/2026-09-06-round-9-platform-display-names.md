# Round 9 — Platform Display Names Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Server view labels platforms the way RomM's own web UI does, so the two Windows platforms read "Windows" and "Windows 9x".

**Architecture:** RomM's platform record carries `name` (the canonical name), `custom_name` (user-set, nullable) and `display_name` (what the web UI shows: the custom name when set, else the name). The rewrite's `Platform` only reads `name`. It gains the two fields and a `label` that mirrors the Python app's rule — first non-empty of `display_name`, `name`, `slug` (`docs/porting/01-romm-api.md:147`) — and the frontend renders that label wherever a platform name is shown. Matching logic (emulator/core lookups by `name`/`slug`, native-platform predicates) is untouched.

**Tech Stack:** Rust (grid-core `romm` module), Svelte 5 + TypeScript, vitest, WebdriverIO E2E with the mock RomM server.

**Spec:** `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` §6 (Server view rail). `docs/porting/01-romm-api.md` already states the label rule.

**Evidence (live server, 2026-09-06, read-only):** 51 platforms; only id 50 differs: `slug=win`, `fs_slug=win9x`, `name=Windows`, `custom_name=Windows 9x`, `display_name=Windows 9x`, and its roms carry `platform_display_name=Windows 9x`. Platform id 2 is `slug=win`, `fs_slug=win`, `name=Windows`, `display_name=Windows`. Installed rows already store the rom's `platform_display_name`, so the Library rail is right; the Server rail is not. Note both platforms share `slug=win`; nothing in the rewrite may key platform identity on slug alone.

## Global Constraints

- Token secrecy: tokens only in the OS keyring and the redacting in-memory type; never in files, logs, errors, IPC, console output or URLs.
- No `git checkout` / `git restore` / `git reset` / `git stash`. Commit from the repo root with `git commit --only -- <paths>`; subjects start with `rewrite: `.
- Rust gates, from `rewrite/`: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p app --all-targets --features e2e -- -D warnings`, `cargo test --workspace`; repo root: `bash rewrite/scripts/check_secret_hygiene.sh`. Frontend, from `rewrite/app`: `npm run check` (baseline 3 warnings: Details.svelte ×2, DownloadsFooter.svelte ×1 — no new ones), `npx vitest run`. E2E: `npm run typecheck` in `rewrite/e2e`; groups via `bash rewrite/scripts/e2e.sh <group…>` from the repo root; never `E2E_SKIP_BUILD=1` after a source change.
- `openapi.json` at the repo root is the wire reference for `PlatformSchema` (`custom_name` nullable string, `display_name` read-only string).
- All `rewrite/` paths below are relative to `rewrite/`.

---

### Task 1: Platform labels use `display_name`

**Files:**
- Modify: `crates/grid-core/src/romm/mod.rs` (`Platform` ~line 203; its tests)
- Modify: `app/src/lib/api.ts` (`Platform` type ~line 11)
- Create: `app/src/lib/server/platformLabel.ts`, `app/src/lib/server/platformLabel.test.ts`
- Modify: `app/src/lib/Server.svelte` (~line 86 `activePlatformName`, ~line 103 rail `label: p.name`, and any other place a platform's `name` is displayed — `grep -n '\.name' app/src/lib/Server.svelte` and judge each: game names stay, platform names use the label)
- Modify: `e2e/fixtures/platforms.json`, `e2e/fixtures/roms.json`, `e2e/fixtures/rom-details.json` (a third platform mirroring the live shape), `e2e/specs/library.spec.ts` (one assertion)
- Modify: `docs/porting/01-romm-api.md` (the `/api/platforms` row ~line 41: the rewrite now reads `custom_name` and `display_name` too)

**Interfaces:**
- Rust:

```rust
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct Platform {
    pub id: i64,
    pub name: String,
    pub slug: String,
    #[serde(default)]
    pub rom_count: i64,
    /// User-set name from RomM's platform settings; `None`/empty when unset.
    #[serde(default)]
    pub custom_name: Option<String>,
    /// What RomM's web UI shows: the custom name when set, else `name`.
    /// Read-only on the wire; older servers omit it (then it is empty).
    #[serde(default)]
    pub display_name: String,
}

impl Platform {
    /// The label the UI shows — `display_name`, else `name`, else `slug`
    /// (`grid_launcher/server/catalog.py`, docs/porting/01-romm-api.md:147).
    pub fn label(&self) -> &str { … }
}
```

  Tests: a payload with `display_name` "Windows 9x" and `name` "Windows" → `label()` "Windows 9x"; a payload without `display_name`/`custom_name` (older server) still deserialises and labels by `name`; an empty `name` labels by `slug`.
- TypeScript: `Platform` gains `custom_name: string | null; display_name: string;`; `platformLabel(p: Pick<Platform, 'display_name' | 'name' | 'slug'>): string` with the same rule and three tests; `Server.svelte` uses it for the rail label and `activePlatformName`.
- E2E fixture: `platforms.json` gains `{ "id": 3, "name": "Windows", "slug": "win", "fs_slug": "win9x", "custom_name": "Windows 9x", "display_name": "Windows 9x", "rom_count": 1 }` and, to keep the live shape, platform 1 and 2 entries gain `"custom_name": null, "display_name": "<their name>"`; `roms.json` gains a `"3"` list with one rom `{ "id": 302, "name": "Win9x Game", "fs_name_no_ext": "Win9x Game", "platform_id": 3, "path_cover_small": null }`; `rom-details.json` gains `302` modelled on `301` with `platform_id` 3 and `platform_display_name` "Windows 9x". `library.spec.ts` "renders both platforms from the fixtures" becomes "renders the platforms from the fixtures" and additionally asserts `platform-btn-3` has text containing "Windows 9x" and `platform-btn-1` does not contain "Windows". Check `library-grid.spec.ts`'s count assertions (`server-platform-counts` "3 games · 0 installed") still hold — they are per selected platform.

- [ ] **Step 1: Rust tests first** — the three `Platform` cases above in `romm/mod.rs`'s test module; run `cargo test -p grid-core platform` → FAIL.
- [ ] **Step 2: Implement** the struct fields and `label()`; run → PASS; the Rust gate list.
- [ ] **Step 3: Frontend tests first** — `platformLabel.test.ts` (three cases); run `npx vitest run platformLabel` → FAIL; implement; wire `Server.svelte`; `npm run check`, `npx vitest run` → PASS.
- [ ] **Step 4: E2E** — fixtures + spec; `cd rewrite/e2e && npm run typecheck`; `bash rewrite/scripts/e2e.sh library` (full build) and, because the fixtures feed every group, the full suite: `bash rewrite/scripts/e2e.sh`.
- [ ] **Step 5: Doc** — the porting doc row.
- [ ] **Step 6: Commit** — `git commit --only -- <paths> -m "rewrite: label platforms with RomM's display name"`.

---

## Verification after the task

1. Full gate list; the full E2E suite (Step 4).
2. Hand-test note: the Server rail should read "Windows" and "Windows 9x"; the Library rail was already right.

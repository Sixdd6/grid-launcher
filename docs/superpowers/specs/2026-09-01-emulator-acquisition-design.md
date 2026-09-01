# Rust rewrite milestone 4 — automated emulator setup (design)

**Status:** approved design, pre-implementation
**Behavior contract:** `docs/porting/04-emulator-launch.md` §12 (+ the source
metadata table) and doc 03's emulator-source install path; the executable
selector's reference is `grid_launcher/emulator/autoconfig.py:19-90`; the
resolver's references are `grid_launcher/emulator/source.py` and
`grid_launcher/background/workers.py:165-260`.
**Builds on:** milestones 1–3.5 (all merged; E2E harness is the gate).

## User journey

Emulators → Add opens on a catalog of known emulators available for this
platform. One click on Install: the download appears in the downloads drawer
like any game, and on completion the emulator exists in the list — name,
arguments, and executable configured, ready to set as a platform default and
play. No binary paths typed, no AppImage hunting. Manual add remains a second
tab; typing a known emulator name there auto-fills its arguments.

## Scope

In scope:

- Source metadata model + normalization (github/gitea/direct incl. alias
  table, tag fallback chain, asset include/exclude/preferred globs,
  `platform_overrides` merge, `platforms` prefix allowlist +
  `manual_install_hint`, `allow_prerelease`, `base_url` for gitea,
  `download_url`/`page_url`/`download_url_regex`/`asset_name` for direct,
  `supplemental_downloads`).
- Release + asset resolution (GitHub and Gitea APIs; direct with the
  page-scrape path — REQUIRED: RetroArch and Redream ship as `direct` with
  `page_url`), per doc 04 §12's exact selection rules.
- Catalog listing/filter per §12 (valid source, platform-gated, dedupe on
  casefolded (name, provider, owner, repo), sort by (name, source_id),
  already-installed filtered by entry name and source_id, AND-of-tokens
  search).
- Install through the existing InstallService queue as a new job kind
  (drawer rows, progress, cancel/retry as for games).
- Install dir `<library>/Emulators/<sanitized stem of archive name>`;
  extraction via the milestone-2 engine; supplemental archives downloaded as
  `<stem>-supplemental-<n><suffix>` siblings and merged into the install dir
  after the main extraction (doc 03 merge semantics: temp dir, copy-over,
  keep unrelated files).
- Executable selection ported verbatim from
  `select_emulator_executable_path` (title-token scoring, the
  eden.exe/azahar.exe preferred-name special cases, `.exe` preference,
  shallowest, casefolded tie-break; the launchable predicate is the
  emulator suffix set `.exe .bat .cmd .ps1 .sh .appimage`), chmod `0o755`
  on unix.
- EmulatorEntry creation from the profile (name via the doc's
  `emulator_profile_for_game` rules incl. `use_game_title_as_name`; args
  from the profile) and NEW optional persisted fields on `EmulatorEntry`:
  `source_id`, `source_provider`, `source_owner`, `source_repo`,
  `source_release_tag` (all `#[serde(default)]`, omitted-when-empty
  serialization; enables future update checks).
- Manual-add auto-fill from NAME: when path and args are both empty and the
  typed name casefold-matches a profile name (exact, else unique substring),
  fill args (and show the profile's display name).
- E2E: a mock forge + a new spec group (see Testing).
- Carried cleanup task (one commit): milestone-2 `installed_match`
  blank-key latent gap + its self-contradictory comment
  (`library/mod.rs` ~336); `CORE_OPTION_TOKENS` reuse in
  `apply_placeholders`; Emulators-panel defaults-select "(none)"
  sentinel/name collision and case/verbatim default matching.

Out of scope (later milestones): compat tools (GE-Proton/Proton-CachyOS
profiles excluded from the catalog), `windows_assets`/`windows_arch`,
firmware after install, version checks/updates (`source_emulator_update`),
per-emulator config writers (doc 05), the `ShadPS4 Qt Launcher`-style
windows-only entries (platform-gated out anyway).

## Global constraints

- Secret rules unchanged; forge requests carry NO Authorization header (the
  reference sends only Accept/API-version/User-Agent for GitHub — port
  those: `Accept: application/vnd.github+json`,
  `X-GitHub-Api-Version: 2022-11-28`, `User-Agent: grid-launcher`; gitea
  sends no extra headers). RomM credentials must never reach a forge host —
  the forge client is a separate reqwest client with no auth header at all.
- The E2E forge redirect (below) is compiled ONLY under the `e2e` feature —
  release builds cannot be redirected. Same standing as the M3.5 release
  rules; the hygiene guard's `cargo tree` checks already cover the feature
  gating pattern, and the redirect must live behind `#[cfg(feature = "e2e")]`.
- grid-core never imports Tauri; errors as Display strings; existing suites
  + the full E2E suite green at every commit; every new flow lands in the
  E2E suite in the same milestone.

## Architecture

```
rewrite/crates/grid-core/src/launch/source.rs   metadata model + normalization +
                                                pure release/asset selection
rewrite/crates/grid-core/src/launch/forge.rs    HTTP: GitHub/Gitea release fetch,
                                                direct page scrape; separate
                                                unauthenticated reqwest client;
                                                e2e-gated base-URL override
rewrite/crates/grid-core/src/launch/catalog.rs  listing/filter/dedupe/sort
rewrite/crates/grid-core/src/launch/emu_install.rs
                                                executable selection + entry
                                                creation + install-dir rules
rewrite/crates/grid-core/src/library/…          InstallService gains the
                                                emulator job kind
```

### Resolution rules (doc 04 §12, normative)

- Provider aliases → `github` (the github-release family), `gitea`,
  `direct`; unrecognized → error at resolve time, pass-through at listing.
- Tag: `release_tag` → `tag` → `version` → `"latest"`; `"latest"` treated as
  unset for selection; endpoint choice: `/releases/tags/{tag}` (explicit),
  `/releases/latest` (literal latest), `/releases` (unset).
- Release selection: drafts always skipped; prereleases skipped unless
  `allow_prerelease`; explicit tag matches `tag_name` case-insensitively;
  else first surviving in list order; failure lists available tags.
- Asset selection: needs `name` + `browser_download_url`; must match ≥1
  include glob and 0 exclude globs (fnmatch, casefolded); sort by
  `(include_index, preferred_index (len when unmatched), state_penalty
  (0 for ""/"uploaded"), casefolded name)`, lowest wins.
- `platform_overrides`: first entry whose key is a prefix of the host
  platform string (`linux` on this target) merges over the source before
  resolution.
- Direct: `platforms` allowlist (prefix match) enforced with
  `manual_install_hint` appended to the failure; `download_url` used
  verbatim when present, else `page_url` fetched and scanned — an
  `href="…"` attribute matching `download_url_regex` wins, else a whole-page
  regex hit (first non-empty capture group, else whole match), joined with
  the page URL; `asset_name` defaults to the URL basename.
- Supplementals: each entry resolved by the same rules; downloaded next to
  the primary as `<stem>-supplemental-<n><suffix>` (asset-name suffix, else
  archive suffix, else `.zip`); merged into the install dir after main
  extraction. Failures are visible failures: a supplemental download error
  fails the download phase; a merge error fails finalize. (The reference
  folded merge errors into joined warning text — that is deviation 4.)

### Install pipeline integration

`InstallService` gains `install_emulator(profile_source_id)`-style entry:
admission through the same queue (dedupe key = the catalog source_id),
download via the existing multi-target downloader (primary + supplementals
as targets — content-length totals when known), finalize = extract → merge
supplementals → select executable → chmod → create/update the EmulatorEntry
in config (load-modify-save; replace an existing entry with the same name
in place) → drawer row Completed. The registry (SQLite) is NOT involved —
emulators are config entries, not installed_games rows (deviation from the
reference, which recorded emulators as pseudo-games; recorded as deviation 1).
Archive name: `<sanitized profile name>-<sanitized tag>.zip` default with the
worker's asset-suffix rewrite (AppImage replaces whole name; differing
suffix replaces suffix).

### E2E forge override (e2e feature only)

`forge.rs` resolves API bases through
`#[cfg(feature = "e2e")] std::env::var("GRID_LAUNCHER_E2E_FORGE_BASE")`:
when set, GitHub API, Gitea base_url, and direct page/download URLs are
rewritten to `<base>/<original host>/<path>` so the mock can serve
everything. Not compiled otherwise.

## IPC + frontend

Commands: `list_emulator_catalog() -> Vec<CatalogEntry{name, source_id,
provider, tag, installed: bool}>` (search filtering client-side),
`install_emulator(source_id: String)`. The downloads event stream already
carries the rows. Emulators panel: Add opens the catalog tab (list +
search + Install buttons, installed entries disabled with a label), Manual
tab holds the existing form with the new name-based auto-fill. Install
errors surface on the drawer row as today.

## Deliberate deviations (recorded in doc 04 at merge)

1. Installed emulators are config entries only — never pseudo-rows in the
   installed-games registry (the reference listed them as library items).
2. Compat-tool profiles are excluded from the catalog entirely this
   milestone (reference listed them in a separate dialog).
3. Version checks deferred; `source_*` fields recorded now.
4. Supplemental failures fail the install (visible) rather than partially
   succeeding.
5. No firmware step after emulator install (firmware subsystem deferred).

## Testing

- source.rs/catalog.rs/emu_install.rs: pure-unit tables for every rule above
  (release/asset selection incl. state_penalty and preferred ordering; alias
  and tag chains; platform_overrides merge; scrape href-vs-page precedence
  with fixture HTML; catalog dedupe/sort/filter; executable selector table
  incl. the eden/azahar preferred names, token scoring, AppImage pick).
- forge.rs: wiremock for GitHub/Gitea endpoints and the direct page fetch;
  header assertions (the three GitHub headers, no Authorization anywhere).
- InstallService: integration tests for the emulator job kind (queue dedupe,
  finalize creates the entry, supplemental merge, failure keeps archive).
- Mock forge for E2E: extend the mock server (or sibling `mock-forge.mjs`)
  with GitHub-shaped `/repos/{o}/{r}/releases/latest` + asset bytes (zip
  containing an executable stub script) and a direct-provider page with an
  href for the scrape path. Unit tests included.
- New E2E spec group `emulator-catalog`: open catalog → entries listed →
  Install a github-provider stub emulator → drawer completes → emulator row
  exists with the profile's args → set as default → Play the seeded game
  with it (argv file assertion) → catalog now shows it installed/disabled.
  A second scenario installs the direct-provider (scrape) stub.
- The carried-cleanup task keeps all existing tests green and adds: a
  regression test for the M2 blank-key gap fix; a defaults-select test if
  cheap (else svelte-check only, noted).

## Exit gate

Full `rewrite/scripts/e2e.sh` green locally (all groups incl.
`emulator-catalog`) and the CI job green on the pushed branch; all other
suites green. Merge to main follows automatically (per the standing testing
model); the user's formal pass stays scheduled for the end of the rewrite
phase.

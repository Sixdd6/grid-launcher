# Automated Emulator Setup Implementation Plan (rewrite milestone 4)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Catalog-driven emulator download/install (GitHub, Gitea, direct with
page scrape) through the existing InstallService queue, with manual-add
name auto-fill, an `emulator-catalog` E2E group, and the carried M2/M3
cleanup items.

**Architecture:** New pure modules `launch/source.rs` (metadata
normalization + release/asset selection), `launch/catalog.rs` (listing),
`launch/emu_install.rs` (executable selection, naming, entry creation);
one HTTP module `launch/forge.rs` (separate unauthenticated reqwest
client, e2e-gated URL rewrite); the queue gains a job-identity enum and
the downloader a response-provider abstraction so emulator jobs ride the
existing pipeline.

**Tech Stack:** Rust (reqwest, serde_json, regex, url, percent-encoding,
wiremock for tests), Tauri 2 commands, Svelte 5, Node mock servers,
WebdriverIO.

**Spec:** `docs/superpowers/specs/2026-09-01-emulator-acquisition-design.md`
(binding authority). Behavior contract: `docs/porting/04-emulator-launch.md`
§12; reference code `grid_launcher/emulator/source.py`,
`grid_launcher/background/workers.py:100-320`,
`grid_launcher/emulator/autoconfig.py:13-90`,
`grid_launcher/ui/emulators.py:159-260`.

## Global Constraints

- Tokens/credentials: never in files, logs, errors, IPC, or console output.
  The forge client sends NO `Authorization` header, ever — it is a separate
  `reqwest::Client`; RomM credentials must never reach a forge host.
- GitHub requests carry exactly: `Accept: application/vnd.github+json`,
  `X-GitHub-Api-Version: 2022-11-28`, `User-Agent: grid-launcher`. Gitea and
  direct requests carry only `User-Agent: grid-launcher` (the reference
  reuses the same header fn for direct; gitea sends `{}` — port: gitea gets
  no extra headers beyond reqwest defaults + the client-level User-Agent;
  set `User-Agent: grid-launcher` at client build time so every forge
  request carries it and nothing else identifying).
- The env override `GRID_LAUNCHER_E2E_FORGE_BASE` is read ONLY under
  `#[cfg(feature = "e2e")]`. grid-core gains feature `e2e = []`; the app's
  `e2e` feature adds `"grid-core/e2e"`. Release builds cannot be redirected.
- grid-core never imports Tauri. Errors cross boundaries as Display strings.
- Error messages that exist in the reference are ported byte-for-byte
  (exact strings are pinned in the tasks below). Do not "improve" them.
- Non-command Tauri code that needs tokio must go through
  `tauri::async_runtime` (M3 rule) — relevant only if Task 7 touches setup.
- Emulators are config entries only. The SQLite registry is never written
  by any emulator install path (spec deviation 1).
- Every task ends with all suites green: `cargo test -p grid-core`,
  `cargo clippy --workspace -- -D warnings`, `cargo fmt --check`, frontend
  `npm run check` + `npm test` when touched, `bash scripts/check_secret_hygiene.sh`.
  Run from `rewrite/`. The full `rewrite/scripts/e2e.sh` gates the milestone
  (Task 8 onward).

## File Structure

```
rewrite/crates/grid-core/src/launch/source.rs      NEW  normalization + selection (pure)
rewrite/crates/grid-core/src/launch/forge.rs       NEW  forge HTTP + scrape + e2e rewrite
rewrite/crates/grid-core/src/launch/catalog.rs     NEW  listing/dedupe/sort
rewrite/crates/grid-core/src/launch/emu_install.rs NEW  selector, naming, entry creation
rewrite/crates/grid-core/src/launch/profiles.rs    MOD  RawProfile/EmulatorProfile gain raw `source`
rewrite/crates/grid-core/src/launch/mod.rs         MOD  pub mod lines
rewrite/crates/grid-core/src/launch/template.rs    MOD  Task 9 CORE_OPTION_TOKENS reuse
rewrite/crates/grid-core/src/config.rs             MOD  EmulatorEntry source_* fields
rewrite/crates/grid-core/src/library/queue.rs      MOD  JobKey identity
rewrite/crates/grid-core/src/library/download.rs   MOD  ResponseProvider generalization
rewrite/crates/grid-core/src/library/mod.rs        MOD  JobPayload + install_emulator
rewrite/crates/grid-core/src/library/registry.rs   MOD  Task 9 blank-key fix
rewrite/crates/grid-core/Cargo.toml                MOD  feature e2e, deps regex/url/wiremock
rewrite/app/src-tauri/Cargo.toml                   MOD  e2e feature adds grid-core/e2e
rewrite/app/src-tauri/src/commands.rs              MOD  two new commands, retry client Option
rewrite/app/src-tauri/src/lib.rs                   MOD  register commands
rewrite/app/src/lib/Emulators.svelte               MOD  catalog tab + manual auto-fill + Task 9
rewrite/app/src/lib/api.ts                         MOD  new invokes
rewrite/app/src/lib/downloads/…                    MOD  snapshot type gains job/source_id
rewrite/e2e/mock-romm/mock-forge.mjs               NEW  forge mock (+ shared archive helpers)
rewrite/e2e/mock-romm/archives.mjs                 NEW  zip/tar.gz builders shared w/ server.mjs
rewrite/e2e/mock-romm/mock-forge.test.mjs          NEW  node:test
rewrite/e2e/seed/emulator-catalog-seed.mjs         NEW  PS2 game seed
rewrite/e2e/specs/emulator-catalog.spec.ts         NEW  the new stage group
rewrite/e2e/wdio.conf.ts                           MOD  forge env passthrough
rewrite/scripts/e2e.sh                             MOD  stage group + forge server lifecycle
rewrite/scripts/check_secret_hygiene.sh            MOD  grid-core/e2e feature guard
docs/porting/04-emulator-launch.md                 MOD  deviations section (Task 10)
```

**Platform string:** the reference compares `sys.platform` prefixes. Pin a
crate-wide constant in source.rs:

```rust
#[cfg(target_os = "linux")]
pub const HOST_PLATFORM: &str = "linux";
#[cfg(target_os = "windows")]
pub const HOST_PLATFORM: &str = "win32";
#[cfg(target_os = "macos")]
pub const HOST_PLATFORM: &str = "darwin";
```

**Parity trap (do NOT use `launch_executable`):** several catalog `source`
blocks carry a `launch_executable` key. The Python reference never reads it
(verified by grep). Executable choice is ONLY the ported
`select_emulator_executable_path` scoring. Any implementer using
`launch_executable` has left the contract.

---

### Task 1: `launch/source.rs` — normalization + release/asset selection (pure)

**Files:**
- Create: `rewrite/crates/grid-core/src/launch/source.rs`
- Modify: `rewrite/crates/grid-core/src/launch/mod.rs` (add `pub mod source;`)
- Modify: `rewrite/crates/grid-core/Cargo.toml` (dep `serde_json` if not present)

**Interfaces (Produces):**
```rust
pub const HOST_PLATFORM: &str; // see above
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct SourceError(pub String);

/// Normalized source metadata — a JSON map, exactly like the Python dict,
/// so platform_overrides can merge over it without a typed re-parse.
pub type SourceMap = serde_json::Map<String, serde_json::Value>;

pub fn normalize_source(raw: &serde_json::Value) -> Result<SourceMap, SourceError>;
pub fn merge_platform_override(source: &mut SourceMap); // first key that is a prefix of HOST_PLATFORM
pub fn select_release<'a>(source: &SourceMap, releases: &'a serde_json::Value)
    -> Result<&'a serde_json::Map<String, serde_json::Value>, SourceError>;
pub fn select_asset<'a>(source: &SourceMap, release: &'a serde_json::Map<String, serde_json::Value>)
    -> Result<&'a serde_json::Map<String, serde_json::Value>, SourceError>;
pub fn fnmatch_case(pattern: &str, text: &str) -> bool; // *, ?, [seq], [!seq]
// string accessor used everywhere: map.get(k) → trimmed String, "" for missing/non-string
pub fn str_field(map: &SourceMap, key: &str) -> String;
```

**Normalization** ports `normalize_emulator_source_metadata`
(source.py:59-168) exactly:

- provider: key `provider`, fallback key `type`, DEFAULT `"github"` when
  both absent; trim + casefold; alias table:
  `github, github-release, github_release, githubrelease → github`;
  `gitea, gitea-release, gitea_release → gitea`;
  `direct, direct-download, direct_download, download, url → direct`;
  anything else passes through unchanged. Empty string (e.g. `provider: 5`
  → non-string → "") errors: `Source metadata is missing provider.`
- `owner` required → error
  `Source metadata is missing required field 'owner'.`
- `repo` required with fallback key `repository` → error
  `Source metadata is missing required field 'repo' (or 'repository').`
- patterns (each: string → one-element list; list → trimmed non-blank
  strings; other types → default; empty result → default):
  include from `asset_patterns` fallback `asset_globs`, default `["*"]`;
  exclude from `asset_exclude_patterns` fallback `exclude_asset_patterns`,
  default `[]`; preferred from `asset_preferred_patterns` fallback
  `preferred_asset_patterns`, default `[]`.
- `release_tag`: first non-blank string among keys **`tag`, `release_tag`,
  `version`** — THIS order (source.py:100). (The catalog listing in Task 3
  uses a DIFFERENT order; both are verbatim from their references.)
- `allow_prerelease`: truthiness of the key (bool; missing → false).
- github/gitea: keep `platform_overrides` when it is a non-empty object.
- gitea: `base_url` required (same missing-field message shape with key
  `base_url`), stored right-trimmed of `/`.
- direct: `download_url` (fallbacks `url`, `browser_download_url`),
  `page_url` (fallbacks `index_url`, `listing_url`), `download_url_regex`
  (fallbacks `url_regex`, `asset_url_regex`), `asset_name` (no fallback) —
  each first non-blank, else `""`. When download_url AND page_url are both
  empty: error
  `Direct source metadata must include either 'download_url' or 'page_url'.`
  Keep `supplemental_downloads` when it is a list — filtered to its object
  elements, stored raw. Keep `platform_overrides` as for github.
- Output map keys: `provider, owner, repo, release_tag, allow_prerelease,
  asset_patterns, asset_exclude_patterns, asset_preferred_patterns` plus
  the provider-specific ones above.

**merge_platform_override** (workers.py:167-174): iterate
`platform_overrides` entries in JSON object order; first entry whose key
string is a prefix of `HOST_PLATFORM` and whose value is an object: shallow-
merge (`source[k] = override[k]` for every key) and stop.

**select_release** ports `_extract_releases` + `_select_github_release`:

- releases value: array → its object elements; object with `assets` array →
  that one release; object with `releases` array → its object elements;
  anything else → error `GitHub release metadata must be a release object,
  a list of release objects, or a dictionary with 'releases'.`
- empty list → `No GitHub releases were provided for '<owner>/<repo>'.`
- tag from source `release_tag`; a casefolded value of `latest` is treated
  as unset. Walk in order: skip `draft: true`; skip `prerelease: true`
  unless `allow_prerelease`; when tag set, `tag_name` must match
  casefolded; first survivor wins.
- No survivor, tag set: `No matching GitHub release was found for tag
  '<tag>' in '<owner>/<repo>'. Available tags: <comma+space list of
  non-blank tag_names, or the word none>.`
- No survivor, no tag: `No usable GitHub release was found for
  '<owner>/<repo>'. All releases were drafts or prereleases.`

**select_asset** ports `_select_github_asset`:

- `assets` non-list or empty → `Selected GitHub release has no assets.
  release_tag='<tag_name trimmed>'`
- Per asset (objects only): `name` and `browser_download_url` both
  non-blank; must match ≥1 include pattern (fnmatch, name and pattern both
  casefolded) and 0 exclude patterns; `preferred_index` =
  first-matching-preferred-pattern index, else `preferred_patterns.len()`;
  `state_penalty` = 0 when trimmed+casefolded `state` (default "uploaded")
  is `""` or `"uploaded"`, else 1. Sort key
  `(include_index, preferred_index, state_penalty, name.casefold())`,
  lowest wins (stable — ties keep first).
- None matched → `No release asset matched configured patterns.
  include=<pylist>, exclude=<pylist>, available_assets=<pylist>` where
  `<pylist>` is Python list repr: `['a', 'b']` / `[]` (write a helper
  `py_list(&[String]) -> String`). `available_assets` collects every
  non-blank asset name seen, including excluded ones.

**fnmatch_case:** `*` any run, `?` one char, `[seq]` and `[!seq]` character
classes with `-` ranges (fnmatch semantics; an unclosed `[` matches a
literal `[`). Both sides are casefolded by the caller.

- [ ] **Step 1: write the failing test module** — tables, one assert per
  row, covering: alias table (all 12 aliases + pass-through `"weird"`),
  provider default github when key absent, missing owner/repo/base_url
  messages verbatim, tag chain order (`{"tag":"a","release_tag":"b"}` → a;
  `{"release_tag":"b","version":"c"}` → b), pattern fallback keys +
  defaults + string-form, direct fallback keys + both-empty error,
  supplemental list filtering (non-dict elements dropped),
  platform_overrides retention rules; merge_platform_override (linux
  override applied over normalized map; `win32` key skipped on linux; first
  matching entry only); select_release (draft skip, prerelease gate,
  casefolded tag match, first-in-order, all three error messages verbatim,
  object-with-assets and object-with-releases shapes); select_asset
  (include/exclude/preferred ordering incl. unmatched-preferred = len,
  state_penalty, casefolded name tiebreak, missing url skips, error message
  with py_list format verbatim); fnmatch_case (`*`,`?`,`[abc]`,`[!abc]`,
  `[a-z]`, real catalog patterns `pcsx2-v*-linux-appimage-x64-Qt.AppImage`
  and `Eden-Linux-*-amd64-clang-pgo.AppImage` against plausible names).
  Add one test normalizing every `source` block of the embedded
  `emulator-autoprofiles.json` (via `profiles::load_profiles` raw JSON —
  until Task 3 lands, parse the include_str! JSON directly in the test):
  every block with a recognized provider normalizes without error.
- [ ] **Step 2: run** `cargo test -p grid-core source::` — expect compile
  failure/red.
- [ ] **Step 3: implement** as specified.
- [ ] **Step 4: run** the suite + clippy + fmt; green.
- [ ] **Step 5: commit** `rewrite: source metadata normalization and release/asset selection`

### Task 2: `launch/forge.rs` — forge HTTP, scrape, e2e rewrite

**Files:**
- Create: `rewrite/crates/grid-core/src/launch/forge.rs`
- Modify: `launch/mod.rs` (`pub mod forge;`)
- Modify: `rewrite/crates/grid-core/Cargo.toml` — add deps `regex`, `url`;
  dev-dep `wiremock`; add `[features] e2e = []`.
- Modify: `rewrite/app/src-tauri/Cargo.toml` — `e2e` feature list gains
  `"grid-core/e2e"`.
- Modify: `rewrite/scripts/check_secret_hygiene.sh` — see below.

**Interfaces (Produces):**
```rust
pub struct ForgeClient { http: reqwest::Client } // User-Agent grid-launcher, timeout 60s, NO auth
pub struct ResolvedDownload {
    pub provider: String, pub owner: String, pub repo: String,
    pub release_tag: String, pub asset_name: String,
    pub download_url: String, pub size: i64, // github asset size, else 0
}
impl ForgeClient {
    pub fn new() -> Result<Self, SourceError>;
    /// normalize → merge override → platforms gate (direct) → fetch/scrape → select.
    /// `raw` is the profile's raw source JSON; `profile_name` feeds the
    /// direct platforms-gate message.
    pub async fn resolve(&self, raw: &serde_json::Value, profile_name: &str)
        -> Result<ResolvedDownload, SourceError>;
    /// GET `url` (after e2e rewrite) with forge headers, streaming response.
    pub async fn get(&self, url: &str, github_headers: bool)
        -> Result<reqwest::Response, SourceError>;
}
fn effective_url(url: &str) -> String; // identity unless e2e env set
```

**resolve, per provider** (workers.py:165-303):

- normalize (Task 1) then `merge_platform_override`.
- direct: platforms gate reads the RAW metadata (`platforms` list on the
  raw source; prefix match against `HOST_PLATFORM`); on failure the message
  is `"<name> has no auto-install source available for this platform.
  <manual_install_hint>"` — `name` = raw `name` key if present else the
  Python literal `This emulator` (our catalog passes `profile_name` — use
  raw source `name` first, then `profile_name`, then `This emulator`), hint
  from raw `manual_install_hint`, whole string trimmed. Then: when
  `download_url` empty and `page_url` set, fetch the page as text
  (utf-8 lossy) and scrape:
  - when `download_url_regex` set: compile case-insensitive
    (`regex::RegexBuilder`, invalid pattern → SourceError with the regex
    error text). Find all `href\s*=\s*["']([^"']+)["']` (case-insensitive)
    captures in page order; for each trimmed non-empty href, compute
    `resolved = Url::parse(page_url)?.join(href)?` (urljoin); take the
    first where the regex matches the raw href OR the resolved string →
    that `resolved` is the download_url. Otherwise search the whole page:
    first match → first non-empty capture group, else the whole match,
    trimmed, urljoined with page_url.
  - still empty → `Direct source metadata did not resolve a download URL
    from the configured page. page_url='<page_url>'`
  - download_url empty and no page → `Direct source metadata did not
    include a download URL.`
  - asset_name default: basename of the URL path (`Url::parse` →
    `path_segments().last()`).
  - result: release_tag = source release_tag or `latest`, size 0.
- github: api_base `https://api.github.com/repos/{owner}/{repo}`,
  github_headers true. gitea: `{base_url}/api/v1/repos/{owner}/{repo}`,
  github_headers false. Endpoint by tag: explicit non-latest tag →
  `/releases/tags/{tag}` with the tag percent-encoded (everything outside
  `ALPHA DIGIT - . _ ~`); literal `latest` (casefold) → `/releases/latest`;
  unset → `/releases`. Parse JSON (non-object/array top level →
  SourceError `Source release API returned an unsupported payload shape.`),
  then Task 1 `select_release` + `select_asset`; result fields from the
  asset (`size` from asset `size`, 0 fallback).
- unrecognized provider → `Unsupported source provider '<p>'. Supported
  providers: github, gitea, direct.`
- HTTP-level failures (connect, non-2xx via `error_for_status`, body read)
  → SourceError with a message that contains the URL's HOST but never any
  header content.

**e2e rewrite — request time, not metadata time** (critical: the catalog's
`download_url_regex` values match absolute production URLs, so scraped
hrefs/pages must keep original URLs; only outgoing requests are diverted):

```rust
#[cfg(feature = "e2e")]
fn effective_url(url: &str) -> String {
    match std::env::var("GRID_LAUNCHER_E2E_FORGE_BASE") {
        Ok(base) if !base.trim().is_empty() => {
            let parsed = url::Url::parse(url); /* on failure return url unchanged */
            // <base>/<host>/<path>?<query>
            ...
        }
        _ => url.to_string(),
    }
}
#[cfg(not(feature = "e2e"))]
#[inline]
fn effective_url(url: &str) -> String { url.to_string() }
```
Every `ForgeClient` request goes through `effective_url` in exactly one
place (`get`). Page scraping still urljoins against the ORIGINAL page_url.

**Hygiene guard:** in `check_secret_hygiene.sh`, next to the existing wdio
cargo-tree checks, add: the default-feature graph must not enable the
grid-core `e2e` feature —
`cargo tree -p app -e features | grep -q "grid-core feature \"e2e\"" && fail`
(and the e2e-feature graph MUST show it). Also assert
`grep -rl GRID_LAUNCHER_E2E_FORGE_BASE crates/ app/src-tauri/src/` matches
only `crates/grid-core/src/launch/forge.rs`.

- [ ] **Step 1: failing tests** (`#[tokio::test]` + wiremock):
  github happy path asserting the three GitHub headers present and
  **`Authorization` absent** (wiremock header-absent match), tag endpoint
  choice (3 cases; percent-encoding of a tag with `/`), gitea base_url
  endpoint with github headers absent, direct page scrape: href-precedence
  (href matches raw), href matching only after urljoin (relative href,
  regex on absolute), whole-page capture-group fallback, whole-match
  fallback, scrape-failure message verbatim, platforms-gate message
  verbatim (hint appended, trimmed), unsupported-provider message,
  non-JSON payload message. e2e rewrite unit tests (feature-gated
  `#[cfg(feature = "e2e")]` test module; run once with
  `cargo test -p grid-core --features e2e`): host+path+query mapping,
  empty env passthrough. Serialize env-var tests with a mutex or use
  temp-env style set/unset within one test.
- [ ] **Step 2: red run.**
- [ ] **Step 3: implement.**
- [ ] **Step 4: green:** `cargo test -p grid-core` AND
  `cargo test -p grid-core --features e2e`; clippy/fmt; hygiene script
  passes (run it).
- [ ] **Step 5: commit** `rewrite: forge client with scrape and e2e-gated redirect`

### Task 3: raw `source` on profiles + `launch/catalog.rs` listing

**Files:**
- Modify: `rewrite/crates/grid-core/src/launch/profiles.rs`
- Create: `rewrite/crates/grid-core/src/launch/catalog.rs`
- Modify: `launch/mod.rs` (`pub mod catalog;`)

**Interfaces:**
- `RawProfile` and `EmulatorProfile` gain
  `pub source: Option<serde_json::Value>` — on `EmulatorProfile` annotate
  `#[serde(skip_serializing)]` so existing IPC payloads (auto-fill list)
  are unchanged. `normalize_one` copies it through untouched.
- Produces:
```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CatalogEntry {
    pub name: String, pub source_id: String, // "{owner}/{repo}"
    pub provider: String, pub owner: String, pub repo: String,
    pub tag: String, pub installed: bool,
}
pub fn catalog_entries(profiles: &[EmulatorProfile]) -> Vec<CatalogEntry>; // installed=false
pub fn mark_installed(entries: &mut [CatalogEntry], config: &Config);
pub fn find_profile<'a>(profiles: &'a [EmulatorProfile], source_id: &str)
    -> Option<&'a EmulatorProfile>; // by "{owner}/{repo}", casefolded
```

**catalog_entries** ports `source_download_emulator_entries`
(ui/emulators.py:168-231) with spec deviation 2:

- skip `is_compat_tool` profiles (deviation 2 — the reference showed them
  in a separate dialog).
- keep profiles whose `source` is an object; platforms gate: raw
  `source["platforms"]` when a non-empty list — some entry must be a
  prefix of `HOST_PLATFORM` (this is what hides the `win32`-only rows).
- provider: raw `provider` (fallback `type`), trim+casefold; map ONLY the
  github family (`github-release`, `github_release`, `githubrelease` →
  `github`, ui/emulators.py:159); everything else passes through
  unchanged (gitea stays `gitea`; an unknown string stays itself — it
  errors at resolve time, not listing time).
- require `owner` and `repo` (fallback `repository`) non-blank; skip row
  otherwise.
- tag: first non-blank of **`release_tag`, `tag`, `version`**, else
  `latest` — THIS order here (ui/emulators.py:205; differs from Task 1).
- `source_id = format!("{owner}/{repo}")`.
- dedupe on casefolded `(name, provider, owner, repo)` keeping the first;
  sort by casefolded `(name, source_id)`.

**mark_installed:** installed when the entry `name` casefold-equals any
config emulator's `name`, OR `source_id` casefold-equals any config
emulator's `source_id` (Task 4 field; compare via the raw string —
`config.emulators` iteration, blank source_id never matches).

**find_profile:** first profile whose source normalizes far enough to have
`owner/repo` matching `source_id` casefolded (use the same raw-field reads
as catalog_entries; skip compat tools).

- [ ] **Step 1: failing tests:** with the REAL embedded catalog on linux:
  entries exclude `GE-Proton`, `Proton-CachyOS` (compat), `ShadPS4 Qt
  Launcher`, `Xenia Canary (Xbox 360)` (win32 platforms gate), include
  `RetroArch (Multi-System)` (direct, source_id
  `libretro/retroarch-nightly`), `Eden (Nintendo Switch)` (provider
  `gitea`), `PCSX2 (Playstation 2)` (provider `github`, from
  `github-release`); sorted by casefolded name; every source_id
  `owner/repo`. Synthetic tables: dedupe casefolded keep-first, tag chain
  order `release_tag` first, unknown provider passes through, missing
  repo skips. mark_installed: by name casefold, by source_id casefold,
  blank source_id never matches. find_profile hit/miss.
- [ ] **Step 2: red run.**
- [ ] **Step 3: implement** (profiles.rs field + catalog.rs).
- [ ] **Step 4: green + clippy/fmt.** Also `npm run check` in
  `rewrite/app` (EmulatorProfile serialization unchanged — should pass
  untouched).
- [ ] **Step 5: commit** `rewrite: emulator catalog listing from autoprofile sources`

### Task 4: `launch/emu_install.rs` + `EmulatorEntry` source fields

**Files:**
- Create: `rewrite/crates/grid-core/src/launch/emu_install.rs`
- Modify: `launch/mod.rs`; `rewrite/crates/grid-core/src/config.rs`

**config.rs:** `EmulatorEntry` gains
```rust
#[serde(default, skip_serializing_if = "String::is_empty")] pub source_id: String,
#[serde(default, skip_serializing_if = "String::is_empty")] pub source_provider: String,
#[serde(default, skip_serializing_if = "String::is_empty")] pub source_owner: String,
#[serde(default, skip_serializing_if = "String::is_empty")] pub source_repo: String,
#[serde(default, skip_serializing_if = "String::is_empty")] pub source_release_tag: String,
```
(Existing struct-literal call sites need `..Default::default()` or the
fields added — derive `Default` for `EmulatorEntry` if it lacks one.)

**Interfaces (Produces):**
```rust
pub fn emulator_install_dir(library: &Path, archive_stem: &str) -> PathBuf;
// <library>/Emulators/<sanitize_component(stem, "emulator")> — reuse library::paths::sanitize_component
pub fn archive_file_name(profile_name: &str, tag: &str, asset_name: &str) -> String;
pub fn supplemental_file_name(primary: &Path, index: usize, asset_name: &str) -> String;
pub fn select_executable(title: &str, install_dir: &Path, archive: &Path) -> Option<PathBuf>;
pub fn launchable_emulator_file(path: &Path) -> bool; // .exe .bat .cmd .ps1 .sh .appimage, case-insensitive
```

**archive_file_name** (emulator_ui_mixin.py:1187 + workers.py:153-163):
base `"{sanitize(profile_name)}-{sanitize(tag)}.zip"` (same
sanitize_component, fallback "emulator"); then asset-suffix rewrite: empty
asset → base; asset ends `.appimage` casefold → the asset_name itself;
asset has no `.suffix` → base; asset suffix casefold == base suffix
(`.zip`) → base; else base with suffix replaced by the asset's.

**supplemental_file_name** (workers.py:147-151): asset ends `.appimage`
casefold → `"{primary_stem}-supplemental-{index}-{asset_name}"`; else
suffix = asset's suffix if non-empty, else primary's suffix if non-empty,
else `.zip` → `"{primary_stem}-supplemental-{index}{suffix}"`. Index is
1-based.

**select_executable** ports autoconfig.py:19-87 with
`extracted_dir = install_dir`, `extracted_path` absent (our pipeline never
sets it — the archive-fallback branch covers non-extracted AppImages):
title trimmed+casefolded; title_tokens = `[a-z0-9]+`-complement split,
len > 2 kept; preferred names: `eden.exe` when title contains
`nintendo switch` or `switch`; `azahar.exe` when `nintendo 3ds` or `3ds`.
When install_dir exists: recursive walk (`walkdir` NOT needed — write a
small recursive fn or use `fs::read_dir` recursion; follow the Python
`rglob` = follows directory symlinks — keep it simple: recurse, skip
unreadable), files passing `launchable_emulator_file`; score
`(preferred_name 0/1, -token_hits as i64, exe_pref 0/1 (.exe casefold),
path_component_count, full_path.to_string_lossy().to_lowercase())`, min
wins. Else/no candidates: archive exists, is file, launchable → archive.
Else None.

- [ ] **Step 1: failing tests:** naming tables (AppImage whole-name
  replace, `.tar.gz` asset → base suffix swap to `.gz`? — NO: Python
  `Path.suffix` of `x.tar.gz` is `.gz`; pin the row
  `("Redream (Sega Dreamcast)", "nightly", "redream.x86_64-linux-v1.5.0-1000-gabc.tar.gz")
  → "Redream (Sega Dreamcast)-nightly.gz"`, matching the reference's
  single-suffix behavior; extraction still sniffs gzip so this is
  correct-by-parity), supplemental names incl. appimage form and both
  suffix fallbacks, install dir sanitization (`Emu: <bad>*chars` →
  underscores). Selector table with tempdir trees: token-hit scoring
  (title `PCSX2 (Playstation 2)` picks `pcsx2-qt` over `updater.sh` —
  note token `pcsx2` needs len>2 ✓), eden.exe preferred for a Switch
  title even against higher token hits, `.exe` preference, shallowest
  wins, casefolded path tiebreak, AppImage-only dir, archive fallback
  (dir missing), None when nothing launchable. Config round-trip: entry
  without source fields serializes byte-identically to before (no new
  keys), entry with fields round-trips.
- [ ] **Step 2: red.** — **Step 3: implement.** — **Step 4: green +
  clippy/fmt.**
- [ ] **Step 5: commit** `rewrite: emulator install naming, executable selection, entry source fields`

### Task 5: queue `JobKey` + downloader `ResponseProvider`

**Files:**
- Modify: `rewrite/crates/grid-core/src/library/queue.rs`
- Modify: `rewrite/crates/grid-core/src/library/download.rs`
- Modify: `rewrite/crates/grid-core/src/library/mod.rs` (mechanical caller
  updates ONLY — the emulator job kind itself is Task 6)

**queue.rs:**
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobKey { Rom(i64), Emulator(String) } // Emulator carries source_id
```
- `DownloadEntry` gains `pub job: &'static str` — serialize as `"game"` /
  `"emulator"` — and `pub source_id: String` (empty for games); `rom_id`
  stays (0 for emulator entries).
- `admit(&mut self, key: JobKey, title: &str, platform: &str) -> Admission`;
  dedupe via stored key comparison (`has_pending(&JobKey)`). Store the key
  on the entry (private field or reconstruct from job+ids — store
  `key: JobKey` privately, derive the public fields from it at insert).
- `retryable(&self, id) -> Option<JobKey>`.
- Everything else unchanged.

**download.rs:** replace the `&RommClient` parameter:
```rust
pub trait ResponseProvider: Sync {
    fn get(&self, target: &FileTarget)
        -> impl std::future::Future<Output = Result<reqwest::Response, LibraryError>> + Send;
}
pub struct RommProvider<'a>(pub &'a RommClient);   // url_path relative + query, as today
pub async fn download_targets<P: ResponseProvider>(provider: &P, targets, cancel, on_progress) -> …
```
`RommProvider::get` = today's `client.get_response(&t.url_path, &query)`.
(Task 6 adds the forge provider.) `FileTarget` unchanged — for forge use,
`url_path` holds an absolute URL and `query` stays empty.

**mod.rs mechanical:** `admit` call sites pass `JobKey::Rom(job.rom_id)`;
`retry` matches `Some(JobKey::Rom(rom_id))` (Emulator arm
`unreachable-for-now` → plain no-op returning Ok, replaced in Task 6);
`spawn_download` wraps the client in `RommProvider`.

- [ ] **Step 1: failing tests:** queue — emulator admit/dedupe (same
  source_id Duplicate, different source_id queues, Rom(5) vs
  Emulator("x/y") independent), snapshot exposes `job` + `source_id`,
  retryable returns the right key for both kinds; existing tests updated
  to the new admit signature (batch edit, keep every assertion).
  download.rs — a test-local provider (wiremock-backed) proving
  download_targets is provider-generic; existing behavior tests keep
  passing under RommProvider.
- [ ] **Step 2: red.** — **Step 3: implement.** — **Step 4:**
  `cargo test -p grid-core` + workspace clippy/fmt green; frontend
  untouched this task (snapshot additions are additive JSON).
- [ ] **Step 5: commit** `rewrite: queue job identity and provider-generic downloader`

### Task 6: `InstallService::install_emulator` — the emulator job kind

**Files:**
- Modify: `rewrite/crates/grid-core/src/library/mod.rs`
- Modify: `rewrite/crates/grid-core/src/launch/forge.rs` (ForgeProvider impl only)

**Interfaces (Produces):**
```rust
pub async fn install_emulator(self: &Arc<Self>, source_id: String) -> Result<(), LibraryError>;
// retry() signature becomes:
pub async fn retry(self: &Arc<Self>, client: Option<Arc<RommClient>>, entry_id: u64) -> Result<(), LibraryError>;
```

**Shape:** `pending_jobs`/dispatch carry `enum JobPayload { Game(InstallJob),
Emulator(EmulatorJob) }`.

```rust
struct EmulatorJob {
    source_id: String,
    profile_name: String,
    profile_args: String,
    raw_source: serde_json::Value,
    library: PathBuf,
    forge: Arc<forge::ForgeClient>,
    /// filled by the download task, consumed by finalize
    resolved: Option<ResolvedPaths>,
}
struct ResolvedPaths {
    install_dir: PathBuf, archive: PathBuf,
    supplementals: Vec<PathBuf>,
    resolved: forge::ResolvedDownload,
}
```

**install_emulator (pre-admission, no network):** library_root()?;
`catalog::find_profile` by source_id (miss →
`LibraryError::Registry("unknown emulator: <source_id>".into())` — an
internal-consistency error, not user-typical); require `profile.source`;
admit `JobKey::Emulator(source_id)` with title = profile name, platform =
`"Emulator"`. Resolution happens INSIDE the download task so failures land
on the drawer row exactly like game failures (matches the reference's
worker-side resolution).

**Download task (Emulator arm of spawn_download):**
1. `forge.resolve(&raw_source, &profile_name)` → on error
   `fail via download_finished(Err(Extract(msg)))`.
2. Check the cancel flag (a cancel during resolution must not download).
3. Resolve supplementals: raw `supplemental_downloads` list (from the
   MERGED normalized source — re-run normalize+merge here or have
   `forge.resolve` also return the merged SourceMap; pin: `resolve`
   returns `(ResolvedDownload, SourceMap)` so supplementals come from the
   merged map, matching workers.py:128 which reads
   `self.source_metadata` = the profile's raw source — NOTE the reference
   reads supplementals from the RAW source, not the merged one, and each
   supplemental spec carries its own platform_overrides which
   `_resolve_source_download` merges per-spec. Port THAT: supplementals
   from raw source, resolved individually via `forge.resolve(spec, name)`).
4. Compute paths: primary archive name via
   `archive_file_name(profile_name, resolved.release_tag, resolved.asset_name)`;
   `install_dir = emulator_install_dir(&library, primary_stem)` where
   primary_stem = file_stem of the archive name; archive dest =
   `install_dir/<archive name>`; supplemental dests via
   `supplemental_file_name(&archive, i, &supp.asset_name)` (siblings).
5. Targets: primary `FileTarget { url_path: resolved.download_url,
   query: vec![], dest: archive, expected_size: resolved.size }` +
   one per supplemental (their sizes). Download via `ForgeProvider`
   (`forge.get(&t.url_path, github_headers)` — github_headers true iff
   that download's provider normalized to github; carry a per-target flag
   or resolve headers per URL host; pin: carry
   `github_headers: bool` alongside each URL in the emulator target list
   by wrapping FileTarget in the provider).
6. Store `ResolvedPaths` on the job, hand to `finish_download` as today.
   `already_installed` short-circuit does NOT apply to emulator jobs
   (registry is games-only) — always finalize on success.

**Finalize (Emulator arm, blocking half):**
1. `should_extract("Emulator", &archive)`? then `extract_archive(archive,
   &install_dir_extract_dest, progress)` — extraction dest IS the install
   dir? No: the game engine's `extraction_dir(archive)` derives
   `<parent>/<stem>`; since archive already sits inside install_dir, that
   would nest. Pin: extract to `install_dir` itself is wrong too
   (wipe_and_recreate would delete the downloaded supplementals sitting
   there). Extract to `extraction_dir(&archive)` =
   `<install_dir>/<archive stem>`… also wrong vs the reference, which
   extracts into install_path directly. RESOLUTION (binding): download the
   archive and supplementals into a sibling staging location — archive
   dest = `<install_dir>/<archive name>` as above, extraction dest =
   temp dir `<install_dir>/.extract-tmp`; after extraction move/merge
   `.extract-tmp`'s contents into `install_dir` (rename each top-level
   entry; cross-device impossible, same dir), remove `.extract-tmp`.
   Non-extractable archive (AppImage): the file in place IS the install.
2. Merge each supplemental that exists: if extractable, extract to
   `<install_dir>/.supp-tmp-<n>` then copy-over into install_dir
   (doc 03 merge semantics: create dirs, overwrite files, keep unrelated
   files), remove temp; a non-extractable supplemental (AppImage) is left
   in place as a sibling file. ANY supplemental extract/merge error fails
   finalize (spec deviation 4 — visible).
3. `select_executable(&profile_name, &install_dir, &archive)`; None →
   `Err(LibraryError::Extract("No launchable emulator executable was found after install".into()))`.
4. `make_executable(&exe)` (existing helper).
5. Config entry (load-modify-save on `config_path`): name = profile_name,
   path = exe string, args = profile_args, source_id, source_provider =
   resolved.provider, source_owner/repo, source_release_tag =
   resolved.release_tag. An existing entry with the same name (casefold?
   NO — exact match, mirroring M3 save_emulator's replace rule) is
   replaced AT ITS INDEX; else push.
6. Delete the primary archive with `delete_with_retry` ONLY when it was
   extracted (AppImage primaries are the install — keep); delete merged
   supplemental archives likewise; failures → warning string (entry still
   Completed), matching game behavior. Any earlier failure keeps all
   archives on disk so retry skips finished downloads.

**retry:** `Some(JobKey::Rom(id))` needs `client` — `None` client →
`Err(LibraryError::Registry("not connected".into()))`;
`Some(JobKey::Emulator(source_id))` → dismiss + `install_emulator`.

- [ ] **Step 1: failing integration tests** (tempdir + wiremock forge,
  fake config file; no RomM server needed): (a) github zip end-to-end —
  entry Completed, install_dir holds extracted tree, exe selected +
  0o755, config entry written with args + all five source fields;
  (b) same-source_id dedupe while active → silent ignore; (c) resolution
  failure → row Failed with the SourceError text, no files;
  (d) supplemental merge — primary zip + supplemental zip (overlapping
  and non-overlapping paths) → merged, overwritten file has supplemental
  content, supplemental archive deleted; (e) supplemental download 404 →
  Failed, primary archive KEPT; (f) AppImage primary (no extraction) →
  file kept, chmod, entry path = the AppImage; (g) replace-in-place: a
  pre-existing config entry with the same name at index 0 of 2 stays at
  index 0 with the new path; (h) retry of a failed emulator row
  re-installs (wiremock now 200) with `retry(None, id)`; (i) NO
  Authorization header on any forge request (wiremock assertion).
- [ ] **Step 2: red.** — **Step 3: implement.** — **Step 4:**
  full `cargo test -p grid-core` (+ `--features e2e`), clippy, fmt,
  hygiene script green.
- [ ] **Step 5: commit** `rewrite: emulator install job through the InstallService queue`

### Task 7: IPC commands + Emulators catalog UI + manual auto-fill

**Files:**
- Modify: `rewrite/app/src-tauri/src/commands.rs`, `src/lib.rs`
- Modify: `rewrite/app/src/lib/api.ts`, `Emulators.svelte`,
  `downloads/` store types (snapshot rows now carry `job`, `source_id`)

**Commands:**
```rust
#[tauri::command] pub fn list_emulator_catalog(state: …) -> Result<Vec<CatalogEntry>, String>
// catalog_entries(load_profiles()) + mark_installed(&mut, &Config::load(config_path)?)
#[tauri::command] pub async fn install_emulator(state: …, source_id: String) -> Result<(), String>
// state.install?.install_emulator(source_id) — errors Display-mapped like game install
// retry command: pass Option<Arc<RommClient>> (None when no session) — emulator retries work offline
```

**Emulators.svelte — Add section becomes two tabs** (Install default,
Manual second; spec user journey):

- Install tab: search input (`data-testid="emu-catalog-search"`,
  client-side AND-of-whitespace-tokens over casefolded name + source_id —
  the reference's filter semantics), rows sorted as delivered; each row
  shows name, provider, tag; Install button
  (`data-testid="emu-catalog-install-{source_id with / → -}"`), on click
  `installEmulator(source_id)` then refresh the catalog after the
  downloads store reports that source_id Completed (subscribe to the
  existing snapshot store; a simple `$effect` re-fetch on any entry with
  `job === 'emulator'` reaching a terminal status is enough). Installed
  rows: button disabled, label `Installed`
  (`data-testid="emu-catalog-installed-{…}"`). Pre-admission errors
  surface in the panel's existing error line; post-admission errors are on
  the drawer row (no new UI).
- Manual tab: the existing form, plus auto-fill from NAME: when the name
  input loses focus or changes AND path AND args are both empty: match
  trimmed casefolded name against visible profile names — exact match,
  else unique substring (name ⊂ profile name); on a hit fill args and show
  the matched profile name as a hint line
  (`data-testid="emu-autofill-hint"`). Never overwrite a non-empty args.
  (The existing path-based auto-fill stays untouched.)
- Downloads drawer rows: no layout change — emulator entries arrive with
  `platform: "Emulator"` and render like any row; update the TS types for
  the two new snapshot fields.
- Defaults selects and the rest of the panel: untouched here (Task 9
  touches the "(none)" handling).

- [ ] **Step 1:** extend api.ts + types; write/adjust component tests where
  the suite has them (vitest: auto-fill matcher as an exported pure helper
  `matchProfileByName(name, profiles)` with tests: exact beats substring,
  ambiguous substring → null, empty → null; keep DOM work untested beyond
  svelte-check).
- [ ] **Step 2:** implement commands + register; implement UI.
- [ ] **Step 3:** `cargo build -p app --features e2e` and default build
  both compile; `npm run check` + `npm test` green; clippy/fmt/hygiene.
- [ ] **Step 4: commit** `rewrite: catalog install UI, manual-add name auto-fill, emulator IPC`

### Task 8: mock forge + `emulator-catalog` E2E group

**Files:**
- Create: `rewrite/e2e/mock-romm/archives.mjs` (zip builder extracted from
  server.mjs + a minimal tar.gz builder: 512-byte ustar headers + zlib
  gzip; mode 0755 on members), `mock-forge.mjs`, `mock-forge.test.mjs`
- Create: `rewrite/e2e/seed/emulator-catalog-seed.mjs`,
  `rewrite/e2e/specs/emulator-catalog.spec.ts`
- Modify: `rewrite/e2e/mock-romm/server.mjs` (import archives.mjs; no
  behavior change — server.test.mjs must stay green),
  `rewrite/e2e/wdio.conf.ts`, `rewrite/scripts/e2e.sh`

**mock-forge.mjs** (node:http, zero deps, `--port 0` prints
`MOCK_FORGE_URL=<url>` on stdout like server.mjs prints its URL — mirror
that convention exactly; request log appended to
`last-run-forge-requests.log`): routes on `/{original-host}/{path}`:

- `/api.github.com/repos/PCSX2/pcsx2/releases/latest` → JSON release
  `tag_name: "v9.9-e2e"`, one asset
  `name: "pcsx2-v9.9-e2e-linux-appimage-x64-Qt.AppImage"`,
  `browser_download_url:
  "https://github.com/PCSX2/pcsx2/releases/download/v9.9-e2e/pcsx2-v9.9-e2e-linux-appimage-x64-Qt.AppImage"`,
  `size` = the stub's byte length, `state: "uploaded"`. (Matches the real
  catalog's linux `asset_patterns` glob.)
- `/github.com/PCSX2/pcsx2/releases/download/v9.9-e2e/…AppImage` → the
  stub "AppImage": a `#!/bin/sh` script that appends `"$@"` to
  `$GRID_E2E_ARGV_FILE` (same contract as the existing launch stubs —
  reuse their argv-file convention verbatim) then sleeps briefly.
- `/redream.io/download` → HTML page containing
  `<a href="https://redream.io/download/redream.x86_64-linux-v1.5.0-1000-gabcdef0.tar.gz">`
  (absolute href — matches the catalog regex as-is) plus decoy hrefs.
- `/redream.io/download/redream.x86_64-linux-v1.5.0-1000-gabcdef0.tar.gz`
  → tar.gz containing `redream` (mode 0755, shell-script bytes).
- Any `Authorization` header on ANY request → respond 500 and log
  `AUTH-HEADER-SEEN` (the spec's no-credential rule, enforced at runtime);
  the wdio spec greps the request log for its absence.
- Unknown path → 404.

**mock-forge.test.mjs** (node:test, explicit file paths in the runner —
this Node's `node --test <dir>` is broken): release JSON shape, AppImage
bytes round-trip, page contains the regex-matching href, tar.gz extracts
(inflate + parse header enough to assert the member name/mode), 500 on
Authorization.

**e2e.sh:** append stage group
`"emulator-catalog:specs/emulator-catalog.spec.ts"`; `seed_script_for_group`
gains `emulator-catalog) → seed/emulator-catalog-seed.mjs`; the run loop
starts mock-forge for that group only (same pattern as the romm mock:
background start, scrape `MOCK_FORGE_URL=`, kill with the existing
run-dir-marker cleanup — it inherits the run-dir env so the marker
mechanism already covers it), exports `E2E_FORGE_URL`. Failure dump also
tails the forge log for that group.

**wdio.conf.ts:** `appEnv` gains
`...(process.env.E2E_FORGE_URL ? { GRID_LAUNCHER_E2E_FORGE_BASE: process.env.E2E_FORGE_URL } : {})`
and passes through `GRID_E2E_ARGV_FILE` the same way the launch group does.

**emulator-catalog-seed.mjs:** copy the launch-seed pattern: registry row +
rom file for one installed PS2 game (platform name `Sony PlayStation 2` —
`platform_matches_keywords("Sony PlayStation 2", ["Playstation 2"])` is
true per the keyword-matcher table, so PCSX2 appears in that platform's
default-emulator select), plus whatever config/session seeding launch-seed
does. Mock-romm args for the group mirror the launch group's.

**emulator-catalog.spec.ts** (one wdio run, ~2 scenarios in order):
1. Connect (existing helper flow) → Emulators → Add → catalog tab visible;
   search narrows rows; `PCSX2 (Playstation 2)` row present.
2. Install PCSX2 → downloads drawer row appears (`job` emulator rows render)
   → wait Completed → emulator list shows `PCSX2 (Playstation 2)` with the
   profile's args in the row/edit form → catalog now shows Installed +
   disabled button.
3. Set it as the `Sony PlayStation 2` default → open the seeded game →
   Play → argv file exists and contains the rom path (launch.spec's
   assertion pattern) → Stop.
4. Install `Redream (Sega Dreamcast)` (direct scrape) → Completed →
   emulator row exists, its path ends with `redream`.
5. Assert the forge request log contains no `AUTH-HEADER-SEEN` and the app
   made zero requests to the mock-romm content endpoints for these
   installs (forge and romm are provably separate clients).

- [ ] **Step 1:** archives.mjs + mock-forge + tests; run the node tests
  (explicit paths) — green; server.test.mjs still green.
- [ ] **Step 2:** seed + spec + e2e.sh + wdio wiring.
- [ ] **Step 3:** `rewrite/scripts/e2e.sh emulator-catalog` green locally
  (fix the app/spec until it is — condition-based waits only, install
  timeout ≤ 15 s for the tiny stubs).
- [ ] **Step 4:** full `rewrite/scripts/e2e.sh` (all groups) green.
- [ ] **Step 5: commit** `rewrite: mock forge and emulator-catalog E2E group`

### Task 9: carried cleanup (M2/M3 parked findings)

**Files:**
- Modify: `rewrite/crates/grid-core/src/library/registry.rs` + the comment
  at `library/mod.rs:333-341`
- Modify: `rewrite/crates/grid-core/src/launch/template.rs`
- Modify: `rewrite/app/src/lib/Emulators.svelte`

One commit, three fixes:

1. **Registry blank-key gap:** `Registry::find(rom_id, title, platform)`
   must not run the (title, platform) fallback when `title.trim()` is
   empty — a blank-title query today can match a blank-titled,
   null-rom_id row. Regression test: insert a row with title `""`,
   rom_id NULL; `find(Some(7), "", "")` returns None. Rewrite the
   mod.rs uninstall comment to describe the now-true behavior (it
   currently both claims and denies the blank-key hazard).
2. **CORE_OPTION_TOKENS reuse:** `apply_placeholders` in template.rs
   duplicates the token list that `CORE_OPTION_TOKENS` defines — make the
   function iterate the constant. Existing tests pin behavior; add none
   unless a gap shows.
3. **Defaults select "(none)":** the per-platform default-emulator select
   uses the literal label as its sentinel value, which collides with an
   emulator actually named `(none)`, and matching is not verbatim. Use
   `""` as the none value, match saved defaults against emulator names
   verbatim (exact, case-sensitive). svelte-check + a vitest for any
   extracted pure helper; if none extracts cheaply, svelte-check only and
   say so in the report.

- [ ] Steps: failing test for (1) → fixes → full suites green → commit
  `rewrite: carried cleanup — registry blank-key guard, token reuse, defaults sentinel`

### Task 10: hygiene run, porting-doc deviations, README

**Files:**
- Modify: `docs/porting/04-emulator-launch.md` — append section
  `## Rust port deviations (milestone 4)`: the spec's five deviations
  verbatim (config-entries-not-registry-rows; compat tools excluded;
  version checks deferred with `source_*` fields recorded; supplemental
  failures fail visibly; no firmware step) plus: `launch_executable`
  intentionally unread (parity with the reference), staged extraction via
  `.extract-tmp` instead of extract-in-place, visible
  "No launchable emulator executable was found after install" failure.
- Modify: `rewrite/README.md` — E2E section: new stage group listed;
  residual manual checklist unchanged.
- Verify `check_secret_hygiene.sh` covers the Task 2 additions and passes
  in both feature states.

- [ ] Docs written → hygiene + full local test suites + `e2e.sh` all
  green → commit `rewrite: milestone 4 docs and hygiene guard`

---

## Self-review notes (already applied)

- Spec coverage: every spec Scope bullet maps to Tasks 1-8; carried
  cleanup → Task 9; deviations recording → Task 10; exit gate = Task 8
  step 4 + CI on the pushed branch.
- The two tag-fallback orders (normalize vs listing) are deliberately
  different — both pinned with their reference lines.
- Supplemental E2E coverage moved to Rust integration tests (Task 6d/6e)
  because a `.7z` cannot be generated dependency-free in Node; the E2E
  direct-provider scenario uses Redream (tar.gz). The spec's testing
  section asks for "a second scenario installs the direct-provider
  (scrape) stub" — satisfied.
- The e2e forge redirect is request-time (Task 2), which the spec's
  metadata-time wording did not anticipate; recorded here as the binding
  interpretation because metadata-time rewriting breaks the catalog's
  absolute-URL regexes.

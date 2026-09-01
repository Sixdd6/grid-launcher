# Core Install Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Rust app downloads games from RomM, extracts them, records installs in SQLite, shows live progress in a downloads UI, and uninstalls — milestone 2 of the rewrite.

**Architecture:** New `library` module in `grid-core` (queue state machine, streamed download, pure-Rust extraction, SQLite registry, uninstall, path rules), an `InstallService` glue type owned by the Tauri `AppState`, new Tauri commands plus a `downloads-changed` event, and Svelte UI (downloads drawer, details overlay, installed badges).

**Tech Stack:** Existing workspace (Rust, Tauri 2, Svelte 5, tokio, reqwest 0.13, wiremock, vitest) plus new crates: `rusqlite` (bundled), `zip`, `tar`, `flate2`, `liblzma`, `bzip2`, `sevenz-rust2`. Add each with `cargo add` so versions resolve current; do not hand-pin guesses.

**Spec:** `docs/superpowers/specs/2026-08-31-install-pipeline-core-design.md` — the binding authority. Doc 03 (`docs/porting/03-library-install.md`) is the behavior contract it cites.

**Branch:** `rust-install`, created from `main`.

## Global Constraints

- Secret rules (normative, from milestone 1): credentials only in OS keyring + `SecretString`; exactly two `expose_secret()` sites (`secrets.rs`, `romm/mod.rs`); no secrets in config, database, logs, IPC payloads, events, or fixtures; `rewrite/scripts/check_secret_hygiene.sh` must pass after every task.
- grid-core never imports Tauri types.
- Errors cross IPC as `Display` strings; no error text embeds requests, headers, or credentials.
- Cancellation is the typed `LibraryError::Cancelled` — never substring-matching "cancel" in error text.
- All persisted paths: config dir (`Config::default_path().parent()`) holds `config.toml` and `grid-launcher.db`.
- Frontend TS types mirror serde output exactly (snake_case, no renames).
- Tests: `cargo test --workspace` from `rewrite/`, `npm test` from `rewrite/app`. Both green at every commit.
- Commit after every task with the `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>` trailer.

---

### Task 1: Path rules (`paths.rs`)

**Files:**
- Create: `rewrite/crates/grid-core/src/library/mod.rs` (module declarations only for now)
- Create: `rewrite/crates/grid-core/src/library/paths.rs`
- Modify: `rewrite/crates/grid-core/src/lib.rs` (add `pub mod library;`)

**Interfaces:**
- Produces:
  - `pub fn sanitize_component(raw: &str, fallback: &str) -> String`
  - `pub fn archive_name(fs_name: &str, title: &str, platform: &str) -> String`
  - `pub fn extraction_dir(archive: &Path) -> PathBuf`
  - `pub fn platform_dir(library: &Path, platform: &str) -> PathBuf`
  - `pub fn candidate_archives(library: &Path, rec: &super::registry::InstalledGame) -> Vec<PathBuf>` — deferred to Task 4 (registry defines the type); in this task implement it over a small local params struct instead: `pub fn candidate_archives(library: &Path, platform: &str, archive_path: &str, archive_name: &str) -> Vec<PathBuf>`
  - `pub fn candidate_extracted_dirs(archive_candidates: &[PathBuf], extracted_dir: &str) -> Vec<PathBuf>`

**Rules (doc 03, exact):**
- Sanitize: every char in `<>:"/\|?*` and every char with code < 32 → `_`; then trailing spaces and dots → `_`; if the result contains only spaces/underscores/dots, return `fallback`.
- Archive name: last path segment of `fs_name` after replacing `\` with `/`; if empty, `format!("{}-{}.zip", sanitize_component(title, "game"), sanitize_component(platform, "platform"))`.
- Extraction dir: `<archive parent>/<archive stem>`; if that path equals the archive or exists as a **file**, `<stem>_extracted`.
- Platform dir: `library.join(sanitize_component(platform, "Platform"))`.
- Candidate archives, deduped by string, order: `archive_path` (`~`-expanded via `shellexpand`-style manual: only leading `~/` → home) when non-empty; `<platform dir>/<archive_name>`; `<library>/<archive_name>`.
- Candidate extracted dirs: `extracted_dir` when non-empty, then for each candidate archive its `extraction_dir`.

**Steps:**

- [ ] **Step 1: Write failing tests** in `paths.rs` `#[cfg(test)]`: sanitization table (`"a<b>c"` → `"a_b_c"`, `"CON."` → `"CON_"`, `"..."` → fallback, control chars, `"Titan A.E."` trailing-dot case → `"Titan A.E_"`), archive naming (backslash path `"dir\\sub\\Game.zip"` → `"Game.zip"`, empty → `"Safe_Title-Safe_Platform.zip"` shape), extraction-dir collision (dir equals archive stem-less file case and existing-file case → `_extracted`), candidate ordering + dedup.
- [ ] **Step 2:** `cargo test -p grid-core library::paths` → FAIL (unresolved functions).
- [ ] **Step 3:** Implement the functions. No dependencies beyond std + `directories` (home for `~`).
- [ ] **Step 4:** `cargo test -p grid-core` → PASS; `cargo fmt --all`; hygiene script.
- [ ] **Step 5:** Commit `rewrite: library path rules (sanitize, naming, candidates)`.

---

### Task 2: Config `library_path`

**Files:**
- Modify: `rewrite/crates/grid-core/src/config.rs`

**Interfaces:**
- Produces: `Config.library_path: String` (serde `#[serde(default)]`, default `""`). Consumers expand `~` themselves via `paths` helpers.

**Steps:**

- [ ] **Step 1: Failing test:** loading a config without the key yields `""`; saving round-trips a set value; `preserves_unknown_keys` still passes (the flatten map must not shadow the new field).
- [ ] **Step 2:** Run → FAIL. **Step 3:** Add the field to the struct and `Default`. **Step 4:** `cargo test -p grid-core` → PASS.
- [ ] **Step 5:** Commit `rewrite: config gains library_path`.

---

### Task 3: RommClient — ROM detail and streaming response

**Files:**
- Modify: `rewrite/crates/grid-core/src/romm/mod.rs`
- Test: `rewrite/crates/grid-core/tests/romm_detail.rs`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
  pub struct RomFile {
      pub id: i64,
      pub file_name: String,
      #[serde(default)]
      pub file_size_bytes: i64,
      #[serde(default)]
      pub is_top_level: bool,
  }

  #[derive(Debug, Clone)]
  pub struct RomDetail {
      pub id: i64,
      pub name: String,                  // name → fs_name_no_ext fallback, like GameSummary
      pub platform_id: i64,
      pub platform_name: String,         // from platform_display_name
      pub fs_name: String,
      pub description: String,           // from summary, "" when null
      pub regions: String,               // arrays joined with ", "
      pub languages: String,
      pub tags: String,
      pub revision: String,              // "" when null
      pub rating: String,                // metadatum.average_rating formatted "{:.1}", "" when null
      pub genres: String,                // metadatum.genres joined ", "
      pub companies: String,             // metadatum.companies joined ", "
      pub first_release_date: String,    // metadatum.first_release_date stringified, "" when null
      pub filesize_bytes: i64,           // fs_size_bytes
      pub server_updated_at: String,     // updated_at
      pub files: Vec<RomFile>,
  }

  impl RommClient {
      pub async fn rom_detail(&self, rom_id: i64) -> Result<RomDetail, RommError>;
      /// Status-checked GET returning the raw response for streaming.
      /// 401/403 → Unauthorized; other non-2xx → Http with excerpt.
      pub(crate) async fn get_response(
          &self, path: &str, query: &[(&str, String)],
      ) -> Result<reqwest::Response, RommError>;
  }
  ```
- Wire mapping (openapi.json `DetailedRomSchema`): `name` nullable, `fs_name_no_ext` string, `summary` nullable, `regions`/`languages`/`tags` string arrays, `revision` nullable, `fs_size_bytes` int, `updated_at` string, `platform_display_name` string, `files: [RomFileSchema]` (`id`, `file_name`, `file_size_bytes`, `is_top_level`), `metadatum: RomMetadataSchema` (`average_rating` nullable number, `genres`/`companies` nullable string arrays, `first_release_date` nullable integer). Decode through a private `RawRomDetail` (all optionals defaulted) so a sparse payload never fails the decode; `From<RawRomDetail> for RomDetail` applies the fallbacks.
- Refactor `get_json` to call `get_response` then `.json()` so the status handling exists once.

**Steps:**

- [ ] **Step 1: Failing tests** (wiremock, pattern of `tests/romm_client.rs`): full payload maps every field (assert joined strings, rating `"87.3"` style); minimal payload (`{"id":1,"fs_name":"g.zip","fs_name_no_ext":"g","platform_id":2,"platform_display_name":"SNES","fs_size_bytes":0,"updated_at":"","regions":[],"languages":[],"tags":[],"files":[]}` + nulls elsewhere) decodes with empty strings; name fallback to `fs_name_no_ext`; 404 → `Http`.
- [ ] **Step 2:** Run → FAIL. **Step 3:** Implement. **Step 4:** All grid-core tests PASS (existing client tests must be untouched).
- [ ] **Step 5:** Commit `rewrite: RommClient rom_detail + status-checked streaming response`.

---

### Task 4: SQLite registry (`registry.rs`)

**Files:**
- Create: `rewrite/crates/grid-core/src/library/registry.rs`
- Modify: `rewrite/crates/grid-core/Cargo.toml` (`cargo add rusqlite --features bundled`)
- Test: `rewrite/crates/grid-core/tests/registry.rs`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
  pub struct InstalledGame {
      pub title: String,
      pub platform: String,
      pub rom_id: Option<i64>,
      pub rom_file_name: String,
      pub archive_path: String,
      pub extracted_path: String,
      pub extracted_dir: String,
      pub multi_file_game_dir: String,
      pub description: String,
      pub rating: String,
      pub genres: String,
      pub regions: String,
      pub languages: String,
      pub tags: String,
      pub revision: String,
      pub companies: String,
      pub first_release_date: String,
      pub filesize_bytes: i64,
      pub server_updated_at: String,
      pub installed_at: i64,
  }

  pub struct Registry { /* Mutex<rusqlite::Connection> */ }
  impl Registry {
      /// Opens/creates; runs migrations; errors if user_version > LATEST (1).
      pub fn open(path: &Path) -> Result<Self, LibraryError>;
      pub fn upsert(&self, rec: &InstalledGame) -> Result<(), LibraryError>;
      pub fn all(&self) -> Result<Vec<InstalledGame>, LibraryError>;   // ordered by title_key
      pub fn find(&self, rom_id: Option<i64>, title: &str, platform: &str)
          -> Result<Option<InstalledGame>, LibraryError>;
      pub fn remove(&self, title: &str, platform: &str) -> Result<bool, LibraryError>;
  }
  ```
- `LibraryError` starts here in `library/mod.rs`:
  ```rust
  #[derive(Debug, thiserror::Error)]
  pub enum LibraryError {
      #[error(transparent)]
      Romm(#[from] crate::romm::RommError),
      #[error("file error: {0}")]
      Io(#[from] std::io::Error),
      #[error("{0}")]
      Extract(String),
      #[error("Archive extracted but no ROM file was found")]
      NoLaunchFile,
      #[error("cancelled")]
      Cancelled,
      #[error("Set a library folder in settings before installing games")]
      LibraryPathUnset,
      #[error("registry: {0}")]
      Registry(String),
  }
  ```
- Schema: exactly the spec's `CREATE TABLE installed_games` (spec §SQLite registry) with `title_key`/`platform_key` computed in Rust as `trim().to_lowercase()`, `UNIQUE(title_key, platform_key)`, `PRAGMA user_version = 1` after creation. Upsert: `INSERT ... ON CONFLICT(title_key, platform_key) DO UPDATE SET <every column>`.
- `find`: when `rom_id` is `Some`, first `SELECT ... WHERE rom_id = ?`; on no row fall back to the identity-key lookup (doc 03 identity rules).
- Invariant (enforced in `upsert`): when `extracted_path` is non-empty, store `archive_path` as `""` regardless of input (doc 03 invariant 4).

**Steps:**

- [ ] **Step 1: Failing tests** (tempdir DB): open creates file + user_version 1; upsert then `all` round-trips every field; second upsert with same identity but different casing/whitespace (`" Zelda "`/`"SNES"` vs `"zelda"`/`"snes"`) replaces the row (len 1); `find` by rom_id wins over identity; `find` falls back to identity when rom_id is None or unmatched; `remove` returns true/false; upsert with both `extracted_path` and `archive_path` set stores empty `archive_path`; opening a DB with `PRAGMA user_version = 99` errors mentioning "newer".
- [ ] **Step 2:** RED. **Step 3:** Implement. **Step 4:** GREEN + fmt + hygiene.
- [ ] **Step 5:** Commit `rewrite: SQLite install registry`.

---

### Task 5: Launch-file selection (`launch_select.rs`)

**Files:**
- Create: `rewrite/crates/grid-core/src/library/launch_select.rs`

**Interfaces:**
- Produces: `pub fn select_launch_file(root: &Path, archive_stem: &str) -> Option<PathBuf>` (walks recursively, applies the ranking) and, for unit testing without a filesystem, `pub(crate) fn rank(files: &[RelFile], archive_stem: &str) -> Option<usize>` over `pub(crate) struct RelFile { pub rel: PathBuf }` — the walker collects `RelFile`s then returns `root.join(&files[rank])`.

**Exact tables (doc 03 §10 — copy verbatim into constants):**
- `ARCHIVE_SUFFIXES`: `zip 7z rar tar gz bz2 xz`
- `PREFERRED_EXTENSIONS` (priority order): `m3u cue chd iso xex bin pbp cso img ccd nrg mdf gdi rvz gcz wbfs gcm dol elf nes fds sfc smc gba gb gbc n64 z64 v64 nds 3ds cia xci nsp gen smd md 32x sms gg pce sgx a26 a52 a78 lnx ws wsc ngp ngc jag rom`
- `SUPPORT_DIRS`: `__macosx glcache cache caches shadercache shaders docs doc manual manuals readme licenses license resources`
- `SUPPORT_EXTENSIONS`: `txt nfo diz log json xml ini cfg conf url pdf html htm png jpg jpeg gif bmp webp svg ico dll so dylib py lua js css db sqlite tmp cache sav srm state states cht slangp slang glsl vert frag`
- Algorithm: pool = files with suffix not in `ARCHIVE_SUFFIXES` (all files when empty). Penalty = (any parent segment, case-folded, in `SUPPORT_DIRS` → +1) + (suffix in `SUPPORT_EXTENSIONS` → +1). Sort key ascending: `(penalty, preferred_rank_or(len+10), stem_matches_archive ? 0 : 1, path_depth, casefolded_path)`. Selection: if any pool file has a preferred extension → sort those, take first. Else narrow to zero-penalty (whole pool when none), stem-matches within that first if any, else sorted first.

**Steps:**

- [ ] **Step 1: Failing tests:** `.chd` beats `.iso` beats `.bin` (list order); file in `docs/` loses to same extension at root; support-extension (.txt) never beats a preferred ext; stem match breaks ties; shallower wins; deterministic tie-break by casefolded path; archives excluded from pool unless pool would be empty; empty dir → None; nested-only preferred file still found (recursive).
- [ ] **Step 2:** RED. **Step 3:** Implement (walk with `std::fs::read_dir` recursion; no new deps). **Step 4:** GREEN.
- [ ] **Step 5:** Commit `rewrite: launch-file selection ranking`.

---

### Task 6: Extraction engine (`extract.rs`)

**Files:**
- Create: `rewrite/crates/grid-core/src/library/extract.rs`
- Modify: `rewrite/crates/grid-core/Cargo.toml` — `cargo add zip tar flate2 liblzma bzip2 sevenz-rust2`
- Test: `rewrite/crates/grid-core/tests/extract.rs`

**Interfaces:**
- Produces:
  ```rust
  /// Platform-name predicates (doc 03 §3, core subset).
  pub fn is_arcade_platform(platform: &str) -> bool; // contains arcade|mame|fbneo|"final burn"
  pub fn should_extract(platform: &str, archive: &Path) -> bool;
  // suffix in {7z zip tar gz bz2 xz}, case-insensitive; arcade ⇒ false

  pub type ExtractProgress<'a> = &'a mut dyn FnMut(u64, u64); // (processed, total)

  /// Wipes and recreates `dest`, extracts, deletes `dest` on failure.
  /// Blocking — callers wrap in spawn_blocking.
  pub fn extract_archive(archive: &Path, dest: &Path, progress: ExtractProgress)
      -> Result<(), LibraryError>;
  ```
- Dispatch (doc 03 §4 order): suffix `.7z` → sevenz-rust2, then system `7z`/`7za`/`7zz` fallback (search `PATH` then `/usr/bin/7z /usr/bin/7za /usr/bin/7zz /usr/lib/p7zip/7za /opt/homebrew/bin/7z /usr/local/bin/7z /usr/local/bin/7za`; args `x <archive> -o<dest> -y`, stdout null, stderr captured; the fallback wipes and recreates `dest` first; if both fail the error contains both messages). Else ZIP signature (`PK\x03\x04`/`PK\x05\x06`/`PK\x07\x08` at offset 0) → `zip` crate. Else tar: sniff gzip (`1f 8b`), bzip2 (`BZh`), xz (`fd 37 7a 58 5a 00`) magic and wrap the reader, else plain tar.
- Traversal guard (all formats, spec deviation 5): zip via `enclosed_name()` — a `None` fails the extraction with `Extract("archive contains an unsafe path: <name>")`; tar via `unpack_in` returning `Ok(false)` → same error; 7z paths validated with the same rule (reject absolute or `..` components) before writing.
- Zip member paths: backslashes normalized to `/` before the guard (doc 03 invariant 18).
- Progress: total = sum of member uncompressed sizes from archive metadata (zip: `file.size()`; tar: header sizes on a first metadata pass when cheap — for tar, stream and emit processed bytes with total 0; 7z: entry sizes when the crate exposes them, else total 0). Emit after each member; the caller throttles.
- NOTE for implementer: verify the current `sevenz-rust2` extraction API (entry iteration + per-entry extraction to a dir) against its docs before coding; if per-entry progress is impractical, extract whole-archive and emit `(0, total)` then `(total, total)`.

**Steps:**

- [ ] **Step 1: Failing tests** building real archives in the test (zip via `zip` writer; tar.gz via `tar`+`flate2` writer; tar.xz via `liblzma`; 7z: commit a tiny fixture built once with `sevenz-rust2`'s writer in the test itself if the crate supports writing, else generate zip/tar only and gate the 7z test on a `7z` binary probe): extracts contents correctly; wipe-before-extract (pre-existing junk file in dest disappears); failure (truncated zip) deletes dest and returns `Extract`; zip entry named `../evil.txt` → unsafe-path error and dest deleted; `should_extract` table incl. arcade never / `.rar` false / case-insensitive suffixes; progress callback sees a final processed == total for zip.
- [ ] **Step 2:** RED. **Step 3:** Implement. **Step 4:** GREEN + fmt + hygiene (workspace).
- [ ] **Step 5:** Commit `rewrite: pure-Rust extraction engine with traversal guard`.

---

### Task 7: Queue state machine (`queue.rs`)

**Files:**
- Create: `rewrite/crates/grid-core/src/library/queue.rs`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
  #[serde(rename_all = "snake_case")]
  pub enum DownloadStatus { Queued, Downloading, Installing, Cancelling, Completed, Failed, Cancelled }

  #[derive(Debug, Clone, serde::Serialize)]
  pub struct DownloadEntry {
      pub id: u64,
      pub rom_id: i64,
      pub title: String,
      pub platform: String,
      pub status: DownloadStatus,
      pub downloaded_bytes: u64,
      pub total_bytes: u64,
      pub speed_bps: f64,
      pub install_processed_bytes: u64,
      pub install_total_bytes: u64,
      pub error: String,
  }

  #[derive(Debug, Clone, serde::Serialize)]
  pub struct DownloadsSnapshot { pub entries: Vec<DownloadEntry> } // newest first

  pub enum Admission { Start(u64), Queued(u64), Duplicate }

  #[derive(Default)]
  pub struct QueueState { /* entries: Vec<DownloadEntry>, next_id, download_active: Option<u64>, finalize_active: Option<u64>, waiting: VecDeque<u64> */ }
  impl QueueState {
      pub fn admit(&mut self, rom_id: i64, title: &str, platform: &str) -> Admission;
      pub fn set_progress(&mut self, id: u64, downloaded: u64, total: u64, speed: f64);
      pub fn set_install_progress(&mut self, id: u64, processed: u64, total: u64);
      /// Download task ended. Ok(()) → status Installing (finalize slot taken);
      /// Err(Cancelled) → Cancelled; Err(e) → Failed with Display text.
      /// `skip_finalize` (already installed) → Completed instead of Installing.
      pub fn download_finished(&mut self, id: u64, result: Result<(), LibraryError>, skip_finalize: bool);
      /// Finalize ended; frees the finalize slot. warning is appended to error
      /// display on Completed entries when non-empty.
      pub fn finalize_finished(&mut self, id: u64, result: Result<(), LibraryError>, warning: &str);
      /// Pops the next waiting id when both slots are free.
      pub fn next_ready(&mut self) -> Option<u64>;
      pub fn request_cancel(&mut self, id: u64) -> CancelAction; // ActiveDownload | RemovedFromQueue | Ignored
      pub fn dismiss(&mut self, id: u64) -> bool;
      /// Retry precondition: status Failed | Cancelled. Returns rom_id.
      pub fn retryable(&self, id: u64) -> Option<i64>;
      pub fn snapshot(&self) -> DownloadsSnapshot;
      pub fn entry(&self, id: u64) -> Option<&DownloadEntry>;
  }
  ```
- Rules (doc 03 §1, exact): duplicate = same rom_id currently in `download_active`/`finalize_active` entry or in `waiting`; admission starts only when `download_active.is_none() && finalize_active.is_none() && waiting is being bypassed` — i.e. `Start` when both slots free, else `Queued`; terminal statuses zero `speed_bps`; progress setters clamp to ≥ 0 (`speed` to ≥ 0.0); queued-cancel sets error `"Cancelled while queued"`; `download_finished(Ok, skip_finalize=false)` moves the id into `finalize_active` and sets `Installing`; snapshot lists entries newest-first (reverse of insertion).

**Steps:**

- [ ] **Step 1: Failing tests** (in-module): admit on idle → Start + Downloading entry; admit while busy → Queued; duplicate rom queued/active → Duplicate; download Ok → Installing + finalize slot busy; finalize Ok → Completed + `next_ready` pops FIFO; finalize Err → Failed with Display text; download Err(Cancelled) → Cancelled status, empty-ish error, speed zeroed; skip_finalize → Completed directly and `next_ready` fires; cancel queued removes from waiting with the exact error string; cancel active → `ActiveDownload` and status Cancelling; dismiss removes; retryable only for Failed/Cancelled; snapshot order.
- [ ] **Step 2:** RED. **Step 3:** Implement. **Step 4:** GREEN.
- [ ] **Step 5:** Commit `rewrite: install queue state machine`.

---

### Task 8: Streamed download (`download.rs`)

**Files:**
- Create: `rewrite/crates/grid-core/src/library/download.rs`
- Test: `rewrite/crates/grid-core/tests/download.rs`

**Interfaces:**
- Consumes: `RommClient::get_response` (Task 3).
- Produces:
  ```rust
  pub struct FileTarget {
      pub url_path: String,              // e.g. "/api/roms/7/content/Game.zip"
      pub query: Vec<(String, String)>,  // e.g. [("file_ids", "42")]
      pub dest: PathBuf,
      pub expected_size: i64,            // server file_size_bytes, 0 unknown
  }

  /// Downloads every target in order. Progress is cumulative across targets:
  /// (downloaded, total, avg_speed_bps) at most every 100 ms plus a final
  /// emit. total = sum of expected_size when all known, else Content-Length
  /// of the current single target, else 0. Checks `cancel` before each chunk;
  /// on cancellation or error deletes the CURRENT partial file and returns
  /// Cancelled / the error. A target whose dest already exists with size ==
  /// expected_size (> 0) is skipped (counted as fully downloaded) — this is
  /// what makes retry-after-failed-finalize cheap (doc 03 invariant 5).
  pub async fn download_targets(
      client: &RommClient,
      targets: &[FileTarget],
      cancel: &AtomicBool,
      on_progress: &mut (dyn FnMut(u64, u64, f64) + Send),
  ) -> Result<(), LibraryError>;
  ```
- Stream via `resp.bytes_stream()` (add reqwest `stream` feature if needed), write with `std::fs::File` guarded writes (chunks arrive as `Bytes`; no 64 KiB re-chunking needed — reqwest's chunks are fine; the 64 KiB rule from doc 03 described urllib and is satisfied by streaming). Parent dirs created first.

**Steps:**

- [ ] **Step 1: Failing tests** (wiremock): single target downloads bytes to dest and final progress equals body length; multi-target accumulates progress across both files and both land; cancellation set after first chunk (use a wiremock delayed-chunk response or set cancel before the second target) → `Cancelled`, partial file of the in-flight target deleted, completed earlier target kept; HTTP 500 → `Romm(Http)` and partial deleted; pre-existing dest with matching expected_size is not re-requested (wiremock `expect(0)` on that path).
- [ ] **Step 2:** RED. **Step 3:** Implement. **Step 4:** GREEN + hygiene.
- [ ] **Step 5:** Commit `rewrite: streamed multi-target download with cancellation`.

---

### Task 9: InstallService (`library/mod.rs`)

**Files:**
- Modify: `rewrite/crates/grid-core/src/library/mod.rs`
- Test: `rewrite/crates/grid-core/tests/install_service.rs`

**Interfaces:**
- Consumes: everything above + `Config`, `RommClient::rom_detail`.
- Produces:
  ```rust
  pub struct InstallService { /* Mutex<QueueState>, Arc<Registry>, config_path: PathBuf,
      notify: std::sync::RwLock<Option<Arc<dyn Fn(DownloadsSnapshot) + Send + Sync>>>,
      cancel_flags: Mutex<HashMap<u64, Arc<AtomicBool>>>,
      pending_jobs: Mutex<HashMap<u64, InstallJob>>,   // queued jobs awaiting a slot
      last_emit: Mutex<std::time::Instant> */ }

  struct InstallJob { rom_id: i64, detail: RomDetail, targets: Vec<FileTarget>,
      primary_archive: PathBuf, multi_file_game_dir: Option<PathBuf>,
      launch_entry: Option<String> }

  impl InstallService {
      pub fn new(registry: Arc<Registry>, config_path: PathBuf) -> Arc<Self>;
      pub fn set_notify(&self, f: Arc<dyn Fn(DownloadsSnapshot) + Send + Sync>);
      pub fn snapshot(&self) -> DownloadsSnapshot;
      pub fn installed(&self) -> Result<Vec<InstalledGame>, LibraryError>;
      pub async fn install(self: &Arc<Self>, client: Arc<RommClient>, rom_id: i64)
          -> Result<(), LibraryError>;   // Err only for pre-admission failures
      pub fn cancel(&self, entry_id: u64);
      pub async fn retry(self: &Arc<Self>, client: Arc<RommClient>, entry_id: u64)
          -> Result<(), LibraryError>;
      pub fn dismiss(&self, entry_id: u64);
      pub fn uninstall(&self, rom_id: i64) -> Result<(), LibraryError>;
  }
  ```
- `install` flow: load `Config` from `config_path`; empty `library_path` → `LibraryPathUnset`. `rom_detail`. Compute the plan (pure helper `plan_install(detail, library) -> InstallJob`): top-level candidates = files where `is_top_level`, name ≠ `game.json`, no `/` or `\` in name; **0 candidates** → error `Extract("the server lists no downloadable file for this game")`; **1** → single-file: dest `<platform dir>/<archive_name(fs_name,…)>`, url `/api/roms/{id}/content/{file_name}` (percent-encode the file name path segment), query `file_ids=<file.id>`; **>1** → multi-file: dir `<platform dir>/<SafeTitle>/`, one target per candidate (each `file_ids=<id>`), launch entry = first name ending `.m3u` (case-insensitive) else first candidate, `primary_archive` = launch entry's dest. Then admit; `Start` → spawn the download task; `Queued` → stash the job; `Duplicate` → Ok(()) silently.
- Download task (tokio::spawn): runs `download_targets` with a fresh cancel flag; progress → `set_progress` + throttled notify. On completion call `handle_download_finished(id, result)`: registry `find(Some(rom_id), title, platform)` — hit ⇒ `skip_finalize = true` (doc 03 §1 step 4). `download_finished(...)`; if Installing, spawn finalize task; then `pump()` (start next ready job); notify.
- Finalize task (`spawn_blocking` for the blocking parts): multi-file → record with `multi_file_game_dir` + `extracted_path` = launch entry path, no extraction. Single-file: `should_extract`? no → (chmod `.appimage` 0o755 on unix) record with `archive_path` only. yes → `extract_archive` into `extraction_dir(archive)` with progress → `set_install_progress` (throttle 150 ms) → `select_launch_file(dir, stem)` else cleanup dir + `NoLaunchFile` → chmod launch file 0o755 (unix) → build `InstalledGame` from `detail` (all metadata fields, `installed_at` = unix now, `extracted_*` set, `archive_path` empty) → `registry.upsert` → archive cleanup: up to 20 unlink attempts 0.25 s apart; still-failing ⇒ warning `"could not delete archive: <path>"` (visible — spec deviation 3). Then `finalize_finished`, `pump()`, notify.
- Failure invariants: finalize error keeps the archive (no deletion on the error path); download error/cancel deletes partials (Task 8 owns that).
- `uninstall`: find by rom_id; no row → error `Registry("not installed")`. If `multi_file_game_dir` exists as dir → remove it (chmod-retry helper: on `PermissionDenied`, walk the tree `chmod` everything writable once and retry). Else remove existing candidate archives + candidate extracted dirs (Task 1 helpers). Success → `registry.remove`; notify is not needed but the command layer refreshes installed lists client-side.
- `pump()`: `next_ready()` → take the stashed job → mark Downloading → spawn its download task.
- Notify throttle: transitions always emit; progress emits at most every 100 ms (`last_emit`).

**Steps:**

- [ ] **Step 1: Failing integration tests** (wiremock serving `/api/roms/{id}` detail + content bytes; tempdir library + registry; a `Vec<DownloadsSnapshot>` collector as notify):
  - single-file zip end-to-end: entry reaches Completed; archive extracted to `<platform>/<stem>/`; archive deleted; registry row has `extracted_path` set, `archive_path` empty; installed badge lookup finds it.
  - multi-file (2 files incl. `game.m3u`): both files in `<SafeTitle>/`, no extraction, row has `multi_file_game_dir` and `extracted_path` ending `game.m3u`.
  - second install while first downloading → Queued, runs after (use a delayed wiremock body).
  - duplicate rom while queued → no new entry.
  - already-installed rom → Completed without extraction (mock content still downloaded once; registry pre-seeded).
  - finalize failure (corrupt zip) → Failed, archive file still on disk.
  - cancel mid-download → Cancelled, partial gone.
  - uninstall removes files + row; uninstall with a read-only subdir still succeeds (chmod retry).
  - `LibraryPathUnset` when config has no library path.
- [ ] **Step 2:** RED. **Step 3:** Implement. **Step 4:** GREEN + fmt + hygiene. Use `tokio::time::pause`-free real awaits; keep test timeouts generous.
- [ ] **Step 5:** Commit `rewrite: InstallService — queue, finalize, uninstall glue`.

---

### Task 10: Tauri commands and event

**Files:**
- Modify: `rewrite/app/src-tauri/src/commands.rs`, `rewrite/app/src-tauri/src/lib.rs`

**Interfaces:**
- Produces (all `Result<_, String>` via the existing `err` helper):
  - `install_game(rom_id: i64)`, `cancel_install(entry_id: u64)`, `retry_install(entry_id: u64)`, `dismiss_download(entry_id: u64)`, `uninstall_game(rom_id: i64)`, `list_downloads() -> DownloadsSnapshot`, `list_installed() -> Vec<InstalledGame>`, `get_library_path() -> String`, `set_library_path(path: String)`
- `AppState` gains `pub install: Arc<InstallService>`. In `run()`: open the registry at `<config dir>/grid-launcher.db` (config dir = `Config::default_path().parent()`), build the service, and in `.setup` register the notify callback: `app.handle().emit("downloads-changed", snapshot)` (tauri `Emitter`). A registry open failure must not crash startup: store `install: Result<Arc<InstallService>, String>` in AppState; install/uninstall/list commands do `state.install.as_ref().map_err(Clone::clone)?` so the UI surfaces the open error instead of the app dying.
- `set_library_path`: load config, set field, save (`~` kept as typed; expansion happens at use). `get_library_path` returns the raw configured string.

**Steps:**

- [ ] **Step 1:** Implement commands + registration (`generate_handler!` grows by nine). Add a unit test for any pure helper introduced; the command layer itself is exercised by the frontend and Task 9's tests.
- [ ] **Step 2:** `cargo test --workspace` + `cargo clippy --workspace -- -D warnings` GREEN; hygiene.
- [ ] **Step 3:** Commit `rewrite: install/uninstall Tauri commands + downloads-changed event`.

---

### Task 11: Frontend API + downloads store + formatting

**Files:**
- Modify: `rewrite/app/src/lib/api.ts`
- Create: `rewrite/app/src/lib/stores/downloads.svelte.ts`, `rewrite/app/src/lib/downloads/format.ts`, `rewrite/app/src/lib/downloads/format.test.ts`

**Interfaces:**
- `api.ts` additions (types mirror serde exactly):
  ```ts
  export type DownloadStatus = 'queued'|'downloading'|'installing'|'cancelling'|'completed'|'failed'|'cancelled';
  export type DownloadEntry = { id: number; rom_id: number; title: string; platform: string;
    status: DownloadStatus; downloaded_bytes: number; total_bytes: number; speed_bps: number;
    install_processed_bytes: number; install_total_bytes: number; error: string };
  export type DownloadsSnapshot = { entries: DownloadEntry[] };
  export type InstalledGame = { title: string; platform: string; rom_id: number | null;
    rom_file_name: string; archive_path: string; extracted_path: string; extracted_dir: string;
    multi_file_game_dir: string; description: string; rating: string; genres: string; regions: string;
    languages: string; tags: string; revision: string; companies: string; first_release_date: string;
    filesize_bytes: number; server_updated_at: string; installed_at: number };
  // + invoke wrappers: installGame, cancelInstall, retryInstall, dismissDownload,
  //   uninstallGame, listDownloads, listInstalled, getLibraryPath, setLibraryPath
  ```
- `format.ts` (pure, ported from doc 03 tables):
  - `formatSize(n)`: units `B KB MB GB TB`, divide by 1024 while ≥ 1024 and not last unit; 0 decimals for B, 1 decimal otherwise; negatives clamp to 0.
  - `percent(done, total)`: integer clamped 0..100, 0 when total ≤ 0.
  - `entryDetail(e)`: the doc 03 per-status table verbatim (`Queued`; `Downloading 42% • 10.5 MB / 25.0 MB • 1.2 MB/s`; `Downloading • 10.5 MB • 1.2 MB/s` when total 0; `Installing 42% • …` / `Installing...`; `Cancelling...`; `Completed • 25.0 MB` / `Completed • Unknown size`; `Failed • <error || 'Unknown error'>`; `Cancelled`).
  - `aggregate(entries)`: finalize running + no active downloads → `Installing 1 game`; else `N active download(s)`; both with ` (N queued download(s))` suffix when queued exist; empty string when no live entries.
  - `actionFor(status)`: `'cancel' | 'installing' | 'retry-dismiss' | 'dismiss'` per the doc table.
- `downloads.svelte.ts`: `$state` snapshot; `init()` calls `listDownloads` and subscribes to `downloads-changed` (tauri `listen`); exposes `entries`, derived `hasLive`.

**Steps:**

- [ ] **Step 1: Failing vitest** for every `format.ts` function (size units incl. 1024 boundaries and negative clamp; percent clamp; each status detail string exactly; aggregate singular/plural + queued suffix; actionFor table).
- [ ] **Step 2:** `npm test` RED. **Step 3:** Implement. **Step 4:** GREEN; `npx svelte-check` 0 errors.
- [ ] **Step 5:** Commit `rewrite: downloads store + display formatting`.

---

### Task 12: Downloads drawer + footer UI

**Files:**
- Create: `rewrite/app/src/lib/Downloads.svelte`
- Modify: `rewrite/app/src/App.svelte` (footer + drawer mount, store init)

**Design (spec §Frontend):** a fixed footer bar (height ~36 px) visible whenever the session is connected: left = `aggregate()` text (or `Downloads` label when idle), right = a slim progress bar (download percent when active with known total; indeterminate while total 0; install percent during finalize). Clicking the footer toggles the drawer: a bottom sheet listing entries newest-first, each row = title, platform, `entryDetail`, progress bar, and the `actionFor` buttons (Cancel / Retry / Dismiss; Installing rows get no button). Buttons call the api wrappers; errors surface as a row-level message. Follow the existing app.css variables (`--bg`, `--text`, `--accent`, `--border`) and Connect.svelte's button styling. Empty state: `No downloads yet`.

**Steps:**

- [ ] **Step 1:** Implement the components; wire `init()` in App.svelte's `$effect`.
- [ ] **Step 2:** `npm test` + `npx svelte-check` GREEN (no new tests — presentational; formatting logic was tested in Task 11).
- [ ] **Step 3:** Commit `rewrite: downloads drawer and footer status bar`.

---

### Task 13: Details overlay, install/uninstall actions, installed badges, library-path setting

**Files:**
- Create: `rewrite/app/src/lib/Details.svelte`, `rewrite/app/src/lib/stores/installed.svelte.ts`
- Modify: `rewrite/app/src/lib/Library.svelte`, `rewrite/app/src/App.svelte`

**Design:**
- `installed.svelte.ts`: `$state` list from `listInstalled()`; `refresh()`; `isInstalled(game: GameSummary, platformName: string)` — rom_id match first (`rom_id === game.id`), else identity `(title, platform)` trim/lowercase match; re-`refresh()` whenever a `downloads-changed` snapshot contains an entry that just turned `completed` (track previous statuses by id) and after `uninstallGame`.
- `Library.svelte`: badge — a small filled dot + `Installed` chip overlaid on the card corner when `isInstalled`; card `onclick` and gamepad `accept` open the details overlay for the focused/clicked game; pass the active platform's name down.
- `Details.svelte`: modal overlay (Escape/`back` closes): cover (reuse `Cover.svelte`), title, platform, and one primary action — `Install` (calls `installGame(game.id)`, disabled with label `Installing…` while an entry for this rom is live) or `Uninstall` (confirm inline: first click turns the button into `Confirm uninstall`); shows command errors inline.
- Library-path banner: in `Library.svelte`, when `getLibraryPath()` is empty show a banner above the grid: text field + `Save` calling `setLibraryPath`; hide once set. Also render install errors from `LibraryPathUnset` as that banner reappearing.
- Gamepad: `accept` on a focused card opens Details; `back` closes Details if open (App.svelte routes nav to Details first when it is open).

**Steps:**

- [ ] **Step 1:** Implement store + components + wiring.
- [ ] **Step 2:** `npm test` + `npx svelte-check` GREEN; `cargo test --workspace` still green.
- [ ] **Step 3:** Commit `rewrite: details overlay, installed badges, library path setting`.

---

### Task 14: Docs, hygiene, CI touch-ups

**Files:**
- Modify: `rewrite/README.md`, `docs/porting/03-library-install.md`, memory of test counts in any doc that states them

**Steps:**

- [ ] **Step 1:** README: update the Test section counts, document `grid-launcher.db`, add milestone 2 manual checklist (copy the spec's 9-step exit gate), note the new crates.
- [ ] **Step 2:** doc 03: add a short `## Rust port deviations (milestone 2)` section listing the spec's six deviations with one line each, so the porting contract records where the Rust behavior intentionally differs.
- [ ] **Step 3:** Run everything: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo fmt --all --check`, `npm test`, `npx svelte-check`, hygiene script, `python -m unittest discover tests/` (must be untouched).
- [ ] **Step 4:** Commit `rewrite: milestone 2 docs + porting-doc deviation notes`.

# Porting behavior reference

Language-neutral specifications of how GRID Launcher works, written so the
application can be reimplemented in another language without re-discovering
behavior from the Python source. Each document records external surfaces (files,
endpoints, processes), data models, step-by-step algorithms, invariants, platform
differences, concurrency contracts, a test oracle, and a `file:line` source map.

The documents describe the code as of the commit that touches them. Anchors like
`(grid_launcher/core/config.py:231)` point into the Python source at that commit;
re-verify anchors after large refactors.

## Documents

| Doc | Subsystem | Covers |
|-----|-----------|--------|
| [01-romm-api.md](01-romm-api.md) | Server communication | RomM endpoint usage, auth, pagination, metadata merge, Discover data layer, RetroAchievements, PCGamingWiki |
| [02-config-and-secrets.md](02-config-and-secrets.md) | Configuration | Config schema, merge/normalize rules, path conventions, secret storage (keyring/DPAPI) |
| [03-library-install.md](03-library-install.md) | Install pipeline | Download queue, archive extraction chain, flattening, install registry, PS3/PS4/Xbox 360 specials, firmware |
| [04-emulator-launch.md](04-emulator-launch.md) | Launch | Emulator selection, placeholder expansion, Wine/Proton dispatch, native launch, emulator acquisition |
| [05-emulator-autoconfig.md](05-emulator-autoconfig.md) | Autoconfig | Per-emulator native config writing (16 emulators), portable modes, RetroAchievements wiring |
| [06-cloud-saves.md](06-cloud-saves.md) | Cloud saves | Sync candidate discovery, mtime-window algorithm, upload planning, restore, block reasons |
| [07-covers-images.md](07-covers-images.md) | Images | Cover URL resolution, cache schemes, async load pipeline, replenishment, TV image loading |
| [08-background-threading.md](08-background-threading.md) | Concurrency | Worker inventory (31), marshalling contracts, cancellation semantics, shutdown behavior |
| [09-tv-mode.md](09-tv-mode.md) | TV mode | Controller input mapping, navigation model, views, overlay stack, pause flow |
| [10-identity-updates.md](10-identity-updates.md) | Identity | Game identity keys and matching, update detection, app version handling |

## How to read these documents

- **Behavior** sections are normative: a port must reproduce them, including the
  quirks. When the reference implementation does something surprising, the doc
  says so explicitly instead of describing an idealized version.
- **Open questions** mark places where the reference implementation is ambiguous,
  inconsistent, or defective. A port team should decide each one deliberately:
  reproduce the quirk for compatibility, or fix it and note the divergence.
  Several open questions are real defects (see below).
- **Test oracle** sections map behavior to the existing unittest suite
  (`python -m unittest discover tests/`). Port tests can be derived from these:
  the referenced tests encode expected inputs and outputs independent of Python.
- Cross-references between documents are intentional; nothing is documented
  twice. When doc A says "see doc B" for a mechanism, doc B is normative.

## Suggested implementation order for a port

1. **02** — path conventions and config are the substrate everything reads.
2. **10** — identity keys; the registry and every cache key depend on them.
3. **01** — the server client; everything else consumes it.
4. **03** — install pipeline (largest self-contained core).
5. **04**, then **05** — launch, then the config writers launch depends on.
6. **06** — cloud saves (depends on 01, 02, 04's emulator resolution).
7. **07** — images (independent; can go earlier if UI work starts early).
8. **08** — not a component: read it before designing the port's concurrency
   model, since the Python implementation leans on runtime-specific marshalling
   rules a port will not inherit.
9. **09** — TV mode, on top of everything above.

## Notable defects in the reference implementation

Recorded as open questions in the individual documents; collected here because a
port should not reproduce them silently:

- `compat_tool_installs` is reset to `{}` on every config load, and managed
  compat-tool installs are never persisted (docs 02, 04).
- `config.json` is written without an atomic temp-file rename, unlike every
  sibling state file (doc 02).
- The TV bridge's `last_played` write is discarded by the installed-game
  normalizer, as are `update_available`, `revision`, and five other fields
  (docs 02, 03, 10).
- The PS3 firmware download disables TLS certificate verification (doc 03).
- `cloud_mixin.py` calls `self._authorized_headers()`, which is defined nowhere;
  the branch is unreachable only because RomM returns relative paths (doc 06).
- Cloud retention pruning never runs for save states, so state records
  accumulate without bound (doc 06).
- The image cache has no eviction and uses two incompatible filename schemes in
  the same directory; the desktop async loader omits the auth header (doc 07).
- The main window's server-game lookup does not use the canonical identity
  matcher, so match results depend on async load order (doc 10).
- No worker drain on shutdown: an in-flight install is abandoned when the app
  closes (doc 08).
- The cloud-save relative-time formatter's `minute` range is unreachable —
  everything under 24 h renders as "N hours ago" (doc 06).
- `ensure_ppsspp_settings` has two unprotected file reads; an unreadable PPSSPP
  ini crashes the sync path (doc 05).

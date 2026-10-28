# Plan: installing emulators from the RomM server, by a packaging standard

## Why
Server emulator packages are matched today by their ROM name, then downloaded
whole and searched for an executable. That has three problems:
- every build in a package is downloaded, for every OS
- the emulator is only known after the download (and Pico-8's Linux build was not
  recognised at all)
- there is no notion of a version, so nothing can be updated

A packaging standard fixes all three.

## The standard (user decisions, 2026-10-28)
Written up for server admins in `docs/server-emulator-packages.md`.

- **Folders:** under RomM's Emulators platform, one folder per platform named by
  its RomM **slug** (`ps2`, `pico`). Emulators that serve several platforms go in
  a shared **`multi`** folder.
- **File names:** each build is a file named
  `<Emulator> (v<version>) (arch-linux|arch-windows|arch-macos).<ext>`. The name
  before the tags is the emulator's GRID name without its platform suffix,
  case-insensitive. The version tag follows RomM's `(vX.Y.Z)` style; the arch tag
  is GRID's own. Several emulators may share a folder.
- **Versions:** GRID installs the newest version for its own OS, and a newer
  version on the server is offered as an update.
- **Standard only:** packages that don't follow the standard are ignored by setup.
  The Server view still installs them as plain ROMs.

RomM sees each slug folder as ONE multi-file ROM (`fs_name` = the slug) and parses
tags only from a ROM's own name. So GRID parses each file's tags from its
`file_name`, and downloads only the chosen file with
`/api/roms/{id}/content/{file_name}?file_ids=<file id>`. That endpoint is already
used for DLC and update content (`library/content.rs`).

## Backend (grid-core)
1. **`library/emulator_package.rs`** (pure):
   - `parse_package_file(name) -> Option<PackageFile {emulator, version, os, ext}>`
   - `compare_versions` (numbers, then a trailing letter suffix)
   - `newest_for_host(files, host) -> per emulator`

   Unit tests cover:
   - every tag form
   - missing or duplicate tags
   - unknown extensions
   - `v0.2.7a6` against `v0.2.7`
   - case

2. **Listing.** `ServerPackage` becomes the folder (ROM id, slug = `fs_name`, its
   files). `identify_package`'s name matching is replaced:
   - **A slug folder** serves the platform with that slug. Each parsed file names
     its emulator, which must be a known profile that is not a compat tool.
   - **`multi`** serves the platforms the emulator's profile claims: keywords, and
     RetroArch where a core is known (the same core rule; checked again after
     install).

   Setup options carry the ROM id, the file id and the version. The order is
   unchanged: catalog first, then server, then RetroArch.

3. **Install job.** A server emulator install downloads only the chosen file (a
   content-style job over `file_ids`), then:
   - unpacks or keeps it
   - registers it with the profile the file name gave, so no executable matching
     is needed to identify it (the executable is still picked from the files)
   - records `source_provider = "romm"`, the ROM id, the file name and the version
     on the entry

   The Downloads row keeps the Emulator badge. The `" (server)"` suffix rule is
   unchanged.

4. **Updates.** Emulators › Installed checks a server-installed emulator against
   the newest version for this OS in its folder, with the same Update flow as
   catalog emulators.

5. **Server view.** Installing a standard slug or `multi` folder from the
   Emulators platform installs that folder's emulators for this OS through
   step 3, instead of downloading every build.

6. **Data fix:** add `ps4` (RomM's slug on this server) beside `ps4--1` in
   `platform-emulators.json`.

## Frontend
- Server options read "PCSX2 2.0.2 · from your server".
- The setup card already follows game rows by ROM id. A file-level job keeps its
  ROM id, so no change should be needed (to verify).

## Tests
- **Rust:** the parser, versions, listing (slug and `multi`), choosing the file for
  this host, the install job against wiremock (only the chosen file id
  requested), registration from the file name, and update detection.
- **e2e:**
  - The mock server's Emulators platform gets a `ps2` folder (Linux and Windows
    PCSX2 builds), a `pico` folder and a non-standard package that must be ignored.
  - The mock serves `file_ids` content.
  - first-run-setup installs PCSX2 from the server and asserts only the Linux file
    was requested (from the mock's request log).
  - server-emulator is updated to the standard layout.
- **Live check (read-only):** list your server's Emulators platform once it is
  repackaged, and confirm the parse.

## Docs
- `docs/server-emulator-packages.md` (the standard).
- SPEC: server emulator packages, setup, updates.

## Order of work (each gated and committed)
1. Parser and versions.
2. Listing and setup options (replaces name matching), plus the `ps4` data fix.
3. File-level install job and registration from the file name.
4. Update detection for server emulators.
5. Server-view install of a standard folder.
6. e2e fixtures and specs, SPEC, design system note.

## Status (2026-10-28)
Steps 1–5 are done. The Server view's details panel does not show "Installing…" for a
standard folder, because its rows are emulator jobs with no ROM id. The Downloads row
shows the progress.

## Not covered (say if wanted)
- CPU architectures other than x86-64: arm64 builds would need an extra tag, such
  as `arch-linux-arm64`.
- Migrating your current Pico-8 package: repackage it on the server as
  `roms/emulators/pico/Pico-8 (v0.2.7a6) (arch-linux).zip` and the Windows twin,
  then rescan.

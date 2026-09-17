# Vulkan renderer seed — design (2026-09-17)

## Goal

A fresh emulator install from GRID starts on the Vulkan renderer wherever the emulator
offers one. The seed never overrides a value the user or the emulator already wrote, and it
never runs against an emulator that was installed earlier.

## Trigger

A Cemu install made by GRID booted Breath of the Wild to a black screen until the renderer
was switched from OpenGL to Vulkan by hand. The cause is GRID's own `settings.xml` template
(`autoconfig/cemu.rs`, `DEFAULT_CEMU_SETTINGS_XML`), transcribed from the Python app, which
pins `<Graphic><api>0</api>` (OpenGL). Cemu itself has defaulted to Vulkan since 1.22.10.

## Scope

Seeded on a fresh install, Linux and Windows only:

| Emulator | File | Seed |
|---|---|---|
| Cemu | `portable/settings.xml` | `<Graphic><api>1</api>`; the template itself changes to `1` |
| Dolphin | `Config/Dolphin.ini` | `[Core] GFXBackend = Vulkan` |
| DuckStation | `settings.ini` | `[GPU] Renderer = Vulkan` |
| PPSSPP | `memstick/PSP/SYSTEM/PPSSPP.INI` | `[Graphics] GraphicsBackend = 3 (VULKAN)` |
| Azahar | `qt-config.ini` | `[Renderer] graphics_api\default=false` and `graphics_api=2` |
| xemu | `xemu.toml` | `[display] renderer = 'VULKAN'` |

Not seeded, and why:

- PCSX2 already seeds `[EmuCore/GS] Renderer = 14` (Vulkan) with per-key preserve.
- RPCS3, Eden, Vita3K default to Vulkan on their own.
- ShadPS4 and KytyPS5 are Vulkan-only.
- Redream and Supermodel have no Vulkan renderer.
- Xenia has no writer; the Linux build is Vulkan-only and D3D12 is the better default on
  Windows.
- RetroArch keeps its default `video_driver`. One global driver serves every core, and a few
  cores (GlideN64, the Dolphin core on some AMD systems) misbehave under `vulkan`.
- macOS: nothing is seeded on any emulator. The emulators' Metal or automatic choices beat
  MoltenVK.

## Mechanism

### Freshness flag

`autoconfig::SyncContext` gains `fresh_install: bool`. The two callers of
`sync_new_emulator` already know the answer:

- `InstallService::finalize_emulator` (`library/mod.rs`) computes `fresh` from
  `write_emulator_entry` immediately before `sync_autoconfig`; it passes that value through.
- `save_emulator` (`app/src-tauri/src/commands.rs`) syncs only when `is_add`; it passes
  `true`.

Every other trigger (RetroArch launch hooks, RetroAchievements fan-out, firmware routing)
does not construct a `SyncContext` for the profile writers and is unaffected. A catalog
update or a reinstall into an existing entry is `fresh == false`: renderers are left alone.

### Writer contract

Each of the six writers gains the renderer key in its existing section pass, guarded by
`fresh_install` and by `cfg!(not(target_os = "macos"))`, with per-key preserve:

- Overwrite-policy writers (Dolphin, DuckStation, Azahar, PPSSPP, Cemu) probe the section
  first and omit the key when it is present, the way PCSX2's `[EmuCore/GS]` and
  DuckStation's `[GPU]` blocks already do.
- xemu's `toml_add_only_section` is add-only by construction.
- Cemu: the create-from-template branch writes `<api>1</api>`. The merge branch adds
  `<api>1</api>` inside `<Graphic>` only when the element has no `<api>` child. When
  `<Graphic>` itself is absent the writer creates it with the `<api>` child only, not the
  full template block.

The flag reaches the writers as one extra `bool` parameter on their `ensure_settings`; the
narrow writers (`ensure_ra_credentials`, `ensure_skip_ipl`, `ensure_gcpad_config`) do not
change. DuckStation's full writer is `ensure_memory_card_settings`; it takes the flag like
the others.

### Idempotency

A second `sync_new_emulator` with `fresh_install == true` over a file that already carries
the key writes nothing. `fresh_install == false` never adds the key.

## Testing

Per writer, in the module's `#[cfg(test)]` block:

1. fresh install, key absent: key is written with the Vulkan literal;
2. fresh install, key present with another value: the value survives;
3. not fresh, key absent: key is not written;
4. second fresh call: `changed == false`.

Cemu additionally asserts the template constant contains `<api>1</api>` and that a merged
file with `<Graphic>` but no `<api>` gains exactly one `<api>1</api>` child.

`autoconfig/mod.rs`: one test that `sync_new_emulator` forwards `fresh_install` to a writer
(observable through the Cemu or Dolphin file it writes into a temp emulator directory).

E2E: the existing `emulators` stage installs a catalog emulator against the mock forge; add
one assertion that the resulting config file carries its Vulkan key. No new stage.

## Documentation

- `.claude/skills/emulator-autoconfig/SKILL.md`: a "Renderer seed" subsection stating the
  flag, the per-key preserve rule, the macOS exclusion, and the table above.
- `ARCHITECTURE.md`: one sentence where `SyncContext` is described, if it is.

## Out of scope

- Re-seeding emulators installed before this change. The user's Cemu is already on Vulkan
  by hand; other installs keep whatever they have.
- A UI toggle for the preferred renderer.
- Xenia, RetroArch and Vita3K writers.
- Per-game renderer overrides.

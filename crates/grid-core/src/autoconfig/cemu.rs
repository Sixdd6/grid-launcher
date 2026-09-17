//! Cemu's portable `settings.xml` and the default controller profile.
//!
//! Ports `grid_launcher/emulator/cemu.py`'s `ensure_cemu_settings`
//! (cemu.py:286-329), `ensure_cemu_controller_config` (cemu.py:333-358) and
//! `cemu_settings_path_candidates` (cemu.py:252-270). See
//! `docs/porting/05-emulator-autoconfig.md` ("Cemu") for the behavior
//! contract.
//!
//! Spec deviation D11 (binding, recorded for Task 13): the reference parses
//! `settings.xml` with `xml.etree.ElementTree` and reserializes the whole
//! root element with it. This crate has no XML crate dependency, so
//! [`apply_forced_elements`] instead does a minimal, hand-written,
//! byte-preserving text edit: it locates each of the six forced
//! `<tag>...</tag>` pairs inside the `<content>...</content>` root by
//! regex on the raw text and rewrites only the inner text (or appends a new
//! `<tag>value</tag>` right before `</content>` when the tag is absent).
//! Every other byte inside the root is left untouched — which is STRONGER
//! than `ElementTree`'s reserialize-the-whole-tree behavior, not weaker.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use super::{paths, EnsureResult};

const DEFAULT_CEMU_XINPUT_CONTROLLER_PROFILE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<emulated_controller>
    <type>Wii U Pro Controller</type>
    <controller>
        <api>XInput</api>
        <uuid>0</uuid>
        <display_name>Controller 1</display_name>
        <rumble>1</rumble>
        <axis>
            <deadzone>0.15</deadzone>
            <range>1</range>
        </axis>
        <rotation>
            <deadzone>0.15</deadzone>
            <range>1</range>
        </rotation>
        <trigger>
            <deadzone>0.15</deadzone>
            <range>1</range>
        </trigger>
        <mappings>
            <entry><mapping>1</mapping><button>13</button></entry>
            <entry><mapping>2</mapping><button>12</button></entry>
            <entry><mapping>3</mapping><button>15</button></entry>
            <entry><mapping>4</mapping><button>14</button></entry>
            <entry><mapping>5</mapping><button>8</button></entry>
            <entry><mapping>6</mapping><button>9</button></entry>
            <entry><mapping>7</mapping><button>42</button></entry>
            <entry><mapping>8</mapping><button>43</button></entry>
            <entry><mapping>9</mapping><button>4</button></entry>
            <entry><mapping>10</mapping><button>5</button></entry>
            <entry><mapping>12</mapping><button>0</button></entry>
            <entry><mapping>13</mapping><button>1</button></entry>
            <entry><mapping>14</mapping><button>2</button></entry>
            <entry><mapping>15</mapping><button>3</button></entry>
            <entry><mapping>16</mapping><button>6</button></entry>
            <entry><mapping>17</mapping><button>7</button></entry>
            <entry><mapping>18</mapping><button>39</button></entry>
            <entry><mapping>19</mapping><button>45</button></entry>
            <entry><mapping>20</mapping><button>44</button></entry>
            <entry><mapping>21</mapping><button>38</button></entry>
            <entry><mapping>22</mapping><button>41</button></entry>
            <entry><mapping>23</mapping><button>47</button></entry>
            <entry><mapping>24</mapping><button>46</button></entry>
            <entry><mapping>25</mapping><button>40</button></entry>
        </mappings>
    </controller>
</emulated_controller>
"#;

const DEFAULT_CEMU_SDL_CONTROLLER_PROFILE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<emulated_controller>
    <type>Wii U Pro Controller</type>
    <controller>
        <api>SDLController</api>
        <uuid>0</uuid>
        <display_name>Controller 1</display_name>
        <rumble>1</rumble>
        <axis>
            <deadzone>0.15</deadzone>
            <range>1</range>
        </axis>
        <rotation>
            <deadzone>0.15</deadzone>
            <range>1</range>
        </rotation>
        <trigger>
            <deadzone>0.15</deadzone>
            <range>1</range>
        </trigger>
        <mappings>
            <entry><mapping>1</mapping><button>1</button></entry>
            <entry><mapping>2</mapping><button>0</button></entry>
            <entry><mapping>3</mapping><button>3</button></entry>
            <entry><mapping>4</mapping><button>2</button></entry>
            <entry><mapping>5</mapping><button>4</button></entry>
            <entry><mapping>6</mapping><button>6</button></entry>
            <entry><mapping>7</mapping><button>42</button></entry>
            <entry><mapping>8</mapping><button>43</button></entry>
            <entry><mapping>9</mapping><button>9</button></entry>
            <entry><mapping>10</mapping><button>10</button></entry>
            <entry><mapping>12</mapping><button>11</button></entry>
            <entry><mapping>13</mapping><button>12</button></entry>
            <entry><mapping>14</mapping><button>13</button></entry>
            <entry><mapping>15</mapping><button>14</button></entry>
            <entry><mapping>16</mapping><button>7</button></entry>
            <entry><mapping>17</mapping><button>8</button></entry>
            <entry><mapping>18</mapping><button>45</button></entry>
            <entry><mapping>19</mapping><button>39</button></entry>
            <entry><mapping>20</mapping><button>44</button></entry>
            <entry><mapping>21</mapping><button>38</button></entry>
            <entry><mapping>22</mapping><button>47</button></entry>
            <entry><mapping>23</mapping><button>41</button></entry>
            <entry><mapping>24</mapping><button>46</button></entry>
            <entry><mapping>25</mapping><button>40</button></entry>
        </mappings>
    </controller>
</emulated_controller>
"#;

/// cemu.py:115-237, transcribed verbatim EXCEPT `<Graphic><api>`, which is
/// `1` (Vulkan) here where the reference pinned `0` (OpenGL): Cemu's own
/// default has been Vulkan since 1.22.10, and the OpenGL pin booted Breath
/// of the Wild to a black screen. First line the UPPERCASE-`UTF-8` XML
/// declaration, last line `</content>`, exactly one trailing newline. The
/// six forced elements already carry their desired values here, so the
/// create-from-template branch never needs to touch this text.
const DEFAULT_CEMU_SETTINGS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<content>
    <logflag>0</logflag>
    <advanced_ppc_logging>false</advanced_ppc_logging>
    <mlc_path></mlc_path>
    <permanent_storage>true</permanent_storage>
    <language>0</language>
    <use_discord_presence>false</use_discord_presence>
    <fullscreen_menubar>false</fullscreen_menubar>
    <feral_gamemode>false</feral_gamemode>
    <check_update>false</check_update>
    <receive_untested_updates>false</receive_untested_updates>
    <save_screenshot>true</save_screenshot>
    <vk_warning>false</vk_warning>
    <gp_download>true</gp_download>
    <macos_disclaimer>false</macos_disclaimer>
    <fullscreen>false</fullscreen>
    <proxy_server></proxy_server>
    <disable_screensaver>true</disable_screensaver>
    <play_boot_sound>false</play_boot_sound>
    <console_language>1</console_language>
    <window_position>
        <x>-1</x>
        <y>-1</y>
    </window_position>
    <window_size>
        <x>-1</x>
        <y>-1</y>
    </window_size>
    <window_maximized>true</window_maximized>
    <open_pad>false</open_pad>
    <pad_position>
        <x>-1</x>
        <y>-1</y>
    </pad_position>
    <pad_size>
        <x>-1</x>
        <y>-1</y>
    </pad_size>
    <pad_maximized>false</pad_maximized>
    <show_icon_column>true</show_icon_column>
    <GameList>
        <style>0</style>
        <order></order>
        <name_width>500</name_width>
        <version_width>500</version_width>
        <dlc_width>500</dlc_width>
        <game_time_width>500</game_time_width>
        <game_started_width>500</game_started_width>
        <region_width>500</region_width>
        <title_id>500</title_id>
    </GameList>
    <RecentLaunchFiles/>
    <RecentNFCFiles/>
    <GamePaths/>
    <GameCache/>
    <GraphicPack/>
    <Graphic>
        <api>1</api>
        <device>00000000000000000000000000000000</device>
        <VSync>0</VSync>
        <GX2DrawdoneSync>true</GX2DrawdoneSync>
        <UpscaleFilter>2</UpscaleFilter>
        <DownscaleFilter>0</DownscaleFilter>
        <FullscreenScaling>0</FullscreenScaling>
        <AsyncCompile>true</AsyncCompile>
        <vkAccurateBarriers>true</vkAccurateBarriers>
        <Overlay>
            <Position>0</Position>
            <TextColor>4294967295</TextColor>
            <TextScale>100</TextScale>
            <FPS>true</FPS>
            <DrawCalls>false</DrawCalls>
            <CPUUsage>false</CPUUsage>
            <CPUPerCoreUsage>false</CPUPerCoreUsage>
            <RAMUsage>false</RAMUsage>
            <VRAMUsage>false</VRAMUsage>
            <Debug>false</Debug>
        </Overlay>
        <Notification>
            <Position>1</Position>
            <TextColor>4294967295</TextColor>
            <TextScale>100</TextScale>
            <ControllerProfiles>true</ControllerProfiles>
            <ControllerBattery>true</ControllerBattery>
            <ShaderCompiling>true</ShaderCompiling>
            <FriendService>true</FriendService>
        </Notification>
    </Graphic>
    <Audio>
        <api>3</api>
        <delay>2</delay>
        <TVChannels>1</TVChannels>
        <PadChannels>1</PadChannels>
        <InputChannels>0</InputChannels>
        <TVVolume>30</TVVolume>
        <PadVolume>0</PadVolume>
        <InputVolume>20</InputVolume>
        <TVDevice>default</TVDevice>
        <PadDevice></PadDevice>
        <InputDevice></InputDevice>
    </Audio>
    <Account>
        <PersistentId>2147483649</PersistentId>
        <OnlineEnabled>false</OnlineEnabled>
        <ActiveService>0</ActiveService>
    </Account>
    <AccountService/>
    <Debug>
        <CrashDumpWindows>0</CrashDumpWindows>
        <GDBPort>1337</GDBPort>
    </Debug>
    <Input>
        <DSUC host="127.0.0.1" port="26760"/>
    </Input>
    <EmulatedUsbDevices>
        <EmulateSkylanderPortal>false</EmulateSkylanderPortal>
        <EmulateInfinityBase>false</EmulateInfinityBase>
        <EmulateDimensionsToypad>false</EmulateDimensionsToypad>
    </EmulatedUsbDevices>
</content>
"#;

/// The six forced elements, in the pinned order (cemu.py:314-320).
const FORCED_ELEMENTS: &[(&str, &str)] = &[
    ("use_discord_presence", "false"),
    ("check_update", "false"),
    ("receive_untested_updates", "false"),
    ("gp_download", "true"),
    ("fullscreen", "false"),
    ("window_maximized", "true"),
];

static CONTENT_CLOSE: &str = "</content>";

fn element_regex(tag: &str) -> Regex {
    Regex::new(&format!(r"<{tag}>([^<]*)</{tag}>")).expect("static tag names are valid regex")
}

/// One `<tag>...</tag>` regex per [`FORCED_ELEMENTS`] entry, in the same
/// order, compiled once and reused across every [`apply_forced_elements`]
/// call instead of being rebuilt on each of the six tags on every
/// `ensure_settings` call.
static FORCED_ELEMENT_REGEXES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    FORCED_ELEMENTS
        .iter()
        .map(|(tag, _)| element_regex(tag))
        .collect()
});

/// Rewrite `<tag>...</tag>`'s inner text to `value` inside `root_span` when
/// it differs, or append `<tag>value</tag>` right before the root span's own
/// closing `</content>` when the tag is absent. Returns whether a change was
/// made. `re` is `tag`'s precompiled regex from [`FORCED_ELEMENT_REGEXES`].
fn set_or_insert_element(root_span: &mut String, tag: &str, value: &str, re: &Regex) -> bool {
    if let Some(caps) = re.captures(root_span.as_str()) {
        let whole = caps.get(0).unwrap();
        if &caps[1] == value {
            return false;
        }
        let range = whole.range();
        let replacement = format!("<{tag}>{value}</{tag}>");
        root_span.replace_range(range, &replacement);
        return true;
    }

    let insert_at = root_span.len() - CONTENT_CLOSE.len();
    let insertion = format!("<{tag}>{value}</{tag}>");
    root_span.insert_str(insert_at, &insertion);
    true
}

/// Byte ranges of every `<!-- ... -->` XML comment in `text`, non-greedy so
/// back-to-back comments are captured separately. Used to keep the D11
/// literal-substring scan below from mistaking a `<content>`/`</content>`
/// string that only appears INSIDE a comment for the real root's tags.
///
/// `pub(crate)` — `readers.rs`'s `cemu_mlc_path_from_xml` reuses this
/// (alongside [`position_is_commented_out`]) so its own `<mlc_path>` scan
/// gets the same comment-skipping guarantee this module's writer has,
/// rather than maintaining a second, divergent copy.
static COMMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<!--.*?-->").expect("static regex is valid"));

pub(crate) fn comment_ranges(text: &str) -> Vec<(usize, usize)> {
    COMMENT_RE
        .find_iter(text)
        .map(|m| (m.start(), m.end()))
        .collect()
}

/// `pub(crate)` for the same reason as [`comment_ranges`].
pub(crate) fn position_is_commented_out(pos: usize, comments: &[(usize, usize)]) -> bool {
    comments
        .iter()
        .any(|&(start, end)| pos >= start && pos < end)
}

/// Locate the `<content>...</content>` root's byte span: the FIRST
/// `<content>` open tag and the LAST `</content>` close tag in the raw
/// text, each SKIPPING any occurrence that falls inside an XML comment
/// (`<!-- ... -->`) — a comment elsewhere in the file (commonly right
/// before the root) can otherwise contain a literal `<content>`/`</content>`
/// that this substring scan would mistake for the real root, corrupting the
/// rewritten file. Returns the `(open_start, close_end)` byte range, or
/// `None` when no un-commented `<content>` tag exists, no un-commented
/// `</content>` tag exists, or the close precedes the open.
fn find_root_span(content: &str) -> Option<(usize, usize)> {
    let comments = comment_ranges(content);

    let open_pos = content
        .match_indices("<content>")
        .map(|(idx, _)| idx)
        .find(|&idx| !position_is_commented_out(idx, &comments))?;

    let close_pos = content
        .match_indices(CONTENT_CLOSE)
        .map(|(idx, _)| idx)
        .filter(|&idx| !position_is_commented_out(idx, &comments))
        .last()?;

    if close_pos < open_pos {
        return None;
    }
    Some((open_pos, close_pos + CONTENT_CLOSE.len()))
}

/// D11: locate the `<content>...</content>` root via [`find_root_span`],
/// apply the six forced elements in order, then — only when `seed_renderer`
/// — the `<Graphic><api>` seed ([`seed_graphic_api`]), and return the
/// (possibly edited) root span plus whether anything changed. `None` on any
/// parse failure: empty (after trim) content, no un-commented `<content>`
/// tag, or a close tag that doesn't follow the open tag — cemu.py:308-312's
/// `root is None` / `ET.ParseError` branch.
fn apply_forced_elements(content: &str, seed_renderer: bool) -> Option<(String, bool)> {
    if content.trim().is_empty() {
        return None;
    }
    let (open_pos, close_end) = find_root_span(content)?;

    let mut root_span = content[open_pos..close_end].to_string();
    let mut changed = false;
    for ((tag, value), re) in FORCED_ELEMENTS.iter().zip(FORCED_ELEMENT_REGEXES.iter()) {
        changed |= set_or_insert_element(&mut root_span, tag, value, re);
    }
    if seed_renderer {
        changed |= seed_graphic_api(&mut root_span);
    }

    Some((root_span, changed))
}

/// The `<Graphic>...</Graphic>` element, matched on the literal open tag WITH
/// its `>` so the sibling `<GraphicPack/>` can never match. Group 1 is the
/// inner text; its start is where the `<api>` child goes.
static GRAPHIC_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<Graphic>(.*?)</Graphic>").expect("static regex is valid"));

/// An `<api>` child, probed ONLY inside `<Graphic>`'s inner text — `<Audio>`
/// has its own `<api>` and must not satisfy the probe.
static GRAPHIC_API_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<api>[^<]*</api>").expect("static regex is valid"));

/// Cemu's renderer enum: `1` is Vulkan.
const GRAPHIC_API_VULKAN: &str = "<api>1</api>";

/// The renderer seed for the MERGE branch (spec
/// `2026-09-17-vulkan-renderer-seed-design.md`): add `<api>1</api>` inside
/// `<Graphic>` when that element has no `<api>` child; when `<Graphic>` is
/// absent (or self-closing) create it with the `<api>` child ONLY, right
/// before `</content>` — never the full template block. Per-key preserve:
/// any existing `<api>` value, `0` included, is left alone. Returns whether
/// `root_span` changed.
fn seed_graphic_api(root_span: &mut String) -> bool {
    if let Some(caps) = GRAPHIC_RE.captures(root_span.as_str()) {
        let inner = caps.get(1).unwrap();
        if GRAPHIC_API_RE.is_match(inner.as_str()) {
            return false;
        }
        let insert_at = inner.start();
        root_span.insert_str(insert_at, GRAPHIC_API_VULKAN);
        return true;
    }
    if let Some(pos) = root_span.find("<Graphic/>") {
        root_span.replace_range(
            pos..pos + "<Graphic/>".len(),
            "<Graphic><api>1</api></Graphic>",
        );
        return true;
    }
    let insert_at = root_span.len() - CONTENT_CLOSE.len();
    root_span.insert_str(insert_at, "<Graphic><api>1</api></Graphic>");
    true
}

fn is_windows_host() -> bool {
    cfg!(target_os = "windows")
}

/// `cemu_settings_path_candidates` (cemu.py:252-270), deduped
/// case-insensitively. With a non-blank `emulator_path` (expanded,
/// dir-or-parent): `<emulator_dir>/portable/settings.xml`, then
/// `<emulator_dir>/settings.xml`. Then, by host: on Windows, `%APPDATA%` and
/// `%LOCALAPPDATA%` (each only when set and non-blank once trimmed), each
/// joined with `Cemu/settings.xml`; elsewhere, `<xdg_config_home>/Cemu/settings.xml`.
pub fn settings_path_candidates(emulator_path: &str) -> Vec<PathBuf> {
    settings_path_candidates_for(emulator_path, is_windows_host())
}

/// [`settings_path_candidates`] with an explicit host flag, so a test can
/// drive both branches regardless of the host this crate is compiled for.
fn settings_path_candidates_for(emulator_path: &str, is_windows: bool) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    let trimmed = emulator_path.trim();
    if !trimmed.is_empty() {
        let expanded = paths::expand_user(trimmed);
        if let Some(emulator_dir) = paths::emulator_dir(&expanded) {
            if !emulator_dir.as_os_str().is_empty() {
                candidates.push(emulator_dir.join("portable").join("settings.xml"));
                candidates.push(emulator_dir.join("settings.xml"));
            }
        }
    }

    if is_windows {
        for var in ["APPDATA", "LOCALAPPDATA"] {
            if let Some(dir) = paths::env_dir(var) {
                candidates.push(dir.join("Cemu").join("settings.xml"));
            }
        }
    } else {
        candidates.push(paths::xdg_config_home().join("Cemu").join("settings.xml"));
    }

    paths::dedupe_casefold(candidates)
}

/// `ensure_cemu_settings` (cemu.py:286-329). Blank path (after `.trim()`) or
/// a resolved `emulator_dir` with no path text at all returns
/// [`EnsureResult::unchanged`].
///
/// Creates `<emulator_dir>/portable/` unconditionally before any file
/// check, targeting `<emulator_dir>/portable/settings.xml`. When that file
/// is missing, [`DEFAULT_CEMU_SETTINGS_XML`] is written and `changed = true`
/// is reported immediately. Otherwise the file is parsed via
/// [`apply_forced_elements`] (D11) and rewritten only when something
/// changed, as `<?xml version="1.0" encoding="utf-8"?>\n` (lowercase here,
/// unlike the template) followed by the edited root, with no added trailing
/// newline. **Every** failure — parse error and I/O alike — yields
/// [`EnsureResult::unchanged`] (cemu.py:326-327's bare `except Exception`).
///
/// `fresh_install` gates the renderer seed on the merge branch
/// ([`seed_graphic_api`]): only a fresh install, and never on macOS, adds
/// `<Graphic><api>1</api>` to an existing file lacking it. The
/// create-from-template branch is unaffected — the template already says
/// `1`.
pub fn ensure_settings(emulator_path: &str, fresh_install: bool) -> EnsureResult {
    let trimmed = emulator_path.trim();
    if trimmed.is_empty() {
        return EnsureResult::unchanged();
    }
    let expanded = paths::expand_user(trimmed);
    let Some(emulator_dir) = paths::emulator_dir(&expanded) else {
        return EnsureResult::unchanged();
    };
    if emulator_dir.as_os_str().is_empty() {
        return EnsureResult::unchanged();
    }

    let portable_dir = emulator_dir.join("portable");
    if std::fs::create_dir_all(&portable_dir).is_err() {
        return EnsureResult::unchanged();
    }
    let target = portable_dir.join("settings.xml");

    if !target.exists() {
        return match std::fs::write(&target, DEFAULT_CEMU_SETTINGS_XML) {
            Ok(()) => EnsureResult::at(target, true),
            Err(_) => EnsureResult::unchanged(),
        };
    }

    let Ok(content) = std::fs::read_to_string(&target) else {
        return EnsureResult::unchanged();
    };

    let seed_renderer = fresh_install && cfg!(not(target_os = "macos"));
    let Some((new_root_span, changed)) = apply_forced_elements(&content, seed_renderer) else {
        return EnsureResult::unchanged();
    };

    if changed {
        let output = format!("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n{new_root_span}");
        if std::fs::write(&target, &output).is_err() {
            return EnsureResult::unchanged();
        }
    }

    EnsureResult::at(target, changed)
}

/// `ensure_cemu_controller_config` (cemu.py:333-358). **No `.trim()` and no
/// `~` expansion** on `emulator_path` — cemu.py:339 builds `Path` directly
/// from the raw text (parity, not a bug: see the module-level Cemu section
/// of doc 05). Target: `<emulator_dir>/portable/controllerProfiles/controller0.xml`.
///
/// Already present → no write, `changed = false`, `extras["profile_path"]`
/// still set to the target. Otherwise writes
/// [`DEFAULT_CEMU_XINPUT_CONTROLLER_PROFILE`] on Windows,
/// [`DEFAULT_CEMU_SDL_CONTROLLER_PROFILE`] everywhere else. `config_path` is
/// always `None`; any I/O failure yields [`EnsureResult::unchanged`] with no
/// `profile_path` extra at all (cemu.py:356-357's `except OSError`).
pub fn ensure_controller_config(emulator_path: &str) -> EnsureResult {
    let path = PathBuf::from(emulator_path);
    let emulator_dir = if path.is_dir() {
        path
    } else {
        path.parent().map(Path::to_path_buf).unwrap_or_default()
    };
    let target = emulator_dir
        .join("portable")
        .join("controllerProfiles")
        .join("controller0.xml");

    if target.exists() {
        return EnsureResult::unchanged().with_extra("profile_path", target);
    }

    let profile_content = if is_windows_host() {
        DEFAULT_CEMU_XINPUT_CONTROLLER_PROFILE
    } else {
        DEFAULT_CEMU_SDL_CONTROLLER_PROFILE
    };

    let Some(parent) = target.parent() else {
        return EnsureResult::unchanged();
    };
    if std::fs::create_dir_all(parent).is_err() {
        return EnsureResult::unchanged();
    }

    match std::fs::write(&target, profile_content) {
        Ok(()) => {
            let mut result = EnsureResult::unchanged().with_extra("profile_path", target);
            result.changed = true;
            result
        }
        Err(_) => EnsureResult::unchanged(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_exe(temp: &Path) -> (PathBuf, PathBuf) {
        let dir = temp.join("Cemu");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("cemu.exe");
        std::fs::write(&exe, b"").unwrap();
        (exe, dir)
    }

    fn write_settings(dir: &Path, body: &str) -> PathBuf {
        let target = dir.join("portable").join("settings.xml");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, body).unwrap();
        target
    }

    // --- ensure_settings ---------------------------------------------------

    #[test]
    fn cemu_creates_the_default_settings_xml_when_missing() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());

        let result = ensure_settings(exe.to_str().unwrap(), false);

        assert!(result.changed);
        let target = dir.join("portable").join("settings.xml");
        assert_eq!(result.config_path, Some(target.clone()));
        let text = std::fs::read_to_string(&target).unwrap();
        assert_eq!(text, DEFAULT_CEMU_SETTINGS_XML, "byte-for-byte template");
    }

    #[test]
    fn cemu_enforces_the_six_forced_values_on_an_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        let target = write_settings(
            &dir,
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<content>\n<use_discord_presence>true</use_discord_presence>\n<check_update>true</check_update>\n<receive_untested_updates>true</receive_untested_updates>\n<gp_download>false</gp_download>\n<fullscreen>true</fullscreen>\n<window_maximized>false</window_maximized>\n</content>\n",
        );

        let result = ensure_settings(exe.to_str().unwrap(), false);

        assert!(result.changed);
        let text = std::fs::read_to_string(&target).unwrap();
        assert!(text.starts_with("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n"));
        assert!(text.contains("<use_discord_presence>false</use_discord_presence>"));
        assert!(text.contains("<check_update>false</check_update>"));
        assert!(text.contains("<receive_untested_updates>false</receive_untested_updates>"));
        assert!(text.contains("<gp_download>true</gp_download>"));
        assert!(text.contains("<fullscreen>false</fullscreen>"));
        assert!(text.contains("<window_maximized>true</window_maximized>"));
    }

    #[test]
    fn cemu_preserves_unmanaged_settings() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        write_settings(
            &dir,
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<content>\n<mlc_path>/some/path</mlc_path>\n</content>\n",
        );

        ensure_settings(exe.to_str().unwrap(), false);

        let text = std::fs::read_to_string(dir.join("portable").join("settings.xml")).unwrap();
        assert!(
            text.contains("<mlc_path>/some/path</mlc_path>"),
            "the sentinel element must survive byte-for-byte: {text}"
        );
    }

    #[test]
    fn cemu_no_op_when_all_six_already_match() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        let body = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<content>\n<use_discord_presence>false</use_discord_presence>\n<check_update>false</check_update>\n<receive_untested_updates>false</receive_untested_updates>\n<gp_download>true</gp_download>\n<fullscreen>false</fullscreen>\n<window_maximized>true</window_maximized>\n<Audio><api>3</api><TVVolume>100</TVVolume></Audio>\n</content>\n";
        let target = write_settings(&dir, body);

        let result = ensure_settings(exe.to_str().unwrap(), false);

        assert!(!result.changed);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), body);
    }

    #[test]
    fn cemu_malformed_xml_yields_unchanged() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        write_settings(&dir, "<not_content>oops</not_content>\n");

        let result = ensure_settings(exe.to_str().unwrap(), false);

        assert!(!result.changed);
        assert_eq!(result.config_path, None);
    }

    #[test]
    fn cemu_settings_xml_empty_after_trim_yields_unchanged() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        write_settings(&dir, "   \n  \n");

        let result = ensure_settings(exe.to_str().unwrap(), false);

        assert!(!result.changed);
        assert_eq!(result.config_path, None);
    }

    // --- renderer seed (spec 2026-09-17-vulkan-renderer-seed-design) --------

    /// Every forced element already at its value, so the ONLY thing a fresh
    /// call can still change is the renderer seed.
    const SIX_FORCED: &str = "<use_discord_presence>false</use_discord_presence>\n<check_update>false</check_update>\n<receive_untested_updates>false</receive_untested_updates>\n<gp_download>true</gp_download>\n<fullscreen>false</fullscreen>\n<window_maximized>true</window_maximized>\n";

    fn settled_body(extra: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<content>\n{SIX_FORCED}{extra}</content>\n"
        )
    }

    #[test]
    fn cemu_template_pins_the_vulkan_renderer() {
        assert!(DEFAULT_CEMU_SETTINGS_XML.contains("<api>1</api>"));
        assert!(!DEFAULT_CEMU_SETTINGS_XML.contains("<api>0</api>"));
        assert!(
            DEFAULT_CEMU_SETTINGS_XML.contains("<api>3</api>"),
            "the Audio api is a different element and stays 3"
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn cemu_fresh_install_seeds_vulkan_inside_an_existing_graphic_element() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        let target = write_settings(
            &dir,
            &settled_body(
                "<Graphic>\n<device>abc</device>\n</Graphic>\n<Audio><api>3</api></Audio>\n",
            ),
        );

        let result = ensure_settings(exe.to_str().unwrap(), true);

        assert!(result.changed);
        let text = std::fs::read_to_string(&target).unwrap();
        assert!(
            text.contains("<Graphic><api>1</api>\n<device>abc</device>"),
            "the api child goes first inside <Graphic>: {text}"
        );
        assert_eq!(text.matches("<api>1</api>").count(), 1, "{text}");
        assert!(
            text.contains("<Audio><api>3</api></Audio>"),
            "the Audio api is not the Graphic api: {text}"
        );
    }

    #[test]
    fn cemu_fresh_install_keeps_an_existing_graphic_api_value() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        let body = settled_body("<Graphic><api>0</api></Graphic>\n");
        let target = write_settings(&dir, &body);

        let result = ensure_settings(exe.to_str().unwrap(), true);

        assert!(!result.changed);
        let text = std::fs::read_to_string(&target).unwrap();
        assert!(text.contains("<api>0</api>"));
        assert!(!text.contains("<api>1</api>"));
        assert_eq!(text, body, "no write at all");
    }

    #[test]
    fn cemu_non_fresh_call_never_adds_the_graphic_api() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        let body = settled_body("<Graphic><device>abc</device></Graphic>\n");
        let target = write_settings(&dir, &body);

        let result = ensure_settings(exe.to_str().unwrap(), false);

        assert!(!result.changed);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), body);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn cemu_second_fresh_call_is_a_no_op() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        write_settings(
            &dir,
            &settled_body("<Graphic><device>abc</device></Graphic>\n"),
        );

        let first = ensure_settings(exe.to_str().unwrap(), true);
        let second = ensure_settings(exe.to_str().unwrap(), true);

        assert!(first.changed);
        assert!(!second.changed);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn cemu_fresh_install_creates_a_graphic_element_with_only_the_api_when_absent() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        let target = write_settings(&dir, &settled_body(""));

        let result = ensure_settings(exe.to_str().unwrap(), true);

        assert!(result.changed);
        let text = std::fs::read_to_string(&target).unwrap();
        assert!(
            text.contains("<Graphic><api>1</api></Graphic></content>"),
            "created right before the root close, api child only: {text}"
        );
        assert_eq!(text.matches("<Graphic>").count(), 1, "{text}");
        assert!(
            !text.contains("<device>"),
            "not the full template block: {text}"
        );
    }

    // --- comment-aware root-span scan (a decoy `<content>` inside a
    // pre-root comment must never be mistaken for the real root) ----------

    /// Pins the actual bug location: [`find_root_span`] must skip the
    /// `<content>`/`</content>` literals sitting inside the leading
    /// comment and land on the real root that follows it.
    #[test]
    fn cemu_root_span_skips_a_content_literal_inside_a_leading_comment() {
        let content = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!-- example: <content>fake</content> -->\n<content>\n<use_discord_presence>false</use_discord_presence>\n</content>\n";

        let (open, close_end) = find_root_span(content).expect("a real root exists");
        let span = &content[open..close_end];

        assert!(
            span.starts_with("<content>\n<use_discord_presence>"),
            "must land on the real root, not the decoy inside the comment: {span}"
        );
        assert!(
            !span.contains("fake") && !span.contains("-->"),
            "the comment's decoy tags must not leak into the root span: {span}"
        );
    }

    /// End-to-end: nothing needs to change, so [`ensure_settings`] never
    /// writes at all — the whole file, decoy comment included, must stay
    /// byte-for-byte untouched. Before the D11 comment-aware fix, the
    /// buggy scan would still misidentify the root and could report a
    /// spurious change.
    #[test]
    fn cemu_no_op_with_a_content_literal_inside_a_leading_comment_preserves_the_file() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        let body = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!-- example: <content>fake</content> -->\n<content>\n<use_discord_presence>false</use_discord_presence>\n<check_update>false</check_update>\n<receive_untested_updates>false</receive_untested_updates>\n<gp_download>true</gp_download>\n<fullscreen>false</fullscreen>\n<window_maximized>true</window_maximized>\n</content>\n";
        let target = write_settings(&dir, body);

        let result = ensure_settings(exe.to_str().unwrap(), false);

        assert!(!result.changed);
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            body,
            "the comment must be preserved byte-for-byte: no write ever happens"
        );
    }

    /// End-to-end: a real edit IS needed, so a write does happen. The
    /// output must be well-formed — exactly one real `<content>` open and
    /// one `</content>` close — and the forced element must be correctly
    /// updated, not corrupted by the decoy comment.
    #[test]
    fn cemu_edits_correctly_despite_a_content_literal_in_a_leading_comment() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());
        let body = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!-- example: <content>fake</content> -->\n<content>\n<use_discord_presence>true</use_discord_presence>\n<check_update>false</check_update>\n<receive_untested_updates>false</receive_untested_updates>\n<gp_download>true</gp_download>\n<fullscreen>false</fullscreen>\n<window_maximized>true</window_maximized>\n</content>\n";
        let target = write_settings(&dir, body);

        let result = ensure_settings(exe.to_str().unwrap(), false);

        assert!(result.changed);
        let text = std::fs::read_to_string(&target).unwrap();
        assert_eq!(
            text.matches("<content>").count(),
            1,
            "exactly one real root open tag: {text}"
        );
        assert_eq!(
            text.matches("</content>").count(),
            1,
            "exactly one real root close tag: {text}"
        );
        assert!(!text.contains("-->"), "no stray comment terminator: {text}");
        assert!(
            !text.contains("fake"),
            "the decoy text must not leak: {text}"
        );
        assert!(text.contains("<use_discord_presence>false</use_discord_presence>"));
    }

    // --- ensure_controller_config -------------------------------------------

    #[test]
    fn cemu_controller_profile_is_written_once_and_never_overwritten() {
        let temp = tempfile::tempdir().unwrap();

        let first = ensure_controller_config(temp.path().to_str().unwrap());
        let profile_path = temp
            .path()
            .join("portable")
            .join("controllerProfiles")
            .join("controller0.xml");
        assert!(first.changed);
        assert_eq!(first.extras.get("profile_path"), Some(&profile_path));
        let first_text = std::fs::read_to_string(&profile_path).unwrap();

        // Simulate a user hand-editing the profile between runs.
        std::fs::write(&profile_path, "<custom>true</custom>\n").unwrap();

        let second = ensure_controller_config(temp.path().to_str().unwrap());
        assert!(!second.changed);
        assert_eq!(second.extras.get("profile_path"), Some(&profile_path));
        assert_eq!(
            std::fs::read_to_string(&profile_path).unwrap(),
            "<custom>true</custom>\n",
            "an existing profile must never be overwritten"
        );
        assert_ne!(first_text, "<custom>true</custom>\n");
    }

    #[test]
    fn cemu_controller_profile_uses_sdl_off_windows() {
        let temp = tempfile::tempdir().unwrap();

        ensure_controller_config(temp.path().to_str().unwrap());

        let profile_path = temp
            .path()
            .join("portable")
            .join("controllerProfiles")
            .join("controller0.xml");
        let text = std::fs::read_to_string(&profile_path).unwrap();
        if is_windows_host() {
            assert!(text.contains("<api>XInput</api>"));
        } else {
            assert!(text.contains("<api>SDLController</api>"));
            assert!(!text.contains("<api>XInput</api>"));
        }
        assert!(text.contains("<type>Wii U Pro Controller</type>"));
    }

    // --- settings_path_candidates -------------------------------------------

    #[test]
    fn cemu_candidates_windows_uses_appdata_and_localappdata() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = crate::test_env::EnvGuard::set(&[
            (
                "APPDATA",
                Some(temp.path().join("appdata").to_str().unwrap()),
            ),
            (
                "LOCALAPPDATA",
                Some(temp.path().join("localappdata").to_str().unwrap()),
            ),
        ]);

        let candidates = settings_path_candidates_for("", true);

        assert!(candidates.contains(
            &temp
                .path()
                .join("appdata")
                .join("Cemu")
                .join("settings.xml")
        ));
        assert!(candidates.contains(
            &temp
                .path()
                .join("localappdata")
                .join("Cemu")
                .join("settings.xml")
        ));
    }

    #[test]
    fn cemu_candidates_non_windows_uses_xdg_config_home() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let xdg = temp.path().join("xdg-config");
        let _guard =
            crate::test_env::EnvGuard::set(&[("XDG_CONFIG_HOME", Some(xdg.to_str().unwrap()))]);

        let candidates = settings_path_candidates_for("", false);

        assert!(candidates.contains(&xdg.join("Cemu").join("settings.xml")));
    }

    #[test]
    fn cemu_candidates_include_portable_and_flat_paths_for_a_given_exe() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path());

        let candidates = settings_path_candidates(exe.to_str().unwrap());

        assert_eq!(candidates[0], dir.join("portable").join("settings.xml"));
        assert_eq!(candidates[1], dir.join("settings.xml"));
    }
}

//! Launch argument template construction: RetroArch core-argument path
//! derivation, POSIX-ish template splitting, placeholder validation and
//! substitution, and the RetroArch core-argument normalization pass that
//! resolves a relative core path against the emulator directory. Ports
//! `grid_launcher/emulator/launch.py:31-147,202-221`. See
//! `docs/porting/04-emulator-launch.md` §5 and §7.

use std::path::{Path, PathBuf};

const CORE_OPTION_TOKENS: [&str; 3] = ["-L", "--libretro", "--core"];

/// Placeholder values substituted into a launch argument template
/// (`launch_placeholders_for_game`, launch.py:79).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Placeholders {
    pub rom: String,
    pub core: String,
    pub ps3_launch_target: String,
}

/// The host OS string consumed by [`retroarch_core_argument_path`]'s
/// default extension selection: `"windows"`, `"macos"`, or `"linux"`
/// (mirrors `sys.platform` dispatch at launch.py:40).
pub fn host_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

/// Derives the RetroArch `-L` core-argument path from a configured core
/// value (`retroarch_core_argument_path`, launch.py:31):
///
/// - blank (after trim) → `""`;
/// - backslashes become forward slashes; if the result still contains `/`,
///   it is already a path and is returned as-is;
/// - otherwise the extension is chosen by `os` (`"windows"` → `.dll`,
///   `"macos"` → `.dylib`, else `.so`);
/// - a trailing `.dll`/`.dylib`/`.so` is stripped case-insensitively (first
///   match in that order wins);
/// - `_libretro` is appended unless the base already ends with it
///   (case-insensitive);
/// - the result is `cores/<base>_libretro<ext>`.
pub fn retroarch_core_argument_path(value: &str, os: &str) -> String {
    let core = value.trim();
    if core.is_empty() {
        return String::new();
    }

    let normalized = core.replace('\\', "/");
    if normalized.contains('/') {
        return normalized;
    }

    let extension = match os {
        "windows" => ".dll",
        "macos" => ".dylib",
        _ => ".so",
    };

    let mut base = normalized.as_str();
    for known in [".dll", ".dylib", ".so"] {
        if base.len() < known.len() {
            continue;
        }
        let split_at = base.len() - known.len();
        if base.is_char_boundary(split_at) && base[split_at..].eq_ignore_ascii_case(known) {
            base = &base[..split_at];
            break;
        }
    }

    let core_file = if base.to_lowercase().ends_with("_libretro") {
        format!("{base}{extension}")
    } else {
        format!("{base}_libretro{extension}")
    };
    format!("cores/{core_file}")
}

/// Trims whitespace, then drops one matching leading/trailing `"` or `'`
/// pair when the trimmed string is at least 2 characters
/// (`strip_wrapping_quotes`, launch.py:104).
fn strip_wrapping_quotes(token: &str) -> String {
    let trimmed = token.trim();
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() >= 2 {
        let first = chars[0];
        let last = chars[chars.len() - 1];
        if first == last && (first == '"' || first == '\'') {
            return chars[1..chars.len() - 1].iter().collect();
        }
    }
    trimmed.to_string()
}

/// POSIX-mode split, mirroring `shlex.split(template, posix=True)` closely
/// enough for launch templates: whitespace separates tokens; `'...'` is a
/// literal span (no escapes); inside `"..."`, `\"` and `\\` are the only
/// escapes (any other backslash is kept literally, along with the char it
/// precedes); outside quotes, a backslash escapes the following character.
/// An unterminated quote is reported as `Err`.
fn split_posix(template: &str) -> Result<Vec<String>, ()> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut chars = template.chars();

    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if in_token {
                    tokens.push(std::mem::take(&mut current));
                    in_token = false;
                }
            }
            '\'' => {
                in_token = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(ch) => current.push(ch),
                        None => return Err(()),
                    }
                }
            }
            '"' => {
                in_token = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some('"') => current.push('"'),
                            Some('\\') => current.push('\\'),
                            Some(other) => {
                                current.push('\\');
                                current.push(other);
                            }
                            None => return Err(()),
                        },
                        Some(ch) => current.push(ch),
                        None => return Err(()),
                    }
                }
            }
            '\\' => {
                in_token = true;
                match chars.next() {
                    Some(ch) => current.push(ch),
                    None => return Err(()),
                }
            }
            other => {
                in_token = true;
                current.push(other);
            }
        }
    }

    if in_token {
        tokens.push(current);
    }
    Ok(tokens)
}

/// Fallback splitter used when [`split_posix`] hits an unbalanced quote:
/// splits on whitespace but keeps `"..."`/`'...'` spans glued to whatever
/// precedes/follows them, with quote characters preserved verbatim and no
/// escape processing. An unterminated quote span simply runs to the end of
/// the string. This intentionally does not byte-match Python's
/// `shlex.split(..., posix=False)` fallback (which can itself raise on some
/// unbalanced-quote inputs) — this fallback always succeeds.
fn split_fallback(template: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut quote: Option<char> = None;

    for c in template.chars() {
        if let Some(q) = quote {
            current.push(c);
            if c == q {
                quote = None;
            }
            continue;
        }

        if c.is_whitespace() {
            if in_token {
                tokens.push(std::mem::take(&mut current));
                in_token = false;
            }
            continue;
        }

        in_token = true;
        current.push(c);
        if c == '"' || c == '\'' {
            quote = Some(c);
        }
    }

    if in_token {
        tokens.push(current);
    }
    tokens
}

/// Tokenizes a launch argument template (`split_launch_template_args`,
/// launch.py:130): a blank (after trim) template yields `[]`; otherwise
/// [`split_posix`] is tried first, falling back to [`split_fallback`] on an
/// unbalanced quote. The fallback never fails, so in practice this always
/// returns `Ok`; it keeps a `Result` signature to compose with the rest of
/// the template pipeline.
pub fn split_template(template: &str) -> Result<Vec<String>, String> {
    if template.trim().is_empty() {
        return Ok(Vec::new());
    }
    match split_posix(template) {
        Ok(tokens) => Ok(tokens),
        Err(()) => Ok(split_fallback(template)),
    }
}

/// Validates that every placeholder mentioned in `template` has a
/// non-blank value (`validate_launch_placeholders`, launch.py:140).
/// Returns the exact configured-emulator or configured-PS3-target message
/// as an `Err` when the corresponding placeholder is missing.
pub fn validate_placeholders(template: &str, ph: &Placeholders) -> Result<(), String> {
    if template.contains("%core%") && ph.core.trim().is_empty() {
        return Err(
            "No RetroArch core is configured for this platform. Set one in Emulators > Defaults."
                .to_string(),
        );
    }
    if template.contains("%ps3_launch_target%") && ph.ps3_launch_target.trim().is_empty() {
        return Err("No PS3 ISO or game ID was found for this game.".to_string());
    }
    Ok(())
}

/// Substitutes placeholders into each token and normalizes the result
/// (`apply_launch_placeholders_to_args`, launch.py:111):
///
/// - remembers, per token, whether the raw token contained `%core%`;
/// - replaces `%rom%`, `%core%`, `%ps3_launch_target%` in that order
///   (plain, unanchored replace);
/// - strips one wrapping quote pair via [`strip_wrapping_quotes`];
/// - if the token had `%core%` and the resolved core is blank, pops the
///   preceding `-L`/`--libretro`/`--core` argument (if any) and drops the
///   token instead of keeping it;
/// - drops any token that resolved to the empty string.
pub fn apply_placeholders(tokens: Vec<String>, ph: &Placeholders) -> Vec<String> {
    let core_missing = ph.core.trim().is_empty();
    let mut resolved_args: Vec<String> = Vec::with_capacity(tokens.len());

    for token in tokens {
        let had_core_placeholder = token.contains("%core%");
        let resolved = token
            .replace("%rom%", &ph.rom)
            .replace("%core%", &ph.core)
            .replace("%ps3_launch_target%", &ph.ps3_launch_target);
        let resolved = strip_wrapping_quotes(&resolved);

        if had_core_placeholder && core_missing {
            if resolved_args
                .last()
                .is_some_and(|last| CORE_OPTION_TOKENS.contains(&last.as_str()))
            {
                resolved_args.pop();
            }
            continue;
        }

        if !resolved.is_empty() {
            resolved_args.push(resolved);
        }
    }

    resolved_args
}

/// RetroArch-only post-pass (`normalized_retroarch_core_args`,
/// launch.py:202): for every element except the last that equals `-L`,
/// `--libretro`, or `--core`, if the following token is non-blank,
/// relative, and `emulator_dir.join(token)` exists as a file, rewrites it
/// to the canonicalized absolute path. Absolute paths and candidates that
/// don't resolve to an existing file are left untouched.
pub fn normalized_retroarch_core_args(emulator_dir: &Path, args: Vec<String>) -> Vec<String> {
    let mut normalized = args;
    if normalized.is_empty() {
        return normalized;
    }

    let last_index = normalized.len() - 1;
    for index in 0..last_index {
        if !CORE_OPTION_TOKENS.contains(&normalized[index].as_str()) {
            continue;
        }

        let core_token = normalized[index + 1].trim().to_string();
        if core_token.is_empty() {
            continue;
        }

        let core_path = PathBuf::from(&core_token);
        if core_path.is_absolute() {
            continue;
        }

        let candidate = emulator_dir.join(&core_path);
        if !candidate.is_file() {
            continue;
        }

        if let Ok(resolved) = std::fs::canonicalize(&candidate) {
            normalized[index + 1] = resolved.to_string_lossy().into_owned();
        }
    }

    normalized
}

/// Builds the resolved argv for a launch (`resolve_launch_arguments_for_game`,
/// launch.py:150, steps 2-6): the template is `entry_args.trim()` when
/// non-empty, else `"%rom%"`; `global_launch_args.trim()` is appended with a
/// single space when non-blank. The combined template is split, validated,
/// then substituted. The `Err` strings are the two verbatim messages from
/// [`validate_placeholders`] (the spawn layer wraps them as
/// `"Invalid launch arguments: <e>"`).
pub fn build_args(
    entry_args: &str,
    global_launch_args: &str,
    ph: &Placeholders,
) -> Result<Vec<String>, String> {
    let entry_trimmed = entry_args.trim();
    let base = if entry_trimmed.is_empty() {
        "%rom%"
    } else {
        entry_trimmed
    };

    let global_trimmed = global_launch_args.trim();
    let template = if global_trimmed.is_empty() {
        base.to_string()
    } else {
        format!("{base} {global_trimmed}")
    };

    let tokens = split_template(&template)?;
    validate_placeholders(&template, ph)?;
    Ok(apply_placeholders(tokens, ph))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn placeholders(rom: &str, core: &str, ps3: &str) -> Placeholders {
        Placeholders {
            rom: rom.to_string(),
            core: core.to_string(),
            ps3_launch_target: ps3.to_string(),
        }
    }

    // --- host_os -----------------------------------------------------------

    #[test]
    fn host_os_is_one_of_the_known_values() {
        assert!(["windows", "macos", "linux"].contains(&host_os()));
    }

    // --- retroarch_core_argument_path --------------------------------------

    #[test]
    fn core_path_blank_is_empty() {
        assert_eq!(retroarch_core_argument_path("   ", "linux"), "");
    }

    #[test]
    fn core_path_windows_extension() {
        assert_eq!(
            retroarch_core_argument_path("snes9x", "windows"),
            "cores/snes9x_libretro.dll"
        );
    }

    #[test]
    fn core_path_macos_extension() {
        assert_eq!(
            retroarch_core_argument_path("snes9x", "macos"),
            "cores/snes9x_libretro.dylib"
        );
    }

    #[test]
    fn core_path_linux_extension() {
        assert_eq!(
            retroarch_core_argument_path("snes9x", "linux"),
            "cores/snes9x_libretro.so"
        );
    }

    #[test]
    fn core_path_slash_passthrough_after_backslash_normalization() {
        assert_eq!(
            retroarch_core_argument_path("cores\\snes9x_libretro.so", "linux"),
            "cores/snes9x_libretro.so"
        );
    }

    #[test]
    fn core_path_libretro_suffix_not_doubled() {
        assert_eq!(
            retroarch_core_argument_path("snes9x_libretro", "linux"),
            "cores/snes9x_libretro.so"
        );
    }

    #[test]
    fn core_path_strips_dll_case_insensitively() {
        assert_eq!(
            retroarch_core_argument_path("snes9x.DLL", "windows"),
            "cores/snes9x_libretro.dll"
        );
    }

    // --- split_template ------------------------------------------------------

    #[test]
    fn split_blank_template_is_empty() {
        assert_eq!(split_template("   ").unwrap(), Vec::<String>::new());
        assert_eq!(split_template("").unwrap(), Vec::<String>::new());
    }

    #[test]
    fn split_handles_single_and_double_quotes() {
        assert_eq!(
            split_template("-L \"%core%\" 'single quoted'").unwrap(),
            vec!["-L", "%core%", "single quoted"]
        );
    }

    #[test]
    fn split_falls_back_on_unbalanced_quote() {
        // Pinned per the task brief: the fallback keeps the quote span
        // glued to what precedes it, quote char preserved, no escape
        // processing, running to end of string.
        assert_eq!(
            split_template("-fullscreen \"unclosed").unwrap(),
            vec!["-fullscreen", "\"unclosed"]
        );
    }

    // --- validate_placeholders -----------------------------------------------

    #[test]
    fn validate_missing_core_message_is_verbatim() {
        let ph = placeholders("%rom%", "", "");
        let err = validate_placeholders("-L %core% %rom%", &ph).unwrap_err();
        assert_eq!(
            err,
            "No RetroArch core is configured for this platform. Set one in Emulators > Defaults."
        );
    }

    #[test]
    fn validate_missing_ps3_target_message_is_verbatim() {
        let ph = placeholders("/roms/game.iso", "", "");
        let err = validate_placeholders("%ps3_launch_target%", &ph).unwrap_err();
        assert_eq!(err, "No PS3 ISO or game ID was found for this game.");
    }

    #[test]
    fn validate_passes_when_all_mentioned_placeholders_are_present() {
        let ph = placeholders("/roms/game.zip", "cores/snes9x_libretro.so", "");
        assert!(validate_placeholders("-L %core% %rom%", &ph).is_ok());
    }

    // --- apply_placeholders ---------------------------------------------------

    #[test]
    fn apply_substitutes_and_strips_wrapping_quotes() {
        let ph = placeholders("/roms/game.zip", "cores/snes9x_libretro.so", "");
        let tokens = vec!["\"%core%\"".to_string(), "%rom%".to_string()];
        let result = apply_placeholders(tokens, &ph);
        assert_eq!(result, vec!["cores/snes9x_libretro.so", "/roms/game.zip"]);
    }

    #[test]
    fn apply_drops_tokens_that_resolve_to_empty() {
        let ph = placeholders("/roms/game.zip", "cores/snes9x_libretro.so", "");
        let tokens = vec!["".to_string(), "%rom%".to_string()];
        let result = apply_placeholders(tokens, &ph);
        assert_eq!(result, vec!["/roms/game.zip"]);
    }

    #[test]
    fn apply_pops_preceding_core_flag_when_core_is_blank() {
        let ph = placeholders("/roms/game.zip", "", "");
        let tokens = vec!["-L".to_string(), "%core%".to_string(), "%rom%".to_string()];
        // Validation would normally have caught this; apply_placeholders
        // implements the cleanup anyway for callers that skip validation.
        let result = apply_placeholders(tokens, &ph);
        assert_eq!(result, vec!["/roms/game.zip"]);
    }

    // --- normalized_retroarch_core_args ---------------------------------------

    #[test]
    fn normalize_rewrites_relative_core_to_absolute() {
        let dir = tempfile::tempdir().unwrap();
        let cores_dir = dir.path().join("cores");
        fs::create_dir_all(&cores_dir).unwrap();
        let core_file = cores_dir.join("snes9x_libretro.so");
        fs::write(&core_file, b"core bytes").unwrap();

        let args = vec![
            "-L".to_string(),
            "cores/snes9x_libretro.so".to_string(),
            "%rom%".to_string(),
        ];
        let result = normalized_retroarch_core_args(dir.path(), args);
        let expected = fs::canonicalize(&core_file).unwrap();
        assert_eq!(result[1], expected.to_string_lossy());
    }

    #[test]
    fn normalize_leaves_absolute_core_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let args = vec!["--core".to_string(), "/opt/cores/snes9x.so".to_string()];
        let result = normalized_retroarch_core_args(dir.path(), args.clone());
        assert_eq!(result, args);
    }

    #[test]
    fn normalize_ignores_flag_in_last_position() {
        let dir = tempfile::tempdir().unwrap();
        let args = vec!["--libretro".to_string()];
        let result = normalized_retroarch_core_args(dir.path(), args.clone());
        assert_eq!(result, args);
    }

    #[test]
    fn normalize_leaves_missing_file_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let args = vec![
            "-L".to_string(),
            "cores/does_not_exist.so".to_string(),
            "%rom%".to_string(),
        ];
        let result = normalized_retroarch_core_args(dir.path(), args.clone());
        assert_eq!(result, args);
    }

    // --- build_args -------------------------------------------------------

    #[test]
    fn build_args_end_to_end_with_real_looking_core() {
        let ph = placeholders(
            "/roms/Super Game.zip",
            "cores/snes9x_libretro.so",
            String::new().as_str(),
        );
        let result = build_args("-L \"%core%\" \"%rom%\"", "", &ph).unwrap();
        assert_eq!(
            result,
            vec!["-L", "cores/snes9x_libretro.so", "/roms/Super Game.zip"]
        );
    }

    #[test]
    fn build_args_blank_entry_defaults_to_rom_and_appends_global() {
        let ph = placeholders("/roms/game.zip", "", "");
        let result = build_args("   ", "-fullscreen", &ph).unwrap();
        assert_eq!(result, vec!["/roms/game.zip", "-fullscreen"]);
    }

    #[test]
    fn build_args_propagates_validation_error() {
        let ph = placeholders("/roms/game.zip", "", "");
        let err = build_args("-L %core% %rom%", "", &ph).unwrap_err();
        assert_eq!(
            err,
            "No RetroArch core is configured for this platform. Set one in Emulators > Defaults."
        );
    }
}

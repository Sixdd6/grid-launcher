//! TV mode's native half: the UI mode and fullscreen, the controller list,
//! "Back to the game", Quit GRID, the folder browser's listing and the
//! clipboard read for Connect (TV mode plan § 1.3).

use super::AppState;
use crate::gamepad::identity::PadInfo;
use grid_core::fsbrowse::FolderListing;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

/// Which interface is on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiMode {
    Desktop,
    Tv,
}

/// The current mode. Never persisted (TV mode README § Decided "Entering
/// and leaving"): GRID starts in desktop mode unless started with `--tv`.
pub struct UiModeState {
    mode: Mutex<UiMode>,
    started_with_tv: bool,
}

impl UiModeState {
    pub fn new(started_with_tv: bool) -> Self {
        Self {
            mode: Mutex::new(UiMode::Desktop),
            started_with_tv,
        }
    }

    pub fn mode(&self) -> UiMode {
        *self.mode.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn set(&self, mode: UiMode) {
        *self.mode.lock().unwrap_or_else(|e| e.into_inner()) = mode;
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UiModeInfo {
    pub mode: UiMode,
    /// GRID was started with `--tv`.
    pub started_with_tv: bool,
    /// The main window is fullscreen right now.
    pub fullscreen: bool,
}

fn main_window(app: &AppHandle) -> Result<tauri::WebviewWindow, String> {
    app.get_webview_window("main")
        .ok_or_else(|| "the main window is gone".to_string())
}

/// Switches the interface. TV: the window goes fullscreen and the controller
/// reader starts and opens its gate. Desktop: the gate closes
/// (`released_all`) and the window leaves fullscreen, back to the geometry it
/// had. The window-state plugin never saves or restores fullscreen
/// (`lib.rs`), and GRID never exits while fullscreen ([`exit_app`]), so a
/// later desktop start opens at the desktop geometry.
pub fn apply_ui_mode(app: &AppHandle, state: &AppState, mode: UiMode) -> Result<(), String> {
    let window = main_window(app)?;
    match mode {
        UiMode::Tv => {
            state.ui_mode.set(UiMode::Tv);
            window.set_fullscreen(true).map_err(|e| e.to_string())?;
            state.gamepad.ensure_started();
            state.gamepad.set_tv_active(true);
        }
        UiMode::Desktop => {
            state.gamepad.set_tv_active(false);
            state.ui_mode.set(UiMode::Desktop);
            window.set_fullscreen(false).map_err(|e| e.to_string())?;
        }
    }
    tracing::info!(?mode, "ui mode");
    Ok(())
}

#[tauri::command]
pub fn get_ui_mode(app: AppHandle, state: State<'_, AppState>) -> UiModeInfo {
    UiModeInfo {
        mode: state.ui_mode.mode(),
        started_with_tv: state.ui_mode.started_with_tv,
        fullscreen: main_window(&app)
            .and_then(|w| w.is_fullscreen().map_err(|e| e.to_string()))
            .unwrap_or(false),
    }
}

#[tauri::command]
pub fn set_ui_mode(app: AppHandle, state: State<'_, AppState>, mode: UiMode) -> Result<(), String> {
    apply_ui_mode(&app, &state, mode)
}

/// The controllers connected right now (the reader's list; empty until TV
/// mode first turns on, since the reader starts then).
#[tauri::command]
pub fn gamepad_list(state: State<'_, AppState>) -> Vec<PadInfo> {
    state.gamepad.pads()
}

/// Playing › Back to the game: minimizes GRID so the game's window is in
/// front. GRID comes back by itself when the last game ends (`lib.rs`).
#[tauri::command]
pub fn tv_yield_to_game(app: AppHandle) -> Result<(), String> {
    main_window(&app)?.minimize().map_err(|e| e.to_string())
}

/// Menu › Quit GRID: the same exit the window's close button makes.
#[tauri::command]
pub fn quit_app(app: AppHandle, state: State<'_, AppState>) {
    exit_app(&app, state.ui_mode.mode());
}

/// How long [`exit_app`] waits for the window to leave fullscreen.
const LEAVE_FULLSCREEN_WAIT: std::time::Duration = std::time::Duration::from_millis(1500);
/// After it has left, how long the resize and move events get to reach the
/// window-state plugin's cache.
const SETTLE: std::time::Duration = std::time::Duration::from_millis(200);

/// Exits GRID. From TV mode the window first leaves fullscreen, so the
/// window-state plugin's exit save records the desktop geometry, not the
/// screen's (TV mode plan, risk R2); the plugin saves the size it sees on
/// resize and on exit, fullscreen or not.
pub fn exit_app(app: &AppHandle, mode: UiMode) {
    if mode == UiMode::Desktop {
        app.exit(0);
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        if let Ok(window) = main_window(&app) {
            let _ = window.set_fullscreen(false);
            let start = std::time::Instant::now();
            while window.is_fullscreen().unwrap_or(false) && start.elapsed() < LEAVE_FULLSCREEN_WAIT
            {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            std::thread::sleep(SETTLE);
        }
        app.exit(0);
    });
}

/// One folder for TV mode's folder browser. An empty `path` starts in the
/// home folder. `files`: list files too (picking a program or a game file).
#[tauri::command]
pub async fn list_folder(path: String, files: bool) -> Result<FolderListing, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let start = if path.trim().is_empty() {
            home_dir().ok_or_else(|| "no home folder".to_string())?
        } else {
            PathBuf::from(path)
        };
        grid_core::fsbrowse::list_folder(&start, files)
            .map_err(|e| format!("could not open {}: {e}", start.display()))
    })
    .await
    .map_err(|e| format!("list_folder did not finish: {e}"))?
}

fn home_dir() -> Option<PathBuf> {
    directories::UserDirs::new().map(|dirs| dirs.home_dir().to_path_buf())
}

/// Connect's View-to-paste: the clipboard's text, straight into the field.
/// It may be a token, so it is never logged; an empty or non-text clipboard
/// is an empty string.
#[tauri::command]
pub fn read_clipboard_text(app: AppHandle) -> String {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard().read_text().unwrap_or_default()
}

/// Whether the clipboard holds text the on-screen keyboard can paste, so its
/// R3 Paste hint can grey out when there is none. Only a yes or no crosses
/// IPC: the text itself (maybe a token) stays here until a paste asks for it.
#[tauri::command]
pub fn clipboard_has_text(app: AppHandle) -> bool {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard().read_text().is_ok_and(|t| pasteable(&t))
}

/// Text a paste would put in a one-line field: anything but line breaks and
/// other control characters (the keyboard drops those; `osk.ts` `pasteText`).
fn pasteable(text: &str) -> bool {
    text.chars()
        .any(|c| !c.is_control() && c != '\u{2028}' && c != '\u{2029}')
}

/// e2e only: puts text on the clipboard, so a spec can check View-to-paste
/// on Connect (the specs seed a non-secret server address).
#[cfg(feature = "e2e")]
#[tauri::command]
pub fn e2e_clipboard_write(app: AppHandle, text: String) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

/// e2e only: feeds synthetic controller input through the same mapping and
/// gate a real controller goes through (`gamepad::GamepadInput`'s wire format).
#[cfg(feature = "e2e")]
#[tauri::command]
pub fn e2e_gamepad_inject(state: State<'_, AppState>, events: Vec<crate::gamepad::GamepadInput>) {
    state.gamepad.apply(events);
}

#[cfg(test)]
mod tests {
    use super::pasteable;

    #[test]
    fn pasteable_text_is_anything_but_line_breaks_and_controls() {
        assert!(pasteable("https://romm.local\n"));
        assert!(pasteable(" "));
        assert!(!pasteable(""));
        assert!(!pasteable("\r\n\t"));
        assert!(!pasteable("\u{2028}\u{2029}\u{7f}"));
    }
}

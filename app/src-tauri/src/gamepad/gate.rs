//! When controller input reaches the UI. Pure.
//!
//! TV mode reads the controller only while TV mode is on and GRID's window
//! has focus (TV mode README § Decided Implementation, 2026-11-10). A running
//! game doesn't close it: while GRID's window is focused, presses reach the
//! Playing page (Back to the game, Stop game); while the game's window has
//! focus, GRID's doesn't, and the gate is closed. Closing the gate sends
//! `released_all`, so no held button or repeat survives into the game or
//! another window.

/// The gate's inputs. It is open while TV mode is on and the window is
/// focused; `games_running` is tracked beside them (GRID comes back to the
/// front when the last game ends) but doesn't close it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gate {
    pub tv_active: bool,
    pub window_focused: bool,
    pub games_running: bool,
}

impl Default for Gate {
    /// GRID starts in desktop mode with its window focused and no game.
    fn default() -> Self {
        Self {
            tv_active: false,
            window_focused: true,
            games_running: false,
        }
    }
}

/// What one change did to the gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    None,
    Opened,
    /// Open → closed: the caller sends `released_all`.
    Closed,
}

impl Gate {
    pub fn open(&self) -> bool {
        self.tv_active && self.window_focused
    }

    /// Applies `change` and reports whether the gate opened or closed.
    pub fn update(&mut self, change: impl FnOnce(&mut Gate)) -> Transition {
        let before = self.open();
        change(self);
        match (before, self.open()) {
            (true, false) => Transition::Closed,
            (false, true) => Transition::Opened,
            _ => Transition::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_gate() -> Gate {
        Gate {
            tv_active: true,
            window_focused: true,
            games_running: false,
        }
    }

    #[test]
    fn it_starts_closed_because_grid_starts_in_desktop_mode() {
        assert!(!Gate::default().open());
        let mut gate = Gate::default();
        assert_eq!(gate.update(|g| g.tv_active = true), Transition::Opened);
    }

    #[test]
    fn tv_mode_off_or_losing_focus_closes_it() {
        let changes: [fn(&mut Gate); 2] = [|g| g.tv_active = false, |g| g.window_focused = false];
        for change in changes {
            let mut gate = open_gate();
            assert_eq!(gate.update(change), Transition::Closed);
            assert!(!gate.open());
        }
    }

    #[test]
    fn a_running_game_leaves_it_open_while_grid_is_focused() {
        let mut gate = open_gate();
        assert_eq!(gate.update(|g| g.games_running = true), Transition::None);
        assert!(gate.open());
    }

    #[test]
    fn it_closes_while_a_game_runs_and_grid_is_unfocused() {
        let mut gate = open_gate();
        gate.update(|g| g.games_running = true);
        assert_eq!(
            gate.update(|g| g.window_focused = false),
            Transition::Closed
        );
        assert!(!gate.open());
        // GRID focused again with the game still running: Playing listens.
        assert_eq!(gate.update(|g| g.window_focused = true), Transition::Opened);
        assert!(gate.open());
    }

    #[test]
    fn it_stays_closed_in_desktop_mode_whatever_else_holds() {
        let mut gate = Gate::default();
        assert_eq!(gate.update(|g| g.games_running = true), Transition::None);
        assert_eq!(gate.update(|g| g.window_focused = true), Transition::None);
        assert!(!gate.open());
    }

    #[test]
    fn closing_an_already_closed_gate_is_no_transition() {
        let mut gate = open_gate();
        gate.update(|g| g.window_focused = false);
        assert_eq!(gate.update(|g| g.tv_active = false), Transition::None);
        assert_eq!(gate.update(|g| g.games_running = false), Transition::None);
    }
}

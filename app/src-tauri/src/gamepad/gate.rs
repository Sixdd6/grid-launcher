//! When controller input reaches the UI. Pure.
//!
//! TV mode reads the controller only while TV mode is on, GRID's window has
//! focus and no game is running (TV mode README § Decided Implementation;
//! § Pages › Playing). Closing the gate sends `released_all`, so no held
//! button or repeat survives into the game or another window.

/// The three conditions; the gate is open only when all hold.
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
        self.tv_active && self.window_focused && !self.games_running
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
    fn each_condition_closes_it() {
        let changes: [fn(&mut Gate); 3] = [
            |g| g.tv_active = false,
            |g| g.window_focused = false,
            |g| g.games_running = true,
        ];
        for change in changes {
            let mut gate = open_gate();
            assert_eq!(gate.update(change), Transition::Closed);
            assert!(!gate.open());
        }
    }

    #[test]
    fn closing_an_already_closed_gate_is_no_transition() {
        let mut gate = open_gate();
        gate.update(|g| g.games_running = true);
        assert_eq!(gate.update(|g| g.window_focused = false), Transition::None);
        assert_eq!(gate.update(|g| g.window_focused = true), Transition::None);
    }

    #[test]
    fn it_reopens_when_the_game_ends() {
        let mut gate = open_gate();
        assert_eq!(gate.update(|g| g.games_running = true), Transition::Closed);
        assert_eq!(gate.update(|g| g.games_running = false), Transition::Opened);
        assert!(gate.open());
    }

    #[test]
    fn it_stays_closed_while_any_condition_fails() {
        let mut gate = open_gate();
        gate.update(|g| g.games_running = true);
        gate.update(|g| g.window_focused = false);
        assert_eq!(gate.update(|g| g.games_running = false), Transition::None);
        assert!(!gate.open());
        assert_eq!(gate.update(|g| g.window_focused = true), Transition::Opened);
    }
}

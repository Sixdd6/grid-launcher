use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Button {
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    South, // accept
    East,  // back
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    LeftStickX,
    LeftStickY,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NavAction {
    Up,
    Down,
    Left,
    Right,
    Accept,
    Back,
}

struct AxisState {
    direction: i8, // -1, 0, 1 after dead zone
    last_emit: Instant,
}

pub struct Mapper {
    dead_zone: f32,
    repeat: Duration,
    axes: std::collections::HashMap<Axis, AxisState>,
}

impl Mapper {
    pub fn new(dead_zone: f32, repeat: Duration) -> Self {
        Self {
            dead_zone,
            repeat,
            axes: Default::default(),
        }
    }

    pub fn button(&mut self, btn: Button, pressed: bool, _now: Instant) -> Option<NavAction> {
        if !pressed {
            return None;
        }
        Some(match btn {
            Button::DpadUp => NavAction::Up,
            Button::DpadDown => NavAction::Down,
            Button::DpadLeft => NavAction::Left,
            Button::DpadRight => NavAction::Right,
            Button::South => NavAction::Accept,
            Button::East => NavAction::Back,
        })
    }

    pub fn axis(&mut self, axis: Axis, value: f32, now: Instant) -> Option<NavAction> {
        let direction = if value > self.dead_zone {
            1
        } else if value < -self.dead_zone {
            -1
        } else {
            0
        };
        let state = self.axes.entry(axis).or_insert(AxisState {
            direction: 0,
            last_emit: now - self.repeat,
        });
        let changed = state.direction != direction;
        state.direction = direction;
        if direction == 0 {
            state.last_emit = now - self.repeat; // reset: next push fires immediately
            return None;
        }
        if !changed && now.duration_since(state.last_emit) < self.repeat {
            return None;
        }
        state.last_emit = now;
        Some(match (axis, direction) {
            (Axis::LeftStickX, 1) => NavAction::Right,
            (Axis::LeftStickX, _) => NavAction::Left,
            // Stick Y is positive-up in gilrs; the thread normalizes if needed.
            (Axis::LeftStickY, 1) => NavAction::Up,
            (Axis::LeftStickY, _) => NavAction::Down,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn mapper() -> Mapper {
        Mapper::new(0.3, Duration::from_millis(200))
    }

    #[test]
    fn dpad_press_maps_and_release_does_not() {
        let mut m = mapper();
        let t = Instant::now();
        assert_eq!(m.button(Button::DpadUp, true, t), Some(NavAction::Up));
        assert_eq!(m.button(Button::DpadUp, false, t), None);
        assert_eq!(m.button(Button::South, true, t), Some(NavAction::Accept));
        assert_eq!(m.button(Button::East, true, t), Some(NavAction::Back));
    }

    #[test]
    fn axis_respects_dead_zone() {
        let mut m = mapper();
        let t = Instant::now();
        assert_eq!(m.axis(Axis::LeftStickY, 0.2, t), None);
        assert_eq!(m.axis(Axis::LeftStickY, 0.9, t), Some(NavAction::Up));
    }

    #[test]
    fn held_axis_repeats_at_interval() {
        let mut m = mapper();
        let t0 = Instant::now();
        assert_eq!(m.axis(Axis::LeftStickX, 1.0, t0), Some(NavAction::Right));
        // Held below the repeat interval: no event.
        assert_eq!(
            m.axis(Axis::LeftStickX, 1.0, t0 + Duration::from_millis(100)),
            None
        );
        // Past the interval: repeat fires.
        assert_eq!(
            m.axis(Axis::LeftStickX, 1.0, t0 + Duration::from_millis(210)),
            Some(NavAction::Right)
        );
        // Returning to center resets so the next push fires immediately.
        assert_eq!(
            m.axis(Axis::LeftStickX, 0.0, t0 + Duration::from_millis(220)),
            None
        );
        assert_eq!(
            m.axis(Axis::LeftStickX, -1.0, t0 + Duration::from_millis(230)),
            Some(NavAction::Left)
        );
    }
}

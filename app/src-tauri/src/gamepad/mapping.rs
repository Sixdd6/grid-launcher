//! Raw controller input → the `gamepad` events TV mode consumes. Pure: the
//! poll thread converts gilrs events into [`RawEvent`]s (`super::from_gilrs`),
//! and the e2e injector deserializes the same shape, so both go through
//! [`Mapper`] and the gate alike.
//!
//! Buttons are reported by their physical position (`south`, `east`, …), not
//! by meaning: Button assignments (Xbox or Nintendo style, per controller)
//! are applied by the UI. The Guide / Home button is reported like any other
//! and bound to nothing (TV mode README, Consultant decisions).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Every button TV mode reads, by physical position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PadButton {
    South,
    East,
    West,
    North,
    LeftBumper,
    RightBumper,
    /// LT: analog on most pads, thresholded at [`TRIGGER_THRESHOLD`].
    LeftTrigger,
    /// RT: analog on most pads, thresholded at [`TRIGGER_THRESHOLD`].
    RightTrigger,
    /// View (Back / Select / Create / −).
    Select,
    /// Menu (Start / Options / +).
    Start,
    /// Home / Guide: reported, never bound.
    Guide,
    LeftStick,
    RightStick,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Left,
    Right,
}

/// The axes TV mode reads. Stick Y is positive up (gilrs' convention).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PadAxis {
    LeftStickX,
    LeftStickY,
    RightStickX,
    RightStickY,
    /// A hat reported as an axis: −1 left, +1 right.
    DpadX,
    /// A hat reported as an axis: +1 up, −1 down.
    DpadY,
}

/// One input from one controller, before mapping: the gilrs event shape,
/// reduced to what TV mode reads. Also the e2e injector's wire format, e.g.
/// `{ "kind": "pressed", "button": "south" }` or
/// `{ "kind": "axis", "axis": "left_stick_x", "value": 0.8 }`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RawEvent {
    Pressed {
        button: PadButton,
    },
    Released {
        button: PadButton,
    },
    /// A button's analog value, 0–1 (gilrs `ButtonChanged`). Only the two
    /// triggers act on it.
    Changed {
        button: PadButton,
        value: f32,
    },
    Axis {
        axis: PadAxis,
        value: f32,
    },
}

/// A stick or trigger value must move at least this far before it is sent
/// again.
pub const ANALOG_STEP: f32 = 0.02;
/// An analog trigger counts as pressed at or past this value.
pub const TRIGGER_THRESHOLD: f32 = 0.5;
/// A hat reported as an axis counts as pressed at or past this magnitude.
const HAT_THRESHOLD: f32 = 0.5;

/// What the reader sends the UI (event `gamepad`, tagged by `kind`). The
/// `connected` / `disconnected` / `released_all` events are built by the
/// service; the mapper produces `button`, `stick` and `trigger`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PadEvent {
    Connected {
        pad: super::identity::PadInfo,
    },
    Disconnected {
        id: u32,
    },
    Button {
        id: u32,
        button: PadButton,
        down: bool,
    },
    /// Both axes of one stick, −1…1, Y positive up.
    Stick {
        id: u32,
        stick: Side,
        x: f32,
        y: f32,
    },
    /// An analog trigger's value, 0…1 (its `button` down / up is sent too).
    Trigger {
        id: u32,
        trigger: Side,
        value: f32,
    },
    /// The gate closed: every held button counts as released, so no repeat
    /// survives a game starting or the window losing focus.
    ReleasedAll,
}

#[derive(Debug, Default, Clone)]
struct PadState {
    down: BTreeSet<PadButton>,
    /// Current stick values (x, y), and the last ones sent.
    sticks: BTreeMap<Side, (f32, f32)>,
    sent_sticks: BTreeMap<Side, (f32, f32)>,
    triggers: BTreeMap<Side, f32>,
    sent_triggers: BTreeMap<Side, f32>,
}

/// Turns raw events into button, stick and trigger events. Buttons go out at
/// once (repeats are not sent: holding is the UI's to time). Sticks and
/// triggers are collected and sent by [`Mapper::flush`], at most once per
/// poll tick each, and only after moving by [`ANALOG_STEP`].
#[derive(Debug, Default)]
pub struct Mapper {
    pads: BTreeMap<u32, PadState>,
}

impl Mapper {
    /// Maps one raw event; returns the button events it caused.
    pub fn feed(&mut self, id: u32, raw: RawEvent) -> Vec<PadEvent> {
        let pad = self.pads.entry(id).or_default();
        let mut out = Vec::new();
        match raw {
            RawEvent::Pressed { button } => match trigger_side(button) {
                // A digital trigger, or gilrs' own threshold: the analog
                // value decides, so take a press as fully pulled.
                Some(side) => set_trigger(pad, id, side, 1.0, &mut out),
                None => set_button(pad, id, button, true, &mut out),
            },
            RawEvent::Released { button } => match trigger_side(button) {
                Some(side) => set_trigger(pad, id, side, 0.0, &mut out),
                None => set_button(pad, id, button, false, &mut out),
            },
            RawEvent::Changed { button, value } => {
                if let Some(side) = trigger_side(button) {
                    set_trigger(pad, id, side, value.clamp(0.0, 1.0), &mut out);
                }
            }
            RawEvent::Axis { axis, value } => {
                let value = value.clamp(-1.0, 1.0);
                match axis {
                    PadAxis::LeftStickX => pad.sticks.entry(Side::Left).or_default().0 = value,
                    PadAxis::LeftStickY => pad.sticks.entry(Side::Left).or_default().1 = value,
                    PadAxis::RightStickX => pad.sticks.entry(Side::Right).or_default().0 = value,
                    PadAxis::RightStickY => pad.sticks.entry(Side::Right).or_default().1 = value,
                    PadAxis::DpadX => {
                        set_button(
                            pad,
                            id,
                            PadButton::DpadLeft,
                            value <= -HAT_THRESHOLD,
                            &mut out,
                        );
                        set_button(
                            pad,
                            id,
                            PadButton::DpadRight,
                            value >= HAT_THRESHOLD,
                            &mut out,
                        );
                    }
                    PadAxis::DpadY => {
                        set_button(pad, id, PadButton::DpadUp, value >= HAT_THRESHOLD, &mut out);
                        set_button(
                            pad,
                            id,
                            PadButton::DpadDown,
                            value <= -HAT_THRESHOLD,
                            &mut out,
                        );
                    }
                }
            }
        }
        out
    }

    /// The stick and trigger values that moved since they were last sent.
    pub fn flush(&mut self) -> Vec<PadEvent> {
        let mut out = Vec::new();
        for (&id, pad) in &mut self.pads {
            for (&side, &(x, y)) in &pad.sticks {
                let sent = pad.sent_sticks.get(&side).copied().unwrap_or((0.0, 0.0));
                if (x - sent.0).abs() >= ANALOG_STEP || (y - sent.1).abs() >= ANALOG_STEP {
                    pad.sent_sticks.insert(side, (x, y));
                    out.push(PadEvent::Stick {
                        id,
                        stick: side,
                        x,
                        y,
                    });
                }
            }
            for (&side, &value) in &pad.triggers {
                let sent = pad.sent_triggers.get(&side).copied().unwrap_or(0.0);
                // A release to exactly 0 is always sent, however small the
                // step, so a trigger never rests "slightly pulled".
                let to_rest = value == 0.0 && sent != 0.0;
                if (value - sent).abs() >= ANALOG_STEP || to_rest {
                    pad.sent_triggers.insert(side, value);
                    out.push(PadEvent::Trigger {
                        id,
                        trigger: side,
                        value,
                    });
                }
            }
        }
        out
    }

    /// Forgets every held button and analog value (the gate closed, or the
    /// controller left), so nothing held before carries over.
    pub fn reset(&mut self) {
        self.pads.clear();
    }

    /// Forgets one controller.
    pub fn forget(&mut self, id: u32) {
        self.pads.remove(&id);
    }
}

fn trigger_side(button: PadButton) -> Option<Side> {
    match button {
        PadButton::LeftTrigger => Some(Side::Left),
        PadButton::RightTrigger => Some(Side::Right),
        _ => None,
    }
}

fn set_button(pad: &mut PadState, id: u32, button: PadButton, down: bool, out: &mut Vec<PadEvent>) {
    let changed = if down {
        pad.down.insert(button)
    } else {
        pad.down.remove(&button)
    };
    if changed {
        out.push(PadEvent::Button { id, button, down });
    }
}

fn set_trigger(pad: &mut PadState, id: u32, side: Side, value: f32, out: &mut Vec<PadEvent>) {
    pad.triggers.insert(side, value);
    let button = match side {
        Side::Left => PadButton::LeftTrigger,
        Side::Right => PadButton::RightTrigger,
    };
    set_button(pad, id, button, value >= TRIGGER_THRESHOLD, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(button: PadButton) -> RawEvent {
        RawEvent::Pressed { button }
    }

    fn release(button: PadButton) -> RawEvent {
        RawEvent::Released { button }
    }

    fn button(id: u32, button: PadButton, down: bool) -> PadEvent {
        PadEvent::Button { id, button, down }
    }

    #[test]
    fn every_digital_button_goes_down_and_up_once() {
        let all = [
            PadButton::South,
            PadButton::East,
            PadButton::West,
            PadButton::North,
            PadButton::LeftBumper,
            PadButton::RightBumper,
            PadButton::Select,
            PadButton::Start,
            PadButton::Guide,
            PadButton::LeftStick,
            PadButton::RightStick,
            PadButton::DpadUp,
            PadButton::DpadDown,
            PadButton::DpadLeft,
            PadButton::DpadRight,
        ];
        let mut m = Mapper::default();
        for b in all {
            assert_eq!(m.feed(3, press(b)), vec![button(3, b, true)], "{b:?} down");
            // A second press without a release (a gilrs repeat) sends nothing.
            assert_eq!(m.feed(3, press(b)), vec![], "{b:?} repeat");
            assert_eq!(m.feed(3, release(b)), vec![button(3, b, false)], "{b:?} up");
            assert_eq!(m.feed(3, release(b)), vec![], "{b:?} second up");
        }
    }

    #[test]
    fn analog_triggers_press_at_one_half() {
        let mut m = Mapper::default();
        let lt = PadButton::LeftTrigger;
        let changed = |value| RawEvent::Changed { button: lt, value };
        assert_eq!(m.feed(0, changed(0.3)), vec![]);
        assert_eq!(m.feed(0, changed(0.5)), vec![button(0, lt, true)]);
        assert_eq!(m.feed(0, changed(0.9)), vec![]);
        // gilrs' own press at its threshold does not double the event.
        assert_eq!(m.feed(0, press(lt)), vec![]);
        assert_eq!(m.feed(0, changed(0.49)), vec![button(0, lt, false)]);
        assert_eq!(
            m.flush(),
            vec![PadEvent::Trigger {
                id: 0,
                trigger: Side::Left,
                value: 0.49
            }]
        );
    }

    #[test]
    fn a_digital_trigger_reports_full_travel() {
        let mut m = Mapper::default();
        let rt = PadButton::RightTrigger;
        assert_eq!(m.feed(1, press(rt)), vec![button(1, rt, true)]);
        assert_eq!(
            m.flush(),
            vec![PadEvent::Trigger {
                id: 1,
                trigger: Side::Right,
                value: 1.0
            }]
        );
        assert_eq!(m.feed(1, release(rt)), vec![button(1, rt, false)]);
        assert_eq!(
            m.flush(),
            vec![PadEvent::Trigger {
                id: 1,
                trigger: Side::Right,
                value: 0.0
            }]
        );
    }

    #[test]
    fn non_trigger_analog_values_are_ignored() {
        let mut m = Mapper::default();
        let ev = RawEvent::Changed {
            button: PadButton::South,
            value: 1.0,
        };
        assert_eq!(m.feed(0, ev), vec![]);
        assert_eq!(m.flush(), vec![]);
    }

    #[test]
    fn sticks_are_sent_once_per_tick_and_only_after_a_real_move() {
        let mut m = Mapper::default();
        let axis = |axis, value| RawEvent::Axis { axis, value };
        m.feed(0, axis(PadAxis::LeftStickX, 0.3));
        m.feed(0, axis(PadAxis::LeftStickX, 0.6));
        m.feed(0, axis(PadAxis::LeftStickY, -0.4));
        // Many axis events in one tick: one stick event with the last values.
        assert_eq!(
            m.flush(),
            vec![PadEvent::Stick {
                id: 0,
                stick: Side::Left,
                x: 0.6,
                y: -0.4
            }]
        );
        assert_eq!(m.flush(), vec![]);
        // A wobble under the step is not sent.
        m.feed(0, axis(PadAxis::LeftStickX, 0.61));
        assert_eq!(m.flush(), vec![]);
        m.feed(0, axis(PadAxis::LeftStickX, 0.63));
        assert_eq!(m.flush().len(), 1);
        // The right stick is its own.
        m.feed(0, axis(PadAxis::RightStickY, 1.0));
        assert_eq!(
            m.flush(),
            vec![PadEvent::Stick {
                id: 0,
                stick: Side::Right,
                x: 0.0,
                y: 1.0
            }]
        );
    }

    #[test]
    fn a_hat_axis_becomes_dpad_buttons() {
        let mut m = Mapper::default();
        let x = |value| RawEvent::Axis {
            axis: PadAxis::DpadX,
            value,
        };
        assert_eq!(
            m.feed(0, x(-1.0)),
            vec![button(0, PadButton::DpadLeft, true)]
        );
        assert_eq!(
            m.feed(0, x(1.0)),
            vec![
                button(0, PadButton::DpadLeft, false),
                button(0, PadButton::DpadRight, true)
            ]
        );
        assert_eq!(
            m.feed(0, x(0.0)),
            vec![button(0, PadButton::DpadRight, false)]
        );
        let y = RawEvent::Axis {
            axis: PadAxis::DpadY,
            value: 1.0,
        };
        assert_eq!(m.feed(0, y), vec![button(0, PadButton::DpadUp, true)]);
    }

    #[test]
    fn two_controllers_are_tracked_apart() {
        let mut m = Mapper::default();
        assert_eq!(m.feed(0, press(PadButton::South)).len(), 1);
        assert_eq!(m.feed(1, press(PadButton::South)).len(), 1);
        m.forget(0);
        assert_eq!(m.feed(0, press(PadButton::South)).len(), 1);
        assert_eq!(m.feed(1, press(PadButton::South)).len(), 0);
    }

    #[test]
    fn reset_forgets_held_buttons_and_sent_values() {
        let mut m = Mapper::default();
        m.feed(0, press(PadButton::South));
        m.feed(
            0,
            RawEvent::Axis {
                axis: PadAxis::LeftStickX,
                value: 1.0,
            },
        );
        m.flush();
        m.reset();
        assert_eq!(m.feed(0, press(PadButton::South)).len(), 1);
        assert_eq!(m.flush(), vec![]);
    }

    #[test]
    fn the_wire_format_is_snake_case_and_tagged() {
        let raw: RawEvent =
            serde_json::from_str(r#"{"kind":"pressed","button":"left_bumper"}"#).unwrap();
        assert_eq!(raw, press(PadButton::LeftBumper));
        let raw: RawEvent =
            serde_json::from_str(r#"{"kind":"axis","axis":"right_stick_y","value":-0.5}"#).unwrap();
        assert_eq!(
            raw,
            RawEvent::Axis {
                axis: PadAxis::RightStickY,
                value: -0.5
            }
        );
        let ev = serde_json::to_value(button(2, PadButton::DpadUp, true)).unwrap();
        assert_eq!(
            ev,
            serde_json::json!({"kind": "button", "id": 2, "button": "dpad_up", "down": true})
        );
        let ev = serde_json::to_value(PadEvent::ReleasedAll).unwrap();
        assert_eq!(ev, serde_json::json!({"kind": "released_all"}));
    }
}

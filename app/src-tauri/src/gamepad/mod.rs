//! TV mode's controller reader (TV mode README § Decided Implementation): a
//! native gilrs reader that runs for TV mode only and sends every controller
//! input to the UI as the `gamepad` event.
//!
//! - The poll thread (`gamepad-poll`) starts the first time TV mode turns on
//!   and runs for the rest of the process. A gilrs that cannot start (no
//!   `/dev/input` access, a container, CI) is logged once at `warn`; the
//!   keyboard still works, and the e2e injector still does.
//! - Input passes only while the [`Gate`] is open: TV mode on and GRID's
//!   window focused, a game running or not (the Playing page answers it).
//!   Closing it sends `released_all`.
//! - Connections are always reported, so the UI's controller list stays
//!   right while the gate is closed.

pub mod gate;
pub mod identity;
pub mod mapping;

use gate::{Gate, Transition};
use identity::PadInfo;
use mapping::{Mapper, PadAxis, PadButton, PadEvent, RawEvent};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// The event every controller input reaches the UI as (`GAMEPAD_EVENT` in
/// `api.ts`); its payload is a [`PadEvent`].
pub const GAMEPAD_EVENT: &str = "gamepad";

/// How long the poll thread sleeps between ticks: short while input passes,
/// longer while the gate is closed (connections still get noticed).
const TICK_OPEN: Duration = Duration::from_millis(8);
const TICK_CLOSED: Duration = Duration::from_millis(50);

/// One thing that happened to the controllers: what the poll thread reads
/// from gilrs, and what the e2e injector sends (`e2e_gamepad_inject`, as
/// `{ "kind": "input", "id": 0, "event": { "kind": "pressed", "button":
/// "south" } }` and so on).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GamepadInput {
    Connect {
        pad: PadInfo,
    },
    Disconnect {
        id: u32,
    },
    Input {
        id: u32,
        event: RawEvent,
    },
    /// GRID's window gained or lost focus (e2e only stands in for the
    /// window manager with this).
    Focus {
        focused: bool,
    },
}

#[derive(Debug, Default)]
struct Shared {
    gate: Gate,
    mapper: Mapper,
    pads: BTreeMap<u32, PadInfo>,
}

impl Shared {
    /// Applies one input; returns what to send.
    fn handle(&mut self, input: GamepadInput) -> Vec<PadEvent> {
        match input {
            GamepadInput::Connect { pad } => {
                self.mapper.forget(pad.id);
                self.pads.insert(pad.id, pad.clone());
                vec![PadEvent::Connected { pad }]
            }
            GamepadInput::Disconnect { id } => {
                self.mapper.forget(id);
                self.pads.remove(&id);
                vec![PadEvent::Disconnected { id }]
            }
            GamepadInput::Input { id, event } => {
                if self.gate.open() {
                    self.mapper.feed(id, event)
                } else {
                    Vec::new()
                }
            }
            GamepadInput::Focus { focused } => self.update_gate(|g| g.window_focused = focused).1,
        }
    }

    /// The stick and trigger values to send at the end of a tick.
    fn flush(&mut self) -> Vec<PadEvent> {
        if self.gate.open() {
            self.mapper.flush()
        } else {
            Vec::new()
        }
    }

    fn update_gate(&mut self, change: impl FnOnce(&mut Gate)) -> (Transition, Vec<PadEvent>) {
        let transition = self.gate.update(change);
        if transition == Transition::Closed {
            self.mapper.reset();
            (transition, vec![PadEvent::ReleasedAll])
        } else {
            (transition, Vec::new())
        }
    }
}

type Emitter = Box<dyn Fn(&PadEvent) + Send + Sync>;

/// The reader: its gate, the connected controllers and the poll thread.
pub struct GamepadService {
    shared: Mutex<Shared>,
    started: AtomicBool,
    emitter: OnceLock<Emitter>,
}

impl GamepadService {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            shared: Mutex::new(Shared::default()),
            started: AtomicBool::new(false),
            emitter: OnceLock::new(),
        })
    }

    /// Where events go (the Tauri `gamepad` event). Set once, in `setup`.
    pub fn set_emitter(&self, emit: impl Fn(&PadEvent) + Send + Sync + 'static) {
        let _ = self.emitter.set(Box::new(emit));
    }

    fn emit(&self, events: Vec<PadEvent>) {
        if let Some(emit) = self.emitter.get() {
            for event in &events {
                emit(event);
            }
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Shared> {
        // A panic while holding the lock leaves plain data behind: carry on.
        self.shared.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn update_gate(&self, change: impl FnOnce(&mut Gate)) -> Transition {
        let (transition, events) = self.lock().update_gate(change);
        self.emit(events);
        transition
    }

    /// TV mode turned on or off.
    pub fn set_tv_active(&self, active: bool) -> Transition {
        self.update_gate(|g| g.tv_active = active)
    }

    /// GRID's window gained or lost focus.
    pub fn set_window_focused(&self, focused: bool) -> Transition {
        self.update_gate(|g| g.window_focused = focused)
    }

    /// A game started, or the last one ended.
    pub fn set_games_running(&self, running: bool) -> Transition {
        self.update_gate(|g| g.games_running = running)
    }

    pub fn tv_active(&self) -> bool {
        self.lock().gate.tv_active
    }

    pub fn games_running(&self) -> bool {
        self.lock().gate.games_running
    }

    /// The connected controllers, by id.
    pub fn pads(&self) -> Vec<PadInfo> {
        self.lock().pads.values().cloned().collect()
    }

    /// Applies inputs as one poll tick would, and sends the result.
    pub fn apply(&self, inputs: Vec<GamepadInput>) {
        let events = {
            let mut shared = self.lock();
            let mut events: Vec<PadEvent> = Vec::new();
            for input in inputs {
                events.extend(shared.handle(input));
            }
            events.extend(shared.flush());
            events
        };
        self.emit(events);
    }

    /// Starts the poll thread, once.
    pub fn ensure_started(self: &Arc<Self>) {
        if self.started.swap(true, Ordering::SeqCst) {
            return;
        }
        let service = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name("gamepad-poll".into())
            .spawn(move || service.poll());
        if let Err(e) = spawned {
            tracing::warn!("gamepad: the poll thread did not start: {e}");
        }
    }

    fn poll(&self) {
        let mut gilrs = match gilrs::Gilrs::new() {
            Ok(gilrs) => gilrs,
            Err(e) => {
                tracing::warn!("gamepad: gilrs unavailable ({e}); controller input disabled");
                return;
            }
        };
        let present: Vec<GamepadInput> = gilrs
            .gamepads()
            .map(|(id, pad)| GamepadInput::Connect {
                pad: pad_info(id, &pad),
            })
            .collect();
        tracing::info!(controllers = present.len(), "gamepad: reader started");
        self.apply(present);
        loop {
            let mut inputs = Vec::new();
            while let Some(event) = gilrs.next_event() {
                let id = pad_id(event.id);
                match event.event {
                    gilrs::EventType::Connected => inputs.push(GamepadInput::Connect {
                        pad: pad_info(event.id, &gilrs.gamepad(event.id)),
                    }),
                    gilrs::EventType::Disconnected => inputs.push(GamepadInput::Disconnect { id }),
                    other => {
                        if let Some(event) = from_gilrs(other) {
                            inputs.push(GamepadInput::Input { id, event });
                        }
                    }
                }
            }
            self.apply(inputs);
            let open = self.lock().gate.open();
            std::thread::sleep(if open { TICK_OPEN } else { TICK_CLOSED });
        }
    }
}

fn pad_id(id: gilrs::GamepadId) -> u32 {
    usize::from(id) as u32
}

fn pad_info(id: gilrs::GamepadId, pad: &gilrs::Gamepad<'_>) -> PadInfo {
    PadInfo::new(
        pad_id(id),
        pad.uuid(),
        pad.name(),
        pad.vendor_id(),
        pad.product_id(),
    )
}

/// gilrs' event → the raw event TV mode reads, or `None` for everything it
/// does not (repeats, force feedback, unknown buttons and axes, the Z axes).
fn from_gilrs(event: gilrs::EventType) -> Option<RawEvent> {
    use gilrs::EventType as E;
    Some(match event {
        E::ButtonPressed(b, _) => RawEvent::Pressed { button: button(b)? },
        E::ButtonReleased(b, _) => RawEvent::Released { button: button(b)? },
        E::ButtonChanged(b, value, _) => RawEvent::Changed {
            button: button(b)?,
            value,
        },
        E::AxisChanged(a, value, _) => RawEvent::Axis {
            axis: axis(a)?,
            value,
        },
        _ => return None,
    })
}

fn button(b: gilrs::Button) -> Option<PadButton> {
    use gilrs::Button as B;
    Some(match b {
        B::South => PadButton::South,
        B::East => PadButton::East,
        B::West => PadButton::West,
        B::North => PadButton::North,
        // gilrs names the bumpers "triggers" and the triggers "triggers 2".
        B::LeftTrigger => PadButton::LeftBumper,
        B::RightTrigger => PadButton::RightBumper,
        B::LeftTrigger2 => PadButton::LeftTrigger,
        B::RightTrigger2 => PadButton::RightTrigger,
        B::Select => PadButton::Select,
        B::Start => PadButton::Start,
        B::Mode => PadButton::Guide,
        B::LeftThumb => PadButton::LeftStick,
        B::RightThumb => PadButton::RightStick,
        B::DPadUp => PadButton::DpadUp,
        B::DPadDown => PadButton::DpadDown,
        B::DPadLeft => PadButton::DpadLeft,
        B::DPadRight => PadButton::DpadRight,
        _ => return None,
    })
}

fn axis(a: gilrs::Axis) -> Option<PadAxis> {
    use gilrs::Axis as A;
    Some(match a {
        A::LeftStickX => PadAxis::LeftStickX,
        A::LeftStickY => PadAxis::LeftStickY,
        A::RightStickX => PadAxis::RightStickX,
        A::RightStickY => PadAxis::RightStickY,
        A::DPadX => PadAxis::DpadX,
        A::DPadY => PadAxis::DpadY,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mapping::Side;

    fn pad(id: u32) -> PadInfo {
        PadInfo::new(id, [1; 16], "Test pad", Some(0x045e), Some(0x0b13))
    }

    fn input(id: u32, event: RawEvent) -> GamepadInput {
        GamepadInput::Input { id, event }
    }

    fn south(down: bool) -> RawEvent {
        if down {
            RawEvent::Pressed {
                button: PadButton::South,
            }
        } else {
            RawEvent::Released {
                button: PadButton::South,
            }
        }
    }

    fn open_shared() -> Shared {
        let mut shared = Shared::default();
        shared.update_gate(|g| g.tv_active = true);
        shared
    }

    #[test]
    fn nothing_passes_in_desktop_mode() {
        let mut shared = Shared::default();
        assert_eq!(shared.handle(input(0, south(true))), vec![]);
    }

    #[test]
    fn connections_are_reported_even_while_closed() {
        let mut shared = Shared::default();
        assert_eq!(
            shared.handle(GamepadInput::Connect { pad: pad(4) }),
            vec![PadEvent::Connected { pad: pad(4) }]
        );
        assert_eq!(shared.pads.len(), 1);
        assert_eq!(
            shared.handle(GamepadInput::Disconnect { id: 4 }),
            vec![PadEvent::Disconnected { id: 4 }]
        );
        assert!(shared.pads.is_empty());
    }

    #[test]
    fn buttons_pass_while_open_and_closing_releases_everything() {
        let mut shared = open_shared();
        assert_eq!(
            shared.handle(input(0, south(true))),
            vec![PadEvent::Button {
                id: 0,
                button: PadButton::South,
                down: true
            }]
        );
        // A game starting with GRID still focused: Playing keeps listening.
        let (transition, events) = shared.update_gate(|g| g.games_running = true);
        assert_eq!(transition, Transition::None);
        assert_eq!(events, vec![]);
        assert_eq!(shared.handle(input(0, south(false))).len(), 1);
        // The game's window takes focus: closed, everything released.
        let (transition, events) = shared.update_gate(|g| g.window_focused = false);
        assert_eq!(transition, Transition::Closed);
        assert_eq!(events, vec![PadEvent::ReleasedAll]);
        // Closed: dropped.
        assert_eq!(shared.handle(input(0, south(false))), vec![]);
        // GRID focused again, the game still running: a fresh press goes out.
        shared.update_gate(|g| g.window_focused = true);
        assert_eq!(shared.handle(input(0, south(true))).len(), 1);
    }

    #[test]
    fn losing_focus_closes_the_gate_through_an_input_too() {
        let mut shared = open_shared();
        assert_eq!(
            shared.handle(GamepadInput::Focus { focused: false }),
            vec![PadEvent::ReleasedAll]
        );
        assert_eq!(
            shared.handle(GamepadInput::Focus { focused: false }),
            vec![]
        );
        assert_eq!(shared.handle(input(0, south(true))), vec![]);
    }

    #[test]
    fn sticks_flush_only_while_open() {
        let mut shared = open_shared();
        shared.handle(input(
            1,
            RawEvent::Axis {
                axis: PadAxis::RightStickY,
                value: -1.0,
            },
        ));
        assert_eq!(
            shared.flush(),
            vec![PadEvent::Stick {
                id: 1,
                stick: Side::Right,
                x: 0.0,
                y: -1.0
            }]
        );
        shared.update_gate(|g| g.tv_active = false);
        assert_eq!(shared.flush(), vec![]);
    }

    #[test]
    fn the_service_sends_released_all_when_tv_mode_turns_off() {
        let service = GamepadService::new();
        let sent = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&sent);
        service.set_emitter(move |e| sink.lock().unwrap().push(e.clone()));
        assert_eq!(service.set_tv_active(true), Transition::Opened);
        service.apply(vec![
            GamepadInput::Connect { pad: pad(0) },
            input(0, south(true)),
        ]);
        assert_eq!(service.set_tv_active(false), Transition::Closed);
        let sent = sent.lock().unwrap();
        assert_eq!(sent.len(), 3);
        assert!(matches!(sent[0], PadEvent::Connected { .. }));
        assert!(matches!(sent[1], PadEvent::Button { down: true, .. }));
        assert_eq!(sent[2], PadEvent::ReleasedAll);
        assert_eq!(service.pads(), vec![pad(0)]);
    }

    #[test]
    fn the_injector_wire_format_parses() {
        let inputs: Vec<GamepadInput> = serde_json::from_str(
            r#"[
                {"kind":"connect","pad":{"id":0,"key":"0101","name":"Pad","vendor":null,"product":null}},
                {"kind":"input","id":0,"event":{"kind":"changed","button":"right_trigger","value":0.7}},
                {"kind":"focus","focused":false},
                {"kind":"disconnect","id":0}
            ]"#,
        )
        .unwrap();
        assert_eq!(inputs.len(), 4);
        assert_eq!(
            inputs[1],
            input(
                0,
                RawEvent::Changed {
                    button: PadButton::RightTrigger,
                    value: 0.7
                }
            )
        );
    }

    #[test]
    fn gilrs_buttons_map_by_position() {
        assert_eq!(
            button(gilrs::Button::LeftTrigger),
            Some(PadButton::LeftBumper)
        );
        assert_eq!(
            button(gilrs::Button::LeftTrigger2),
            Some(PadButton::LeftTrigger)
        );
        assert_eq!(button(gilrs::Button::Mode), Some(PadButton::Guide));
        assert_eq!(button(gilrs::Button::C), None);
        assert_eq!(axis(gilrs::Axis::LeftZ), None);
    }
}

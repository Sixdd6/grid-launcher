pub mod mapping;

use mapping::{Axis, Button, Mapper, NavAction};
use std::time::{Duration, Instant};
use tauri::Emitter;

#[derive(Clone, serde::Serialize)]
struct NavPayload {
    action: NavAction,
}

/// Dedicated poll thread; emits "nav" events to the frontend.
pub fn spawn(app: tauri::AppHandle) {
    std::thread::Builder::new()
        .name("gamepad-poll".into())
        .spawn(move || {
            let Ok(mut gilrs) = gilrs::Gilrs::new() else {
                tracing::warn!("gamepad: gilrs unavailable; controller input disabled");
                return;
            };
            let mut mapper = Mapper::new(0.3, Duration::from_millis(200));
            loop {
                while let Some(ev) = gilrs.next_event() {
                    let now = Instant::now();
                    let action = match ev.event {
                        gilrs::EventType::ButtonPressed(b, _) => {
                            translate_button(b).and_then(|b| mapper.button(b, true, now))
                        }
                        gilrs::EventType::ButtonReleased(b, _) => {
                            translate_button(b).and_then(|b| mapper.button(b, false, now))
                        }
                        gilrs::EventType::AxisChanged(a, v, _) => {
                            translate_axis(a).and_then(|a| mapper.axis(a, v, now))
                        }
                        _ => None,
                    };
                    if let Some(action) = action {
                        let _ = app.emit("nav", NavPayload { action });
                    }
                }
                // Held-stick repeats need re-evaluation between events.
                std::thread::sleep(Duration::from_millis(16));
                for (axis, value) in current_axis_values(&gilrs) {
                    if let Some(action) = mapper.axis(axis, value, Instant::now()) {
                        let _ = app.emit("nav", NavPayload { action });
                    }
                }
            }
        })
        .expect("spawn gamepad thread");
}

fn translate_button(b: gilrs::Button) -> Option<Button> {
    Some(match b {
        gilrs::Button::DPadUp => Button::DpadUp,
        gilrs::Button::DPadDown => Button::DpadDown,
        gilrs::Button::DPadLeft => Button::DpadLeft,
        gilrs::Button::DPadRight => Button::DpadRight,
        gilrs::Button::South => Button::South,
        gilrs::Button::East => Button::East,
        _ => return None,
    })
}

fn translate_axis(a: gilrs::Axis) -> Option<Axis> {
    Some(match a {
        gilrs::Axis::LeftStickX => Axis::LeftStickX,
        gilrs::Axis::LeftStickY => Axis::LeftStickY,
        _ => return None,
    })
}

fn current_axis_values(gilrs: &gilrs::Gilrs) -> Vec<(Axis, f32)> {
    let mut out = Vec::new();
    for (_id, gamepad) in gilrs.gamepads() {
        for (ga, la) in [
            (gilrs::Axis::LeftStickX, Axis::LeftStickX),
            (gilrs::Axis::LeftStickY, Axis::LeftStickY),
        ] {
            if let Some(data) = gamepad.axis_data(ga) {
                out.push((la, data.value()));
            }
        }
    }
    out
}

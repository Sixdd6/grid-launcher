# TV mode: handover notes between phases

## After P4 (6e69cc6): native interface for the TV UI
- Event `gamepad` (`GAMEPAD_EVENT` in api.ts), payload `PadEvent` tagged by `kind`:
  `connected {pad: PadInfo}`, `disconnected {id}`, `button {id, button: PadButton, down}`,
  `stick {id, stick: 'left'|'right', x, y}` (−1…1, Y positive up),
  `trigger {id, trigger: 'left'|'right', value}` (0…1), `released_all`.
- `PadInfo = {id: number, key: string (gilrs UUID, 32 hex), name, vendor: number|null, product: number|null}`.
- `PadButton` (physical positions): `south east west north left_bumper right_bumper
  left_trigger right_trigger select start guide left_stick right_stick dpad_up dpad_down
  dpad_left dpad_right`. `guide` is reported but must not be bound (ruling 5).
- Buttons arrive on change only: repeat timing (400/90 ms; keyboard 500 ms then 20/s) is the
  UI's job. Xbox/Nintendo assignment is applied in the UI from `uiSettings.tvAssignments[pad.key]`.
  A `disconnected` pad's held buttons get no up events: treat as released. connected/disconnected
  are sent even while the gate is closed. `gamepad_list` is empty until TV mode first turns on.
- Commands (api.ts wrappers): `getUiMode() -> {mode: 'desktop'|'tv', started_with_tv, fullscreen}`,
  `setUiMode(mode)`, `gamepadList() -> PadInfo[]`, `tvYieldToGame()`, `quitApp()`,
  `listFolder(path, files) -> {path, parent|null, entries:[{name,is_dir}]}` (empty path = home),
  `readClipboardText()`, e2e only `e2eGamepadInject(events: GamepadInput[])` where GamepadInput is
  `{kind:'connect', pad}` | `{kind:'disconnect', id}` | `{kind:'input', id, event}` (event
  `{kind:'pressed'|'released', button}` | `{kind:'changed', button, value}` |
  `{kind:'axis', axis: left_stick_x|left_stick_y|right_stick_x|right_stick_y|dpad_x|dpad_y, value}`)
  | `{kind:'focus', focused}`.
- With `--tv`, Rust is already in TV mode (fullscreen, gate open) before the UI loads: read
  `get_ui_mode` at startup to mount TvShell.
- `[ui]` keys: `open_on` (automatic|discover|library), `tv_prompts` (match|xbox|ps|nintendo|8bitdo),
  `tv_assignments` (pad key → xbox|nintendo). uiSettings store: `openOn`, `tvPrompts`,
  `tvAssignments`, `loaded`, `setOpenOn`.
- `initialView(connected, offline, firstDrive, openOn)` in lib/shell.ts is the shared start rule.
- Unverified (do in P5): window geometry after TV → quit → relaunch (window-state plugin has
  FULLSCREEN removed; `exit_app` leaves fullscreen first).

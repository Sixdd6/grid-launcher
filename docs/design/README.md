# GRID design system and prototype

The source of the **GRID Launcher** design system published at
<https://claude.ai/artifact/VBiKSXWqmUHoXPgqbuxua6> (private; share it from the page).
Nothing here is built into the app: it is the reference the UI work on
`design/ui-pass` is designed against.

## What is here

- `system/` — the design system's files, as published:
  - `README.md` — the brand book: surfaces, colour, type, spacing, motion, voice, icons.
  - `tokens.json` — every token from `app/src/app.css` (light and dark).
  - `components/<Name>/` — a static rendition (`preview.html`) and guidelines
    (`README.md`) for each app component; `components/bundle.css` styles them.
  - `components/CurrentApp/` — a clickable prototype of the app as it is today.
  - `components/ProposedApp/` — the same shell with the changes under discussion;
    its `README.md` lists every difference.
  - `components/TVMode/` — the controller-first, fullscreen TV mode, clickable and
    playable with a controller; its `README.md` describes every screen.
  - `assets/Icons/` — the icons from `app/src/lib/icons.ts`. The app icon is
    `app/src-tauri/icons/icon.png`.
- `prototype/` — the generator that writes the component previews and both app
  prototypes.

## Rebuilding the previews

The two prototype cards are generated; edit the generator, not the HTML.

```bash
python3 -I docs/design/prototype/applayout.py docs/design/system/components
```

- `gen.py` — the small component previews and the shared icon helper.
- `applayout.py` — the Current app shell, then Proposed app by patching it.
- `panes.js` — Current app's Emulators and Settings panes.
- `panes_systems.js`, `next.css` — Proposed app's Settings › Systems.
- `fresh.js`, `fresh.css` — Proposed app's first run (Connect, Choose your
  systems), the setup card and the bottom-right notification cards.
- `serverhead.js`, `serverhead.css` — the folding Server and Library headers.
- `discover.js`, `discover.css` — Server › Discover: the New releases coverflow and
  the auto-advancing Recently added list.
- `themetoggle.js`, `themetoggle.css` — the light / dark toggle.
- `tvmode.py`, `tvmode.js`, `tvmode.css` — the TV mode card (its own shell and focus
  engine): `python3 -I docs/design/prototype/tvmode.py docs/design/system/components`.
- `glyphs/` — the controller button glyphs the TV mode card inlines (Kenney Input
  Prompts, CC0; `map.json` assigns them, `License.txt` is the pack's license).
- `icons.json` — icon paths: the app's (`app/src/lib/icons.ts`) plus proposed ones.

To view a card locally, it needs the design system's `tokens.css`, which the
published page generates from `tokens.json`; open it on the published page instead.

# TV mode implementation checklist

What the real app must do to match the approved TV mode design exactly. Sources:

- `docs/design/system/components/TVMode/README.md` ("README"). Its **Decided** section is binding.
- `docs/design/prototype/tvmode.js` and `tvmode.css`, the clickable prototype.
- `docs/design/prototype/glyphs/README.md` and `map.json`, the button glyph sets.
- `docs/design/system/components/GameBackups/README.md` ("GB card"), which the TV card's Game backups section adapts.

Each item is one testable statement. Copy text is quoted exactly; `<x>` marks a value the app fills in.
Where a prototype string is the only source for a notification, the desktop's wording for the same
event wins (README § The shell › Notifications). Items numbered here stay numbered; the "Not to copy"
and "Conflicts" lists at the end are not part of the build.

## 1. Shell & sizing

- [ ] 1. TV mode is a fullscreen interface that reaches every feature on main with a game controller alone (src: README intro).
- [x] 2. Every measure is in rem, and the root font size is the viewport height / 54 (20px at 1080p), so the layout is the same at 720p, 1080p and 4K (src: README § The shell › Size).
- [x] 3. No text is smaller than 0.85rem (17px at 1080p); body text is 1.05–1.1rem (src: README § The shell › Size). (P14: tv-layout assertMinText at 720p, 1080p and 4K)
- [x] 4. The window itself never scrolls: only the designated areas (rows, grids, rails, panes, lists, pages, Details tab bodies, list overlays) scroll (src: tvmode.css `html, body`; tvmode.js `SCROLLERS`). (P14: tv-layout assertNoDocumentScroll)
- [x] 5. The screen is three bands: top bar 4.6rem, the stage filling the rest, hint bar 3.6rem at the bottom (src: README § The shell; tvmode.css `.top`, `.stage`, `.hints`). (P14: tv-layout assertBands)
- [x] 6. The top bar, hint bar and every tab screen use a 2.4rem side gutter (src: tvmode.css `.top`, `.hints`, `.split`, `.list`, `.page`). (final audit: verified in code)
- [x] 7. Scrollbars are hidden on every scrolling area (src: tvmode.css `scrollbar-width: none`).
- [ ] 8. Text sits only on `panel` or `overlay` surfaces, as on the desktop (src: README § The shell › Background art).
- [x] 9. The background shows the focused game's art, blurred, behind the panels, reaching 6rem past the screen edges so the blur has no hard border (src: README § The shell › Background art; tvmode.css `.tv-art`). (Accepted deviation, consultant ruling F: drawn edge to edge, inset 0, as the backend blurs the art.)
- [x] 10. A layer of the `bg` colour at the opacity set by Settings › Appearance › Background fade covers the art, and Background blur sets the blur (src: README § Background art; tvmode.css `.tv-art::after`; tvmode.js `srowAdj`). (final audit: verified in code)
- [x] 11. The background art changes whenever a cover takes focus (any grid, row, Related row, Search result) and when Details or Playing opens, fading over 360 ms (src: tvmode.js `setFocus`, `hero`, `details().after`, `playing().after`; tvmode.css `.tv-art`). (final audit: verified in code)
- [x] 12. With Background art off, the art is hidden (src: tvmode.js `hero`, `srowAct`). (final audit: verified in code)
- [x] 13. The theme is dark by default, and Settings › Appearance › Theme (Follow system, Dark, Light) still applies, at once when changed (src: README § The shell › Theme; tvmode.js `applyTheme`).
- [x] 14. A rail-and-content screen (Library, Server, Settings) has an 18rem rail, a 1.2rem gap and 1.2rem top padding; the rail and content panels have a 1.2rem bottom margin (src: tvmode.css `.split`, `.rail`, `.split .pane`). (final audit: verified in code)
- [x] 15. TV mode has one cover size: grid and row covers are 10rem wide, the Details cover is 17rem, all at 3:4 with a 0.7rem radius (src: README § Decided Implementation; tvmode.css `.cv`, `.grid .cv`, `.track .cv`, `.det-l .cv`). (final audit: verified in code)
- [x] 16. Grid covers show the title under the cover (0.9rem muted, one line, ellipsis; text-h and semibold when focused); covers in sideways rows and on Details show no label (src: tvmode.css `.cv-l`, `.track .cv-l`, `.det-l .cv-l`). (final audit: verified in code)
- [x] 17. Covers carry the desktop badges: installed dot (green, top right), UPDATE (warning, top left), MISSING (red, top left), platform tag (bottom left) in mixed lists, and a "Queued" or "<n>%" tag (bottom right) while installing (src: README § Tabs › Library; tvmode.js `cover`). (final audit: verified in code)
- [x] 18. MISSING replaces UPDATE when a game is both (src: tvmode.js `cover`). (final audit: verified in code)
- [x] 19. Focus is one 0.2rem ring in the `focus` colour at a 0.2rem offset, the same everywhere (src: README § The shell › Focus; tvmode.css `[data-f].f`). (final audit: verified in code)
- [x] 20. A focused cover grows to 107% with `shadow-lift` and a ring drawn as a 0.22rem `bg` gap and a `focus` band out to 0.42rem (src: README § The shell › Focus; tvmode.css `.cv.f .cv-art`). (final audit: verified in code)
- [x] 21. A focused row (list row, settings row, list option, Connect field) takes the `selected` fill instead of growing (src: README § The shell › Focus; tvmode.css `.lrow.f`, `.srow.f`, `.opt.f`, `.field.f`). (final audit: verified in code)
- [x] 22. A focused button grows to 104% (a primary one also takes `primary-hover`), a focused keyboard key to 106% and a focused media tile to 105% (src: tvmode.css `.btn.f`, `.btn.pri.f`, `.key.f`, `.mtile.f`). (final audit: verified in code)
- [x] 23. Focus changes animate transform, shadow and background over 150 ms (src: tvmode.css `[data-f]`). (final audit: verified in code)
- [x] 24. Disabled controls show at 45% opacity and stay focusable, so A on them can say why (src: tvmode.css `[data-f][disabled]`; tvmode.js `press`). (final audit: verified in code)
- [x] 25. With reduced motion, focus has no transitions or growth, the notification countdown bar does not animate, and scrolls jump (src: tvmode.css `@media (prefers-reduced-motion)`; README § Scrolling).
- [x] 26. Rows, grids and Details tabs leave room around every cover for its 7% growth and ring (rows pad 1.4rem above and below; a grid's scroll area reaches 1.2rem past its header on both sides; a Details tab body has 1rem side padding pulled back by a negative margin), while covers still line up with the titles and headers beside them (src: README § Scrolling; tvmode.css `.track`, `.scroll`, `.tbody`). (P14: tv-layout assertNoClipping after every move)
- [x] 27. Grids lay out 10rem columns that fill the width (space-between), with 1.6rem row and 1.2rem column gaps and 1.4rem / 1.2rem / 2.4rem padding (top / sides / bottom) (src: tvmode.css `.grid`). (final audit: verified in code)
- [x] 28. Buttons are pills at least 3rem tall with 1.05rem semibold text, in default, primary (`primary` fill) and danger (`danger` text and border) styles (src: tvmode.css `.btn`). (final audit: verified in code)
- [x] 29. Switches are 3.4 x 1.9rem, sliders 12rem, radio dots 1.5rem; a switch that is on is `primary` filled (src: tvmode.css `.sw`, `.sl`, `.radio`). (final audit: verified in code)

## 2. Input

### Controller reading

- [x] 30. Controllers are read natively in Rust by a gilrs reader that runs for TV mode only and sends button events to the UI (src: README § Decided Implementation).
- [x] 31. No controller input is acted on while a game is running (src: README § Decided Implementation; README § Pages › Playing). (final audit: verified in code)
- [x] 32. No controller input is acted on while the GRID window isn't focused (src: README § Decided Implementation). (final audit: verified in code)
- [ ] 33. When the game closes, GRID listens to the controller again and then shows the save upload notices (src: README § Pages › Playing).
- [x] 34. The logical inputs are A, B, X, Y, LB, RB, LT, RT, View, Menu, the D-pad, the left stick and the right stick (src: README § Try it; tvmode.js `PADB`). (final audit: verified in code)
- [x] 35. The left stick moves focus like the D-pad once it passes half of its travel (0.5) on an axis (src: README § Try it; tvmode.js `pollPads`).
- [x] 36. The right stick's vertical axis scrolls what you're reading without moving focus, past a 0.25 dead zone, at a speed proportional to the tilt (src: README § Try it; tvmode.js `pollPads`, `stickScroll`).
- [x] 37. The right stick scrolls the Details tab body when Details is open, else the scrolling area holding the focused item, and does nothing while an overlay is open (src: tvmode.js `stickScroll`). (final audit: verified in code)
- [x] 38. The controller's name is its device name with any parenthesised part removed, or "Controller" when that leaves nothing (src: tvmode.js `usePad`).
- [x] 39. When the controller in use disconnects, the next connected controller becomes the one in use; with none left, the name clears (src: tvmode.js `pollPads`, `gamepaddisconnected`).

### Repeat

- [x] 40. A D-pad or left-stick direction acts on press, repeats after 400 ms while held, then every 90 ms (src: README § Try it; tvmode.js `pollPads`).
- [x] 41. Other buttons act once per press and never repeat, except the typing buttons on the on-screen keyboard (src: tvmode.js `pollPads`, `oskRepeats`).
- [x] 42. On the on-screen keyboard, A on a letter, number, symbol, Space or Delete key, and X, Y, LB and RB repeat after 500 ms while held, then every 50 ms (20 times a second) (src: README § Overlays › On-screen keyboard; tvmode.js `oskRepeats`, `pollPads`).
- [x] 43. A held on the Shift key, the symbols key or Done does not repeat (src: README § Overlays › On-screen keyboard; tvmode.js `oskRepeats`).
- [x] 44. Releasing a button ends its repeat at once (src: tvmode.js `pollPads`).

### Multiple controllers

- [x] 45. Each controller keeps its own held-button state, so one controller never cancels or re-fires another's held button (src: tvmode.js `pollPads`).
- [x] 46. Each controller reads its own presses with its own button assignment (src: README § Decided Button assignments). (final audit: verified in code)
- [x] 47. The prompts, the hint bar's controller name and the Button assignments row follow the controller pressed last (a button or the right stick) (src: README § Decided Button assignments; tvmode.js `usePad`).
- [x] 48. When the controller in use changes while Settings › TV mode is showing, the pane redraws to name the new controller (src: tvmode.js `usePad`).

### Button assignments

- [x] 49. Xbox style binds the face buttons as an Xbox pad: bottom is A (select), right is B (back), left is X, top is Y (src: README § Decided Button assignments).
- [x] 50. Nintendo style binds them as a Nintendo pad: right is A (select), bottom is B (back), top is X, left is Y (src: README § Decided Button assignments).
- [x] 51. A controller starts on Nintendo style when it is a Nintendo controller (vendor id 057e, or a name containing Nintendo, Joy-Con or Switch) or an 8BitDo pad (vendor id 2dc8, or a name containing 8BitDo) whose name holds Pro, SN30, Lite, Zero or Micro (src: README § Decided Button assignments; tvmode.js `guessAssign`).
- [x] 52. Every other controller starts on Xbox style: Xbox, PlayStation, the 8BitDo Ultimate, and 8BitDo pads in X-input mode (which report as Microsoft) (src: README § Decided Button assignments; tvmode.js `guessAssign`, `padBrand`).
- [x] 53. A choice made in Settings › TV mode › Button assignments is stored for that controller only and kept (src: README § Decided Button assignments; tvmode.js `paneRows('tv')`).
- [x] 54. A Nintendo-layout pad set to Xbox style has all four face buttons rebound to the Xbox positions, and an Xbox-layout pad set to Nintendo style gets the Nintendo bindings (src: README § Decided Button assignments).
- [x] 55. An assignment rebinds only the four face buttons; shoulders, triggers, View, Menu, the D-pad and the sticks keep their places (src: tvmode.js `pollPads`).
- [x] 56. With no controller, the keyboard and the prompts behave as Xbox style (src: tvmode.js `assignNow`).

### Prompts & glyphs

- [x] 57. Prompts are Kenney's Input Prompts 1.5A glyphs (CC0 1.0), shipped with their `License.txt`, as listed in `docs/design/prototype/glyphs/map.json` (src: README § Decided Glyph art; glyphs/README.md). (final audit: verified in code)
- [x] 58. The Xbox set uses xbox_button_a/b/x/y, xbox_lb/rb/lt/rt, xbox_button_view, xbox_button_menu, xbox_stick_r_vertical (Scroll), xbox_ls and xbox_dpad (src: glyphs/map.json `xbox`). (final audit: verified in code)
- [x] 59. The PlayStation set uses cross/circle/square/triangle for A/B/X/Y, L1/R1/L2/R2 triggers, the DualSense Create (View) and Options (Menu) buttons, playstation_stick_r_vertical, playstation_button_l3 and playstation_dpad (src: glyphs/map.json `ps`; README § Decided Glyph art). (final audit: verified in code)
- [x] 60. The Nintendo set is Switch 2: A/B/X/Y, L/R, ZL/ZR, − (View) and + (Menu), switch_stick_r_vertical, switch_stick_l_press and switch_dpad (src: glyphs/map.json `nintendo`). (final audit: verified in code)
- [x] 61. The 8BitDo (Pro series) set uses the plain letter glyphs, L/R, the rounded L2/R2 (playstation_trigger_l2/r2_alternative) and −/+ (src: glyphs/map.json `8bitdo`; README § Decided Glyph art). (final audit: verified in code)
- [x] 62. Each set uses its own D-pad glyph for "Adjust" and "Read" and its own right-stick glyph for "Scroll" (src: README § Decided Glyph art). (final audit: verified in code)
- [x] 63. Glyphs are drawn as a mask filled with the theme's text colour (`text-h`), `on-primary` inside a primary button and white in the media viewer, so they read in light and dark (src: README § Decided Glyph art; tvmode.css `.gli`). (final audit: verified in code)
- [x] 64. Glyphs are 2.1rem by default, 1.8rem in chips, notifications and the selection bar, and 1.6rem on keyboard keys (src: tvmode.css `.gli`, `.chip .gli`, `.key .gli`). (final audit: verified in code)
- [x] 65. Each glyph carries its button's name as an accessible label (src: tvmode.js `gl`). (final audit: verified in code)
- [x] 66. No brand logo is used (the Kenney Xbox guide glyph is left out), and the Home / Guide button has no glyph (src: README § Decided Glyph art). (final audit: verified in code)
- [x] 67. With Button prompts on Match my controller, Xbox style shows the Xbox set, or the PlayStation set on a PlayStation pad (vendor id 054c, or a name containing PlayStation, DualSense or DualShock) (src: README § Decided Button prompts; tvmode.js `scheme`, `padBrand`).
- [x] 68. With Match my controller, Nintendo style shows the Nintendo set, or the 8BitDo Pro set on an 8BitDo pad (src: README § Decided Button prompts; tvmode.js `scheme`).
- [x] 69. Button prompts set by hand (Xbox, PlayStation, Nintendo, 8BitDo (Pro series)) override the match for every controller (src: README § Decided Button prompts; tvmode.js `scheme`).
- [x] 70. A change of prompt set or assignment redraws the hint bar, the top bar and the open screen at once (src: tvmode.js `srowAct`).

### Keyboard fallback

- [x] 71. A physical keyboard drives TV mode as a controller: arrow keys move, Enter (or Space) is A, Esc or Backspace is B, X is X, Y is Y, Q is LB, E is RB, Z is LT, R is RT, M is Menu, V is View, Page Up / Page Down are the right stick (src: README § Try it; tvmode.js `KEYMAP`).
- [x] 72. A held arrow key repeats like the D-pad; other held keys repeat only where the held controller button would (the on-screen keyboard's typing buttons) (src: tvmode.js `keydown`). (final audit: verified in code)
- [x] 73. Page Up / Page Down scroll the reading area by 70% of its height without moving focus (src: tvmode.js `keydown`). (final audit: verified in code)
- [x] 74. While the on-screen keyboard is open, a printable key types straight in, Backspace deletes before the caret (it does not cancel), and Enter is Done once something was typed on the physical keyboard (src: README § Overlays › On-screen keyboard; tvmode.js `keydown`).

### Mouse

- [x] 75. Pointing at a focusable item moves focus to it without scrolling, but only after the mouse has actually moved (src: README § Try it; tvmode.js `mouseover`). (final audit: verified in code)
- [x] 76. Clicking an item focuses it and does what A does; clicking a hint presses that button; clicking a top-bar tab or a Details tab switches to it; clicking outside a non-blocking overlay closes it (src: README § The shell › Hint bar; tvmode.js `click`, `openLayer`).

## 3. Focus & navigation rules

- [x] 77. Exactly one item has focus at all times, and every screen opens with something focused (src: README § The shell › Focus).
- [x] 78. A screen opens on the item last focused there, else on its designated default (the checked option in a list, Cancel in a confirmation), else on its first item, skipping a disabled item when an enabled one exists (src: tvmode.js `restoreFocus`).
- [x] 79. Each tab and page remembers its focused item, and coming back to it (B from a page, switching tabs and back) returns focus there (src: README § The shell › Focus; tvmode.js `setFocus`, `restoreFocus`).
- [x] 80. Closing an overlay returns focus to the item that had it, without scrolling; if that item was redrawn, to the item with the same identity (src: tvmode.js `closeLayer`). (final audit: verified in code)
- [x] 81. Movement is spatial: only items lying in the pressed direction qualify, and the winner has the lowest score of gap along the direction + 2.2 x offset across it (src: README § The shell › Movement; tvmode.js `move`).
- [x] 82. Rails, rows and lists keep focus inside them along their axis, with no wrapping off the end (src: README § The shell › Movement).
- [x] 83. At a list's end, focus carries on to the nearest item past that end that overlaps the list's column (vertical list) or line (horizontal list), taking its first item in reading order, never sideways into another panel (src: README § The shell › Movement; tvmode.js `move`).
- [x] 84. Down from a list's last item reaches the buttons under it (src: README § The shell › Movement).
- [x] 85. Right from a rail row does what A does: focus goes into the grid or pane beside it, to the item last left there, else its first item (src: README § The shell › Movement; tvmode.js `ACT.railOpen`).
- [x] 86. Moving into a rail always lands on its selected row (the platform, filter or pane you're in), even after a redraw (src: README § The shell › Movement; tvmode.js `move`).
- [x] 87. Moving into a row, pane or button group lands on the item last left there while it still exists, else its first item (src: README § The shell › Movement; tvmode.js `move`).
- [x] 88. Off a grid's edge, focus goes only to what sits straight past that edge; with nothing there, it stays put (src: README § The shell › Movement; tvmode.js `move`).
- [x] 89. Up from a grid's top row reaches Server's header chips and buttons, never the rail; on the Library, Up from the top row stays put (src: README § The shell › Movement).
- [x] 90. Left from a grid's first column goes to the rail's selected row (src: README § The shell › Movement).
- [x] 91. An explicit link overrides geometry: Up from the Related row in Details always returns to the summary (src: README § Pages › Details; tvmode.js `move`, `detBody`).
- [x] 92. B closes the open overlay, unless it is blocking (src: README § Try it; tvmode.js `press`).
- [x] 93. B on a pushed page goes back one page (src: README § Pages; tvmode.js `press`).
- [x] 94. B in select mode leaves select mode before anything else (src: README § Game backups › Select mode; tvmode.js `press`).
- [x] 95. B from a grid, a pane or Discover's content moves focus to the rail's selected row (src: README § Try it; tvmode.js `press`).
- [x] 96. B at a tab's top level (on a rail, on Downloads, on a Server panel with no rail) does nothing (src: README § Try it; tvmode.js `press`).
- [x] 97. B on the startup screens only steps back out of a page pushed from them (the folder browser), never past the first startup screen (src: tvmode.js `press`).
- [x] 98. LB / RB at a tab's top level switch tabs in the order Library, Server, Downloads, Settings, wrapping at the ends (src: README § Try it; tvmode.js `press`, `TAB_ORDER`).
- [x] 99. LB / RB do nothing on pages and startup screens, except where the page uses them (Details tabs, the media viewer, the on-screen keyboard caret) (src: tvmode.js `press`).
- [x] 100. Start presses a page's main button where the page has one: Skip for now on Choose your systems, Backup N games on the Backup page, Set up for GRID on a drive's setup page (src: README § Decided Start).
- [x] 101. While a page's main button is disabled, Start shows an error notice with the reason, as A on the button does (src: README § Decided Start; tvmode.js `rowsPage().menu`).
- [x] 102. Elsewhere Start opens the Menu (src: README § Decided Start).
- [x] 103. While an overlay is open, X, Y, LT, RT, LB, RB, Menu and View do nothing unless that overlay binds them (the on-screen keyboard, the media viewer) (src: tvmode.js `press`).
- [x] 104. Arriving on the Server tab while Discover is selected focuses Discover's content (the cover last focused there, else the first), not the rail (src: README § Tabs › Server › Discover; tvmode.js `render`).
- [x] 105. Moving through the Library, Server or Settings rail switches the grid or pane beside it at once, without A (src: README § Tabs › Library; tvmode.js `ONF`).
- [x] 106. A rail's selected row shows the `selected` fill and a 0.25rem `primary` bar on its left edge (src: tvmode.css `.ri.sel`). (final audit: verified in code)
- [x] 107. Scrolling follows fixed rules and never the browser's "just enough" scroll-into-view (src: README § The shell › Scrolling). (final audit: verified in code)
- [x] 108. On Server › Discover, moving into a shelf scrolls its title to the top of the rows area, and the first shelf scrolls to the very top (src: README § Scrolling; tvmode.js `reveal`).
- [x] 109. Discover's hero has a fixed height, so the rows area never changes size as focus moves (src: README § Scrolling; tvmode.css `.hero`).
- [x] 110. A row that scrolls sideways keeps the focused cover a full row margin inside its left and right edges (src: README § Scrolling; tvmode.js `reveal`).
- [x] 111. In grids, lists, panes and menus, the focused item stays 1.2rem inside the scrolling area; an item taller than the area lines up with its top (src: README § Scrolling; tvmode.js `reveal`).
- [x] 112. Focusing an area's first item scrolls it to the very top and its last item to the very bottom, so a heading above or a note below ("No Backup drives connected") is never stranded (src: README § Scrolling; tvmode.js `reveal`).
- [x] 113. A redraw (an install starting, coming back from Details, switching tabs and back) keeps every scroll position of every rail, row, grid, list and pane on that screen (src: README § Scrolling; tvmode.js `renderStage`). (final audit: verified in code)
- [x] 114. Scrolls glide; with reduced motion, and when a screen first appears or redraws, they jump (src: README § Scrolling; tvmode.js `glide`, `render`).
- [x] 115. After a change of data, focus stays on the same item; if it is gone, focus moves to the enabled item now in its place in the same panel (src: README § The shell › Focus; tvmode.js `repaint`).
- [x] 116. A change never moves the player: in Settings only the pane redraws, never the rail (src: README § The shell › Focus; tvmode.js `repaint`). (final audit: verified in code)
- [x] 117. A redraw caused by a finished install while an overlay is open happens in place; closing the overlay returns focus to its opener, or jumps to the item with the opener's key or the one now in its place (consultant ruling G; src: tvmode.js tick, `closeLayer`).
- [x] 118. While an install runs, only its percentage text changes (cover tags, hero line, top bar), so nothing else redraws (src: tvmode.js tick). (final audit: verified in code)

## 4. Top bar & hint bar

### Top bar

- [x] 119. The top bar is 4.6rem tall on `panel` with a bottom `line` border (src: README § The shell › Top bar; tvmode.css `.top`). (final audit: verified in code)
- [x] 120. Its left end is the GRID icon in `primary`, the "GRID" wordmark and a "TV" pill tag (src: README § Top bar; tvmode.js `paintTop`). (final audit: verified in code)
- [x] 121. At a tab's top level the middle shows the LB glyph, the tabs Library, Server, Downloads, Settings and the RB glyph, centred, with the current tab a `primary` pill (src: README § Top bar; tvmode.js `paintTop`; tvmode.css `.tab.on`). (final audit: verified in code)
- [x] 122. On a pushed page the tabs give way to a breadcrumb: the B glyph, the tab name, "›" and the page title in bold, for example "(B) Library › Starfall Circuit" (src: README § Top bar; tvmode.js `paintTop`). (final audit: verified in code)
- [x] 123. On the startup screens the middle is empty and the right shows only the clock (src: tvmode.js `paintTop`).
- [x] 124. The right shows the live download while one runs: a download icon, its title, a 4rem progress bar and its percent (src: README § Top bar; tvmode.js `paintTop`).
- [x] 125. Then the connection: a green dot and the user name when connected; a red dot and "Offline" when the server can't be reached; a grey dot and "Offline · no server" when GRID is used without a server (src: README § Top bar; README § Game backups › Use without a server; tvmode.js `paintTop`).
- [x] 126. Then a clock (HH:MM, tabular figures) that stays current to the minute (src: README § Top bar; tvmode.js `paintTop`).

### Hint bar

- [x] 127. The hint bar is 3.6rem tall at the bottom on `panel` with a top `line` border, and overlays never cover it (src: README § The shell › Hint bar; tvmode.css `.hints`, `.layer`). (final audit: verified in code)
- [x] 128. Its right side shows what each button does now for the focused item, as the controller's glyph and a label, in the order A, X, the screen's own hints, B "Back" (on pages), Menu "Menu" (src: README § Hint bar; tvmode.js `hintsNow`).
- [x] 129. The A hint names the focused item's action ("Details" on a cover, "Open" on a rail row, "Switch", "Select" or "Edit" on a settings row, …), and is left out when the item is disabled (src: tvmode.js `hintsNow`, `srow`).
- [x] 130. The X hint on a cover is its quick action: "Play" when installed, "Install from drive" on a drive's grid, "Install" otherwise; none while it is queued or installing, when it is Missing or unlinked, or for a game not installed while offline (src: README § Try it; tvmode.js `quickLabel`).
- [x] 131. The X hint elsewhere is the item's own extra action ("Change emulator", "Install <emulator>", "Backup all N…") (src: tvmode.js `hintsNow`).
- [x] 132. The Menu hint is left out on startup screens and where Start presses the page's main button (src: tvmode.js `hintsNow`).
- [x] 133. On startup screens there is no Back hint (src: tvmode.js `hintsNow`).
- [x] 134. Every hint is clickable and does what pressing its button does (src: README § Hint bar; tvmode.js `click`). (final audit: verified in code)
- [x] 135. The left side shows the name of the controller in use (src: README § Hint bar; tvmode.js `paintHints`).
- [x] 136. While the server can't be reached, the left side shows an "Offline" danger pill and "Your installed games still play. Server features return when GRID reconnects." (src: README § Hint bar; tvmode.js `paintHints`).
- [x] 137. Without a server, the left side reads "No server. Your Library and Backup drives work without one." (src: README § Game backups › Use without a server; tvmode.js `paintHints`).
- [x] 138. The hints repaint on every focus change, every state change and when a notification leaves (src: tvmode.js `setFocus`, `toast`). (final audit: verified in code)

## 5. Notifications & setup card

- [x] 139. Notifications are the desktop's events with the desktop's wording (src: README § The shell › Notifications). (final audit: verified in code)
- [x] 140. They show as cards at the top right under the top bar (5.4rem from the top, 2.4rem from the right), 26rem wide, stacked with a 0.7rem gap (src: README § Notifications; tvmode.css `.toasts`).
- [x] 141. At most three cards show; a fourth drops the oldest card that isn't pinned (src: README § Notifications; tvmode.js `toast`).
- [x] 142. The newest card goes on top, under the pinned setup card when one shows (src: tvmode.js `toast`).
- [x] 143. Cards never take focus and don't react to pointing (src: README § Notifications; tvmode.css `.toasts`). (final audit: verified in code)
- [x] 144. A card shows its title (semibold), an optional second line, and, when it offers an action, the View glyph and the action name ("Open Downloads") (src: README § Notifications; tvmode.js `toast`).
- [x] 145. An error card has a `danger` border and title and is announced as an alert; other cards are announced as status (src: tvmode.js `toast`; tvmode.css `.toast.err`).
- [x] 146. A card closes on its own after 10 s, or 15 s for an error, with a countdown bar along its bottom (`primary`, `danger` on errors) (src: tvmode.js `toast`; tvmode.css `.toast .tbar`).
- [x] 147. A card slides in from 1rem to the right over 220 ms (src: tvmode.css `@keyframes tin`). (final audit: verified in code)
- [x] 148. View acts on the newest card that offers an action: the card closes and its action runs (src: README § Try it; tvmode.js `press`, `newestAction`).
- [x] 149. "Open Downloads" closes any pages and shows the Downloads tab (src: tvmode.js `install`).
- [x] 150. Every card also goes to Menu › Notifications with its time (src: README § Notifications; tvmode.js `toast`).
- [x] 151. After Install N emulators on Choose your systems, the setup card follows the installs, pinned to the top of the stack and never dropped (src: README § Startup screens › Choose your systems; tvmode.js `setupCard`).
- [x] 152. The setup card has one segment per install, each emulator once, empty while it waits (src: README § Choose your systems; tvmode.js `ACT.chGo`, `setupCard`).
- [x] 153. A segment's first half fills while that emulator downloads and its second half while it installs; it is full when done and red if it failed (src: README § Choose your systems; tvmode.css `.segs i.fail`).
- [x] 154. The setup card's title reads "Setting up N installs · K done" ("1 install" for one), then "N installs done" (src: tvmode.js `setupCard`).
- [x] 155. A line under the bar names the step: "<Emulator> · downloading 62%", then "<Emulator> · installing…" (src: README § Choose your systems; tvmode.js `setupCard`).
- [x] 156. The setup card has no countdown while anything still installs; once the last install is done, the line reads "N systems are ready to play." ("1 system is ready to play.") and the card closes after 10 seconds (src: README § Choose your systems; tvmode.js `setupCard`).

## 6. Server › Discover

- [x] 157. Discover is the first row of the Server rail, above a divider, a "PLATFORMS" heading and the platforms (src: README § Tabs › Server › Discover; tvmode.js `TABS.server.render`).
- [x] 158. Discover shows what TV mode's Home tab showed, unchanged for now: a hero over rows that scroll sideways (src: README § Decided No Home tab).
- [x] 159. The hero is a fixed 13.6rem tall inside the Server tab (src: README § Scrolling; tvmode.css `#srv-main .hero`).
- [x] 160. The hero panel (up to 46rem wide) shows the focused game's platform (uppercase, muted), its title as a logo (2.6rem, one line, ellipsis), a played / install state line and two lines of summary (src: README § Discover; tvmode.js `hero`; tvmode.css `.hero-box`, `.logo`, `.hero .sum`).
- [x] 161. The hero line reads "Files not found" for a Missing game; "Played <when> · <emulator>" (or "Installed · <emulator>", "No emulator" when none) for an installed game; "Installing… <n>%" or "Queued" while installing; else "Released <d Mon yyyy> · Not installed" (src: tvmode.js `hero`).
- [x] 162. "<when>" reads "today", "yesterday", "<n> days ago" under 14 days, else "<n> weeks ago" (src: tvmode.js `ago`).
- [x] 163. The hero follows focus, and shows the first cover's game when the tab opens (src: tvmode.js `setFocus`, `srvPaint`, `render`).
- [x] 164. The rows, in order: Continue playing; New releases with "the last six months, on your systems"; Recently added with "to your server"; Updates with "<n> games", only when there are any; Your library with "<n> games" (src: README § Discover; tvmode.js `discoverHTML`).
- [x] 165. Continue playing holds installed games with play history, most recently played first (src: tvmode.js `discoverHTML`).
- [x] 166. New releases holds server games released in the last six months on the systems you play, newest first (src: README § Discover; tvmode.js `discoverHTML`).
- [x] 167. Recently added holds games newly added to the server, newest first, leaving out those already in New releases (src: tvmode.js `discoverHTML`).
- [x] 168. Updates holds installed games with an update; Your library holds every installed game (src: tvmode.js `discoverHTML`).
- [x] 169. An empty row shows "Nothing here yet." (src: tvmode.js `discoverHTML`).
- [x] 170. Row titles are 1.25rem semibold, with their extra text muted at 1rem (src: tvmode.css `.shelf h2`).
- [x] 171. Discover covers carry platform tags (src: tvmode.js `discoverHTML`).
- [x] 172. A on a cover opens Details, X plays or installs it, Y opens Search everything (src: README § Discover).
- [x] 173. Select mode and Backup all aren't offered on Discover: RT does nothing and no Select hint shows (src: README § Discover; tvmode.js `TABS.server.rt`, `hints`).
- [x] 174. Up from Discover's top row and Down from its bottom row do nothing (never the rail); Left from a row's first cover goes to the rail (src: README § Discover; tvmode.js `move`).
- [x] 175. The Discover hints are Y "Search" (plus A and X for the cover) (src: tvmode.js `TABS.server.hints`).

## 7. Library

- [x] 176. The Library rail lists All games, Recent and Updates, a divider, then two folders: Platforms (each installed platform) and Backup drives (src: README § Tabs › Library; tvmode.js `libRailHTML`).
- [x] 177. Every filter row carries its game count: All games counts installed games, Recent those played in the last 30 days, Updates those with an update, each platform its installed games (src: README § Library; tvmode.js `libRailHTML`).
- [x] 178. Moving through the rail filters the grid at once, and changing the filter clears the Library search (src: README § Library; tvmode.js `ONF.lib`).
- [x] 179. A or Right on a filter row moves into the grid (src: README § Library).
- [x] 180. A folder's header row shows a chevron and the folder name; A opens and closes it, the chevron turns 90° when open, and the A hint reads "Open" or "Close" (src: README § Library; tvmode.js `folder`; tvmode.css `.ri.fold .chev.open`).
- [x] 181. Right on a folder header still goes to the grid (src: README § Library).
- [x] 182. A closed folder shows no rows, and the grid's header still names the filter (src: README § Library).
- [x] 183. Both folders start open, and each stays as you left it for the session (src: README § Library; tvmode.js `S.folds`).
- [x] 184. Rows inside a folder are indented (2.3rem) under its header (src: tvmode.css `.ri.sub`).
- [x] 185. The header shows the filter name (1.6rem), "<n> games" ("1 game"), a "Search: <text>" chip while searching, then on the right chips for RT "Select", LT "Sort: <order>" and Y "Search" (src: tvmode.js `libPaint`).
- [x] 186. The grid shows large covers; covers carry platform tags in All games, Recent and Updates, and not under a single platform (src: README § Library; tvmode.js `libPaint`).
- [x] 187. Y opens the on-screen keyboard titled "Search your library", and the grid narrows to titles containing the text as you type (src: README § Library; tvmode.js `TABS.library.y`).
- [x] 188. LT opens the "Sort by" list: Recently played (the default), Recently installed, Title, Platform (src: README § Library; tvmode.js `sortSheet`).
- [x] 189. A search with no match shows "No games match “<text>”" and "Press (Y) to change the search." (src: tvmode.js `libPaint`).
- [x] 190. An empty Updates filter shows "Everything is up to date" and "Games with a newer copy on your server show here." (src: tvmode.js `libPaint`).
- [x] 191. Another empty filter shows "No games here yet" and "Install games from the Server tab." (src: tvmode.js `libPaint`).
- [x] 192. A on a cover opens Details; X is its quick action (Play, or Install) (src: README § Try it).
- [x] 193. The Library hints are Y "Search", LT "Sort", RT "Select" (src: tvmode.js `TABS.library.hints`).
- [x] 194. While a game installs, its covers show "Queued" or the live percent (src: README § Library; tvmode.js `cover`, tick).
- [x] 195. When an install finishes, the tag goes and the installed dot comes on every cover of that game at once, wherever focus is (src: README § Library; tvmode.js tick).

## 8. Server platforms

- [x] 196. Under Discover, the Server rail lists the server's platforms, each with its game count (src: README § Tabs › Server; tvmode.js `TABS.server.render`).
- [x] 197. A platform's header reads "<n> games · <k> installed", or "<m> of <n> games match · <k> installed" while a search narrows the grid; every count is the number of games the grid shows (src: README § Server; tvmode.js `srvCount`).
- [x] 198. The header's emulator chip reads "Emulator: <name>", or "No default emulator" in the warning style (src: README § Server; tvmode.js `srvPaint`).
- [x] 199. The header's firmware chip reads "Firmware: <n> files · installed" or "Firmware: <n> files", or a plain unfocusable "No server firmware" (src: tvmode.js `srvPaint`).
- [x] 200. A on the emulator or firmware chip (hint "Open in Systems") opens that system's page (src: README § Server; tvmode.js `srvPaint`).
- [x] 201. Native platforms (Windows, Linux) have no emulator or firmware chips (src: tvmode.js `srvPaint`).
- [x] 202. The header also has a "Search: <text>" chip while searching, then on the right the "Backup all N…" button (when the platform has games), and RT "Select" and Y "Search" chips (src: tvmode.js `srvPaint`).
- [x] 203. Server grid covers carry no platform tag (src: tvmode.js `srvPaint`).
- [x] 204. Y opens the keyboard titled "Search <Platform>", and the grid narrows as you type; changing the platform clears the search (src: tvmode.js `TABS.server.y`, `ONF.srv`).
- [x] 205. A search with no match shows "No games match “<text>”" (src: tvmode.js `srvPaint`).
- [x] 206. The hints on a platform are Y "Search" and RT "Select" (src: tvmode.js `TABS.server.hints`).
- [x] 207. Offline, the Server tab shows one panel: "Can't reach your server", "<server URL> isn't answering. Your installed games still play from the Library.", with Try again (primary) and Open Library (src: README § Server; tvmode.js `TABS.server.render`).
- [x] 208. Try again reconnects ("Reconnecting to <URL>…", then "Connected to <URL>"), and Open Library shows the Library tab (src: tvmode.js `reconnect`, `ACT.gotab`).

## 9. Downloads

- [x] 209. Downloads has three panels, Active, Queued and Completed, each heading with its row count, in a centred column up to 70rem wide (src: README § Tabs › Downloads; tvmode.js `TABS.downloads.render`; tvmode.css `.list-in`).
- [x] 210. An empty panel reads "Nothing active.", "Nothing queued." or "Nothing completed." (src: tvmode.js `seg`).
- [x] 211. Completed holds finished, failed and cancelled rows (src: tvmode.js `seg`).
- [x] 212. Each row shows the title with its kind badge (Update, Backup, Library), a status line "<Platform> · <status>", a progress bar while active, the network / disk sparkline and the speed (src: README § Downloads; tvmode.js `row`).
- [x] 213. An active download's status reads "Downloading · <n>% · <speed> · <time> left"; a queued one "Queued" (src: tvmode.js `row`).
- [x] 214. A failed row's status is in the `danger` colour ("Failed · <reason>"); a cancelled one reads "Cancelled" (src: tvmode.js `row`, `dlrow`).
- [x] 215. The sparkline draws network in `primary` and disk in `graph-disk` (src: tvmode.js `row`).
- [x] 216. The rows form one vertical list per panel, and the A hint reads "Cancel" (active, queued), "Retry or dismiss" (failed) or "Dismiss" (finished) (src: tvmode.js `row`).
- [x] 217. A opens the row's actions as a list titled with the row's title over "<Platform> · <kind>": Cancel (danger) while active or queued, Retry when failed or cancelled, Dismiss when finished, failed or cancelled, and "Open <title>" for a game (src: README § Downloads; tvmode.js `ACT.dlrow`).
- [x] 218. Cancel stops the row ("Cancelled <title>") and the next queued row starts (src: tvmode.js `ACT.dlrow`).
- [x] 219. Retry queues the row again ("<title> added to download queue") (src: tvmode.js `ACT.dlrow`).
- [x] 220. Dismiss removes the row and focus moves to the row now in its place (src: tvmode.js `ACT.dlrow`).
- [x] 221. "Open <title>" opens the game's Details over Downloads (src: tvmode.js `ACT.dlrow`).
- [x] 222. Y clears every finished, failed and cancelled row; the hint Y "Clear completed" shows only when there are any (src: README § Downloads; tvmode.js `TABS.downloads`).
- [x] 223. Progress updates while Downloads shows keep focus and scroll (src: tvmode.js tick).
- [x] 224. B on Downloads does nothing (src: README § Try it).

## 10. Settings

- [x] 225. The Settings rail lists the panes in this order: Appearance, Backup drives, Cloud saves, Connection, Library, RetroAchievements, Systems, TV mode, Updates (src: README § Tabs › Settings; README § Game backups › Settings › Backup drives; tvmode.js `PANES`).
- [x] 226. Moving through the rail shows each pane at once; A or Right moves into the pane (src: README § Tabs › Library "The same holds for every rail"; tvmode.js `ONF.set`).
- [x] 227. Each pane starts with its name as a heading (1.6rem) (src: tvmode.js `setPaint`).
- [x] 228. A switch row flips with A (hint "Switch") (src: README § Settings; tvmode.js `srowAct`).
- [x] 229. A choice row shows its current value and a chevron, and A (hint "Select") opens a list with a check on the current value (src: README § Settings; tvmode.js `srow`, `srowAct`).
- [x] 230. A slider row shows its bar and value; Left / Right adjust it by its step while it is focused, ◀ ▶ arrows show only while it is focused, and the hint reads D-pad "Adjust" (src: README § Settings; tvmode.js `srowAdj`, `TABS.settings.hints`; tvmode.css `.adj-arrows`).
- [x] 231. A text row shows its value, "Not set" in italics when empty, or dots when masked, and A (hint "Edit") opens the on-screen keyboard (src: README § Settings; tvmode.js `srow`, `srowAct`).
- [x] 232. Information rows aren't focusable; action rows show a chevron, and a dangerous action's label is in `danger` (src: tvmode.js `srow`; tvmode.css `.srow.dng`).
- [x] 233. Every change applies at once and redraws only the pane, keeping focus on the row (src: README § The shell › Focus; tvmode.js `repaint`).
- [x] 234. Appearance › Open GRID on is a choice with the line "Here and in desktop mode" and the options "Automatic: Discover when connected, else Library", "Server › Discover" and "Library" (src: README § Decided Where GRID opens; tvmode.js `paneRows('appearance')`).
- [x] 235. Appearance › Theme is a choice: Follow system, Dark, Light (src: tvmode.js `paneRows('appearance')`).
- [x] 236. Appearance › Background art is a switch with "The focused game's art, blurred, behind everything" (src: tvmode.js `paneRows('appearance')`).
- [x] 237. Appearance › Background fade is a slider from 0 to 100 in steps of 5, shown as "<n>%" and "100% · off" at 100 (src: tvmode.js `paneRows('appearance')`).
- [x] 238. Appearance › Background blur is a slider from 0 to 100 in steps of 10, shown as "<n>%" (src: tvmode.js `paneRows('appearance')`).
- [x] 239. Appearance › Hardware acceleration is a switch with "Renders on the graphics card. Applies after a restart." (src: tvmode.js `paneRows('appearance')`).
- [x] 240. Appearance ends with the information row "Card size, game labels and card display" · "Desktop grids only" (src: README § Decided Implementation; tvmode.js `paneRows('appearance')`).
- [x] 241. Cloud saves has the switches Restore before launch ("Fetch the latest cloud save before a game starts"), Upload after exit ("Send new saves when a game closes") and Skip when the local save is newer (src: tvmode.js `paneRows('cloud')`).
- [x] 242. Cloud saves has the sliders Upload delay (0–60 in steps of 5, "<n> s") and Saves to keep per game (0–50 in steps of 1, "Unlimited" at 0) (src: tvmode.js `paneRows('cloud')`).
- [x] 243. Connection shows Server, User and Credential ("Stored in the system keyring · verified", or "· not verified" while offline), then Reconnect and Disconnect (src: tvmode.js `paneRows('connection')`).
- [x] 244. Disconnect (danger, "Returns to Connect. Your library and settings stay.") asks "Disconnect from <URL>?" / "GRID returns to Connect. Installed games, saves and settings stay on this computer." with Disconnect (danger), then shows Connect (src: tvmode.js `paneRows('connection')`).
- [x] 245. Library shows Library folder and the action Change library folder… ("Move your games, emulators and saves, or start fresh"), which opens the folder browser titled "Choose the new library folder" (src: tvmode.js `paneRows('library')`, `changeLibrary`).
- [x] 246. Picking the current library folder or a folder inside it shows the error "Can't use that folder" / "It is the current library or inside it." (src: tvmode.js `changeLibrary`).
- [x] 247. Picking another folder opens the list "Move to <path>?" (with the desktop's copy summary) offering Move existing files ("Runs in Downloads") and Start fresh ("Leave or delete the old files") (src: tvmode.js `changeLibrary`).
- [x] 248. Move existing files adds a Library row to Downloads and notifies "Moving the library to <path>…" / "Installs and launches wait until it finishes", then "Library moved" (src: tvmode.js `changeLibrary`, tick).
- [x] 249. Start fresh asks in a list "Delete the old game files, or leave them on disk?" with Leave them on disk and Delete them (<size>) (danger); Delete asks "Delete <size> of game files?" / "games/ in <old folder> is removed. Emulators and saves stay. This can't be undone." with Delete (danger) (src: tvmode.js `changeLibrary`).
- [x] 250. RetroAchievements has Username (text), Token (masked text, "Write-only: GRID never shows it again", the keyboard opens empty), Save ("Sets the login in every emulator that supports it") and Clear (danger) (src: tvmode.js `paneRows('ra')`).
- [x] 251. Saving the RetroAchievements login notifies "RetroAchievements login saved"; Clear notifies "RetroAchievements login cleared" (src: tvmode.js `paneRows('ra')`).
- [x] 252. Systems starts with its heading and the buttons "Install recommended (<n>)" (primary, when systems lack an emulator), Check for updates and Add manually (src: README § Settings; tvmode.js `systemsPane`).
- [x] 253. While Install recommended runs, its button reads "Installing <n> emulators…" ("1 emulator") and can't be pressed again (src: README § The shell › Focus; tvmode.js `systemsPane`).
- [x] 254. When Install recommended goes away (nothing missing), focus moves to the control now in its place in the same panel (src: README § The shell › Focus; tvmode.js `repaint`).
- [x] 255. Systems lists systems with no emulator first, then those with one, then Windows (Proton) (src: tvmode.js `systemsPane`).
- [x] 256. A system row shows its name, its emulator (and core) or "Games for this system can't start yet. <Emulator> is recommended.", and a status pill: "Ready" with a green dot, or the status in the warning style (src: tvmode.js `systemsPane`).
- [x] 257. A on a system row opens its System page (hint "Open"); X is "Change emulator", or "Install <recommended>" for a system with none (src: README § Try it; tvmode.js `systemsPane`).
- [x] 258. Systems ends with an "Other emulators" group listing installed emulators no system uses, or the note "Every installed emulator is used by a system." (src: tvmode.js `systemsPane`).
- [x] 259. Check for updates notifies "Checking <n> emulators for updates…", then the result (src: tvmode.js `ACT.checkUpd`).
- [x] 260. Installing an emulator notifies "Installing <emulator>…", then "<emulator> installed" / "Used for <system>" (src: tvmode.js `installEmu`).
- [x] 261. TV mode › Button prompts is a choice with "Which controller the on-screen buttons look like" and the options Match my controller, Xbox, PlayStation, Nintendo, 8BitDo (Pro series) (src: README § Decided Button prompts; tvmode.js `paneRows('tv')`).
- [x] 262. TV mode › Button assignments is a choice whose line names the controller it applies to ("For <controller>", or "For the controller in use" with none), with the options Xbox style and Nintendo style (src: README § Decided Button assignments; tvmode.js `paneRows('tv')`).
- [x] 263. The Button assignments list is subtitled with the controller's name and describes each option: Xbox style "Bottom selects, right goes back", Nintendo style "Right selects, bottom goes back" (src: tvmode.js `srowAct`).
- [x] 264. The Button assignments row shows the style in effect for the controller in use, its own choice or its default (src: README § Decided Button assignments; tvmode.js `assignNow`).
- [x] 265. TV mode › Controller shows the controller in use, or "None found · the keyboard works too" (src: tvmode.js `paneRows('tv')`).
- [x] 266. TV mode › Switch to desktop mode ("Also in the Menu. To come back: the TV button in the desktop top bar, or start GRID with --tv") leaves TV mode (src: README § Decided Entering and leaving; tvmode.js `paneRows('tv')`).
- [x] 267. TV mode › Quit GRID asks the Quit confirmation (src: tvmode.js `paneRows('tv')`).
- [x] 268. Updates shows Version and Last checked, an action row "GRID <version> is out" ("GRID only checks. Update it the way you installed it.") when a newer release exists, and a Debug logging switch (src: tvmode.js `paneRows('updates')`).
- [x] 269. The update row opens a list titled "GRID <version>" with the release link as its line, offering Open release in desktop mode and Dismiss this notice ("Update notice dismissed") (src: tvmode.js `paneRows('updates')`).

## 11. Details

- [x] 270. A on a cover pushes Details, titled with the game in the breadcrumb (src: README § Pages › Details; tvmode.js `ACT.details`).
- [x] 271. Details has a 17rem left column with the cover and a status line, and a right panel with the title (2.2rem), the header line, chips, one row of buttons, then the tabs (src: README § Details; tvmode.css `.det`).
- [x] 272. The left line reads "Last played <when or never>" and "Launches with <emulator or no emulator>" for an installed game (src: tvmode.js `details`).
- [x] 273. For a game not installed whose system has no emulator it reads "No emulator for <system>. Install installs <recommended> too."; otherwise "Not installed · <size>" (src: tvmode.js `details`).
- [x] 274. The header line reads "<Platform> · <year> · <developer> · <genres> · <rating>" (src: tvmode.js `details`).
- [x] 275. The chips include "Installed" when installed and "Update available" (warning style) when an update exists, beside the desktop's other chips (src: tvmode.js `details`).
- [x] 276. The buttons form one horizontal row, left to right in the order listed for each state (src: README § Details; tvmode.js `details`).
- [x] 277. An installed game's buttons are Play (primary, play icon), Update (when one exists), Game settings (native games), Uninstall (danger), Backup to drive… (src: README § Details; tvmode.js `details`).
- [x] 278. A game not installed has Install (primary, download icon; "Install app" for a native game) and Backup to drive… (src: README § Details; tvmode.js `details`).
- [x] 279. Offline, Install is disabled (src: tvmode.js `details`).
- [x] 280. While the game is queued or installing, the first button is "Cancel install · <n>%" or "Cancel · queued" (src: README § Details; tvmode.js `details`).
- [x] 281. A Missing game has Locate… (primary), Remove from library (keeps files) and Backup to drive…; the left line reads "Files not found. Play and Uninstall are unavailable until the files are found." (src: README § Details; tvmode.js `details`).
- [x] 282. An unlinked game has Link to server game (primary) and Remove from library (keeps files), and no Backup to drive…; the left line reads "Not linked to a server game. Play, cloud saves and updates need the link." (src: README § Details; tvmode.js `details`).
- [x] 283. Uninstall and Remove buttons use the danger style (src: tvmode.js `details`).
- [x] 284. Cancel install asks "Cancel <title>?" / "The download stops and its files are removed. You can install it again later." with Cancel install (danger), then notifies "Cancelled <title>" (src: tvmode.js `ACT.cancelDl`).
- [x] 285. Update asks "Update <title>?" / "Saves and configuration are kept." with Update, then notifies "Update for <title> added to download queue" (src: tvmode.js `ACT.update`).
- [x] 286. Uninstall asks "Uninstall <title>?" / "Its files are deleted from this computer. Cloud saves stay on your server." with Uninstall (danger), then notifies "<title> uninstalled" (src: tvmode.js `ACT.uninstall`).
- [x] 287. Remove from library asks "Remove <title> from the library?" / "Only the library entry goes. Its folder stays on disk: <path>." with Remove from library (danger), then notifies "<title> removed from the library" and goes back (src: tvmode.js `ACT.removeRow`).
- [x] 288. Install adds the game to the queue and notifies "<title> added to download queue" (with "<emulator> installs with it" when the system has no emulator) and the action Open Downloads (src: tvmode.js `install`).
- [x] 289. Install while the server can't be reached notifies the error "Couldn't add <title> to the download queue: the server can't be reached" (src: tvmode.js `install`). (final audit: verified in code)
- [x] 290. Locate… opens the folder browser titled "Where is <title>?"; a good pick notifies "<title> found" / "Plays from <path>", a wrong one the error "Couldn't find <title> in <path>" / "Pick the folder that holds the game." (src: tvmode.js `ACT.locate`).
- [x] 291. The tabs are Overview, Media, Saves, and Achievements when the server lists any, between LB and RB glyphs; the current tab is underlined in `primary` (src: README § Details; tvmode.js `details`; tvmode.css `.ttab.on`).
- [x] 292. LB / RB switch Details tabs, wrapping at the ends (src: README § Details; tvmode.js `detTabStep`).
- [x] 293. Switching tabs focuses the new tab's first item, with the tab scrolled to its top; a tab with nothing to focus puts focus on the first button (src: README § Details; tvmode.js `detShowTab`).
- [x] 294. The Details hints are RS "Scroll", or D-pad "Read" while the summary is focused (src: tvmode.js `details().hints`).
- [x] 295. Overview shows the summary, a "Related" row with its count (up to six games from the same platform), and a facts panel (Developer, Release, Genres, Players, Size) (src: README § Details; tvmode.js `detBody`).
- [x] 296. Down from the buttons focuses the summary (hint A "Read") (src: README § Details).
- [x] 297. While the summary is focused, Down scrolls the tab by up to 6rem while more of the summary is below the view, and Up while more is above; only then does focus move on (Related below, the buttons above) (src: README § Details; tvmode.js `press`).
- [x] 298. Up from the Related row always returns to the summary (src: README § Details).
- [x] 299. Moving up into the buttons scrolls the tab back to its top (src: README § Details; tvmode.js `setFocus`).
- [x] 300. From anywhere in Details, the right stick scrolls the summary, Related row and facts panel together as one page, without moving focus (src: README § Details; tvmode.js `stickScroll`).
- [x] 301. Media shows a "Screenshots" row and a "Videos" row, each with its count; tiles are 8rem tall at each image's own aspect ratio, and a video tile shows a play icon (src: README § Details; tvmode.js `detBody`).
- [x] 302. A on a tile (hint "View", or "Play video") opens the fullscreen viewer on it (src: README § Details; tvmode.js `detBody`).
- [x] 303. The viewer is black and fullscreen, shows the image at 70% of the screen height and a caption "(LB) Screenshot <i> of <n> · <title> (RB)" ("Trailer · <title>" for the video) (src: tvmode.js `viewer`; tvmode.css `.viewer`).
- [x] 304. In the viewer, LB / RB (and Left / Right) step through the screenshots and videos as one sequence, stopping at the ends; B closes it (src: README § Details; tvmode.js `viewer`).
- [x] 305. The viewer hints are LB "Previous", RB "Next", B "Close" (src: tvmode.js `viewer`).
- [x] 306. Saves starts with a Saves | States switch, the heading "Cloud Saves" or "Cloud States" and the button "Upload latest save" or "Upload latest state" (src: README § Details; tvmode.js `detBody`).
- [x] 307. Each save row shows the emulator, "<when> · <size>" (plus " · slot <n>" for a state), and the newest carries a LATEST badge (src: README § Details; tvmode.js `detBody`).
- [x] 308. A on a save row (hint "Restore or delete") opens a list titled "Save from <when>" ("State from …") with Restore to this computer and Delete from the server (danger) (src: README § Details; tvmode.js `ACT.saveRow`).
- [x] 309. Restore asks "Restore this save?" / "It replaces the save for <title> on this computer. The one there now is overwritten." with Restore, then notifies "Save restored for <title>" (src: tvmode.js `ACT.saveRow`).
- [x] 310. Delete asks "Delete this save?" / "It is removed from your server. This can't be undone." with Delete (danger), then notifies "Save deleted from the server" (src: tvmode.js `ACT.saveRow`).
- [x] 311. A deleted entry leaves the list, focus moves to the entry now in its place, and the newest one left is marked Latest (src: README § Details; tvmode.js `repaint`).
- [x] 312. Upload notifies "Uploading the latest save…", then "Save uploaded for <title>", and the upload appears at the top of the list as Latest (src: README § Details; tvmode.js `ACT.uploadSave`).
- [x] 313. An empty list reads "No cloud saves for this game. Upload the latest one, or play and GRID uploads it when the game closes." ("states" for States) (src: README § Details; tvmode.js `detBody`).
- [x] 314. Saves on an unlinked game reads only "Cloud saves need the link to a server game."; on a game never installed or played, "No cloud saves yet. They appear after you play." (src: tvmode.js `detBody`).
- [x] 315. Achievements shows a summary line ("<k> of <n> unlocked · <p> of <total> points · <h> hardcore") and a list of rows with the badge (locked ones grey and faded), name, "Unlocked <when>" (with " · Hardcore") or the description, and "<n> pts" (src: tvmode.js `detBody`; tvmode.css `.badge.locked`).
- [x] 316. Play notifies "Launching <title>…" (with "Restored the latest cloud save" when Restore before launch is on) and pushes Playing (src: tvmode.js `play`).
- [ ] 317. Play on a Missing game notifies "Couldn't launch <title>: files not found"; on an unlinked one "Couldn't launch <title>: not linked to a server game" (src: tvmode.js `play`).
- [x] 318. Play with no emulator notifies "Couldn't launch <title>: no emulator for <Platform>" with the action "Set up <Platform>", which opens that System page (src: tvmode.js `play`).

## 12. Pages

### General

- [x] 319. A page is pushed over its tab, the top bar shows its breadcrumb, and B goes back to the screen under it with focus and scroll as they were (src: README § Pages; tvmode.js `push`, `pop`).
- [x] 320. A settings-style page is a centred panel up to 62rem wide with a title (1.9rem) and a muted line under it, and scrolls as one list (src: tvmode.css `.page`, `.page-in`).

### Playing

- [x] 321. Playing shows "NOW PLAYING", the title as a logo (3rem), "<Platform> · <emulator> · <m:ss>" with the session time counting up each second (src: README § Pages › Playing; tvmode.js `playing`).
- [x] 322. Playing shows the note "The game has the controller while it runs. GRID listens again when it closes and uploads your save." (without " and uploads your save" when Upload after exit is off) (src: tvmode.js `playing`).
- [x] 323. Playing has Back to the game (focused) and Stop game (danger) (src: README § Playing; tvmode.js `playing`).
- [x] 324. Back to the game brings the game's window to the front, with GRID behind it (src: tvmode.js `ACT.resume`).
- [x] 325. Stop game asks "Stop <title>?" / "Unsaved progress in the game is lost." with Stop game (danger); stopping goes back and, with Upload after exit on, notifies "Uploading the save for <title>…" then "Save uploaded for <title>" (src: README § Playing; tvmode.js `ACT.stop`).

### System

- [x] 326. A System page opens from Settings › Systems or a Server header chip, titled with the system, under it "<emulator> · <status>" (src: README § Pages › System; tvmode.js `systemPage`).
- [x] 327. Its Emulator row shows the emulator and opens a list "Emulator for <system>" with Installed and Download groups (unfocusable headings), "(recommended)" after the recommended one, and None at the end (src: README § System; README § Overlays › Lists; tvmode.js `emuItems`, `pickEmu`).
- [x] 328. Picking an emulator from the Download group installs it; picking an installed one makes it the system's emulator (src: README § System; tvmode.js `pickEmu`).
- [x] 329. The page shows the version ("<version> · up to date") and, when newer, the action "Update to <version>" (src: README § System; tvmode.js `systemPage`).
- [x] 330. A RetroArch system has a "RetroArch core" row that opens the list "Core for <system>" (src: README § System; tvmode.js `systemPage`).
- [x] 331. A system that needs firmware has "<System> firmware · <n> files · from your server" ("Each file is checked against the size and MD5 the server lists", value Install), which notifies "Installing <System> firmware for <emulator>…" then "<System> firmware installed for <emulator>"; installed firmware shows "<System> firmware" · "Installed" (src: README § System; tvmode.js `systemPage`).
- [x] 332. The page has Open <emulator> ("Opens the emulator on its own to change its settings…") and Emulator settings ("Program, launch options, cloud save folders") (src: README § System; tvmode.js `systemPage`).
- [x] 333. Remove <emulator> (danger) asks "Remove <emulator>?" / "Games and saves stay. <System> games can't start until another emulator is picked." with Remove (danger), notifies "<emulator> removed" and goes back (src: README § System; tvmode.js `systemPage`).
- [x] 334. The page ends with Setup tips as an information row (src: README § System; tvmode.js `systemPage`).
- [x] 335. A system with no emulator has instead "Install <recommended> (recommended)" and "Choose another emulator" (src: tvmode.js `systemPage`).
- [x] 336. Windows has "Default Proton version" ("Runs Windows games on Linux"), "Proton launcher (umu-run)" with its version, "Reinstall umu-launcher" and the information row "First Windows game" · "Downloads the Steam Linux Runtime (about <size>)" (src: README § System; tvmode.js `systemPage`).

### Add an emulator manually / Emulator settings

- [x] 337. Add manually pushes "Add an emulator manually" with "For an emulator GRID doesn't install. It never becomes a system's default by itself."; Emulator settings pushes "<emulator> settings" (src: README § Pages; tvmode.js `emuForm`).
- [x] 338. The form's rows are "Which emulator is it?" (add only; a list of the known emulators and Something else), Name (keyboard), Program (folder browser), Launch options (keyboard, default "%rom%", "%rom% is replaced by the game file") and "Cloud saves (advanced)" (src: README § Pages; tvmode.js `emuForm`).
- [x] 339. Program shows the file name, with the full path (or "Browse for the emulator's program") under it, and opens the folder browser titled "Choose the program" in file mode (src: tvmode.js `emuForm`).
- [x] 340. "Cloud saves (advanced)" shows "Save strategy: <strategy>" ("Save and state folders, files not to upload") and opens the list "How does it store saves?": Automatic, One file per game, A folder per game (src: tvmode.js `emuForm`).
- [x] 341. The form ends with Save ("<emulator> settings saved") or Add emulator ("<name> added"), which goes back (src: tvmode.js `emuForm`).

### Game settings

- [x] 342. Game settings (native games) shows the game title with "Game settings" under it (src: README § Pages › Game settings; tvmode.js `gameSettings`).
- [x] 343. Launch file shows the file and opens a list of the game's launch files; its line reads "Set by game.json" when it came from there, else "Your choice" (src: README § Game settings; tvmode.js `gameSettings`).
- [x] 344. Launch parameters shows the value or "None" ("Quotes group words") and opens the keyboard (src: README § Game settings; tvmode.js `gameSettings`).
- [x] 345. Reload launch info from server notifies "Launch info reloaded" / "game.json sets <file>"; Save notifies "Game settings saved" and goes back (src: README § Game settings; tvmode.js `gameSettings`).

### Link to server game

- [x] 346. Link to server game shows "Link <title>" and "Pick the <Platform> game on your server that this is." (plus "Search: <text>" while searching) over a list of the platform's server games not installed, each with a badge, title and "<Platform> · <year>" (src: README § Pages › Link; tvmode.js `linkPage`).
- [x] 347. A on a game (hint "Link") links it, notifies "<title> linked to <server game>" and goes back (src: tvmode.js `ACT.doLink`).
- [x] 348. Y (hint "Search") opens the keyboard "Search <Platform>" and the list narrows as you type; no match reads "No games match." (src: README § Link; tvmode.js `linkPage`).

### Folder browser

- [x] 349. One folder browser page serves every path: Locate…, Change library folder, Relocate the library, Program, and Connect's Library folder (src: README § Pages › Folder browser; tvmode.js `openBrowser`).
- [x] 350. It shows its title, the current path in a monospace box, then a list whose first row is "Up one folder" (absent at the root) (src: README § Folder browser; tvmode.js `openBrowser`).
- [x] 351. A on a folder (hint "Open") opens it and focuses its first row; A on "Up one folder" (hint "Up") goes up (src: README § Folder browser; tvmode.js `openBrowser`).
- [x] 352. Y "Use this folder" picks the folder shown and goes back; in file mode (Program) there is no Y, files show without a chevron and A on a file (hint "Choose") picks it (src: README § Folder browser; tvmode.js `openBrowser`).

### Search everything

- [x] 353. Y on Discover opens the keyboard titled "Search everything" (src: README § Pages › Search everything; tvmode.js `searchAll`).
- [x] 354. Done with nothing typed just closes the keyboard (src: tvmode.js `searchAll`).
- [x] 355. Done with text pushes "Search: <text>" with "<n> results for “<text>”" ("1 result") over a grid of covers with platform tags, or "Nothing matches. Try fewer letters." (src: README § Search everything; tvmode.js `searchAll`).

### Notifications page

- [x] 356. Menu › Notifications closes any pages and pushes Notifications: "Notifications" with the count, then the session's notices newest first (src: README § Pages › Notifications; tvmode.js `notesPage`).
- [x] 357. Each notice row shows its title (error ones in `danger`) and its second line with the time, "<line> · HH:MM" (src: tvmode.js `notesPage`).
- [x] 358. With none it reads "Nothing yet. Every notice GRID shows lands here too." (src: tvmode.js `notesPage`).
- [x] 359. Y "Clear all" empties the list (src: tvmode.js `notesPage`).

## 13. Overlays

### General

- [x] 360. An overlay covers the stage and the top bar with a `scrim` and a 6px backdrop blur, but not the hint bar; notification cards stay above it (src: tvmode.css `.layer`; consultant decision after P6).
- [x] 361. Focus stays inside the top overlay until it closes (src: tvmode.js `scope`).

### Menu

- [x] 362. Menu (Start) opens a 30rem panel from the right (src: README § Overlays › Menu; tvmode.css `.guide`).
- [x] 363. The panel's head shows the clock (2.2rem) and "<user> · <server host>", or "Offline" (src: README § Menu; tvmode.js `guideMenu`).
- [x] 364. The items, in order: Discover, Downloads (with " · <n> active" counting active and queued rows), Notifications (" · <n>"), Settings, Reconnect, Switch to desktop mode, Quit GRID (src: README § Menu; tvmode.js `guideMenu`).
- [x] 365. Discover closes pages and shows Server › Discover with focus on its content; Downloads and Settings close pages and show that tab (src: tvmode.js `guideMenu`).
- [x] 366. The Menu's hints are A "Select" and B "Close" (src: tvmode.js `guideMenu`).
- [x] 367. The Menu is not available on the startup screens (src: tvmode.js `press`).

### Lists

- [x] 368. Every choice is a centred list (36rem wide, at most 80% of the screen height, scrolling inside) with a title, an optional line under it, and a check on the current value (src: README § Overlays › Lists; tvmode.js `openSheet`; tvmode.css `.sheet`).
- [x] 369. A list opens focused on the checked option (src: tvmode.js `openSheet`).
- [x] 370. Group headings (Installed / Download) are not focusable (src: README § Lists; tvmode.js `openSheet`).
- [x] 371. A dangerous option is in `danger`; an option may show a muted line on its right (src: tvmode.js `openSheet`; tvmode.css `.opt`).
- [x] 372. A picks the option and closes the list; B closes it without change; the hints are A "Select", B "Back" (src: tvmode.js `press`, `hintsNow`).

### Confirmations

- [x] 373. A destructive action always asks first, in a centred dialog (40rem) with a title, a body, and Cancel then the action button, right-aligned (src: README § Overlays › Confirmations; tvmode.js `confirmBox`).
- [x] 374. The dialog opens with Cancel focused, so a double press of A can't confirm by accident (src: README § Confirmations; tvmode.js `confirmBox`).
- [x] 375. The action button is danger-styled for destructive actions, primary otherwise; B cancels (src: tvmode.js `confirmBox`).
- [x] 376. Quit GRID asks "Quit GRID?" / "Downloads in progress pause and resume the next time GRID starts." with Quit (src: tvmode.js `quitBox`).

### On-screen keyboard

- [x] 377. The keyboard slides up from the bottom across the full width, on `overlay`, with its title (muted) and the field with a blinking caret (src: README § Overlays › On-screen keyboard; tvmode.js `openOSK`; tvmode.css `.osk`).
- [x] 378. It has four rows of keys: 1 2 3 4 5 6 7 8 9 0 / q w e r t y u i o p / a s d f g h j k l - / z x c v b n m . / : (src: README § On-screen keyboard; tvmode.js `KEYS`).
- [x] 379. The last row holds Shift (with the LT glyph), the symbols key "@ _ #", Space (with the Y glyph), Delete "⌫" (with the X glyph) and Done (primary, with the Menu glyph) (src: README § On-screen keyboard; tvmode.js `openOSK`).
- [x] 380. Keys are 4.6 x 3.4rem; Shift, symbols, Delete and Done are double width and Space five keys wide (src: tvmode.css `.key`).
- [x] 381. Shift capitalises the next letter only, and its key shows the selected fill while on (src: tvmode.js `openOSK`; tvmode.css `.key.on`).
- [x] 382. The symbols key opens the list "Symbols": @ _ # % ? = & " ! +, and picking one types it (src: tvmode.js `openOSK`).
- [x] 383. Shortcuts: X deletes, Y adds a space, LB / RB move the caret, LT is Shift, Menu is Done, B cancels (src: README § On-screen keyboard; tvmode.js `press`).
- [x] 384. The keyboard's hints are A "Type", X "Delete", Y "Space", LB "◀", RB "▶", Menu "Done", B "Cancel" (src: tvmode.js `openOSK`).
- [x] 385. B cancels and leaves the value as it was; Done keeps the typed value (src: README § On-screen keyboard; tvmode.js `openOSK`).
- [x] 386. Masked fields (tokens, passwords) show dots and open empty (src: README § On-screen keyboard; tvmode.js `srowAct`, `ACT.cfield`).
- [x] 387. A search updates its results with every key (src: README § On-screen keyboard).
- [x] 388. Redrawing the keyboard after each key keeps focus on the same key (src: tvmode.js `openOSK`).

## 14. Startup

### Startup screens

- [x] 389. A startup box always fits between the top bar and the hint bar, so the screen never scrolls (src: README § Startup screens; tvmode.css `.start`).
- [x] 390. A list longer than the space scrolls inside the box, under its title and above its buttons, which never leave view (src: README § Startup screens; tvmode.css `.start-body`, `.start-foot`). (final audit: verified in code)
- [x] 391. The box doesn't repeat the GRID wordmark (src: README § Startup screens).

### Connect

- [x] 392. Connect shows "Connect to your RomM server" and "Everything here works with a controller. A keyboard works too." (src: tvmode.js `connectPage`).
- [x] 393. Its rows: Server ("https://…" placeholder), Sign in with (API token / Username and password, a list), then API token ("Paste or type your token") or Username ("Your RomM user") and Password ("Your password"), then Library folder (src: README § Startup screens › Connect; tvmode.js `connectPage`).
- [x] 394. A on Server, API token, Username or Password opens the keyboard titled "Server address", "API token", "Username" or "Password"; the token and password are masked and open empty (src: tvmode.js `ACT.cfield`).
- [x] 395. A on Library folder opens the folder browser titled "Library folder" (src: README § Connect; tvmode.js `ACT.cfield`).
- [x] 396. View pastes from the clipboard into the field, and the hint reads View "Paste" (src: README § Connect; tvmode.js `connectPage`).
- [x] 397. Connect (primary) is disabled until the server, credentials and library folder are filled; A on it then says "Fill in the server, the API token and the library folder first." ("the username and password" for that sign-in) (src: tvmode.js `connectPage`).
- [x] 398. A rejected token returns to Connect with the reason in `danger` above the rows, as on the desktop (src: README § Connect; tvmode.js `connectPage`).
- [x] 399. A successful connect notifies "Connected to <URL>" and opens Choose your systems (src: tvmode.js `ACT.connectGo`).

### Choose your systems

- [x] 400. Choose your systems shows "Choose your systems" and "Tick the systems you want to play. GRID installs the recommended emulator for each. (X) changes the emulator; (Menu) skips this step. You can change this later in Settings › Systems." (src: tvmode.js `choosePage`).
- [x] 401. Each system is a row with a switch (off at first), its name, "<n> games" (Windows: "Proton for Windows games") and its emulator, the recommended one by default (src: README § Choose your systems; tvmode.js `choosePage`).
- [x] 402. A ticks or unticks the system (hint "Tick" / "Untick") (src: README § Choose your systems; tvmode.js `choosePage`).
- [x] 403. X (hint "Change emulator") opens the list "Emulator for <system>" (Installed and Download groups, None); picking one also ticks the system (src: README § Choose your systems; tvmode.js `ACT.chEmu`).
- [x] 404. The buttons under the list are "Install <n> emulators" ("1 emulator"; "Install emulators", disabled, with none ticked) and "Skip for now" with the Menu glyph (src: README § Choose your systems; tvmode.js `choosePage`).
- [x] 405. Down from the last system reaches the buttons (src: README § Choose your systems).
- [x] 406. Menu (Start) skips from anywhere on the page, and the hint reads Menu "Skip for now" (src: README § Choose your systems; README § Decided Start; tvmode.js `choosePage`).
- [x] 407. Install installs one emulator per name ticked (each emulator once), opens where GRID opens and shows the setup card (src: README § Choose your systems; tvmode.js `ACT.chGo`).
- [x] 408. Skip for now opens where GRID opens without installing (src: tvmode.js `ACT.chSkip`).

### Unreachable library

- [x] 409. When the library folder can't be reached, a blocking dialog shows "Your library can't be reached" / "<path> isn't there: its drive may not be connected. GRID can't run without it." (src: README § Startup screens › Unreachable library; tvmode.js `unreachable`).
- [x] 410. Its buttons are Relocate the library… (primary, focused) and Close without changing anything (src: README § Unreachable library; tvmode.js `unreachable`).
- [x] 411. B does nothing on it, and only A "Select" shows in the hints (src: README § Unreachable library; tvmode.js `press`, `hintsNow`).
- [x] 412. Relocate the library… opens the folder browser titled "Relocate the library"; picking a folder notifies "Library relocated to <path>" / "No files were moved" (src: tvmode.js `press`).

### Where GRID opens

- [x] 413. With Open GRID on set to Automatic, GRID opens on Server › Discover when it is connected to a RomM server (src: README § Decided Where GRID opens; tvmode.js `openStart`).
- [x] 414. With Automatic and the server unreachable, GRID opens on the Library (src: README § Decided Where GRID opens; tvmode.js `openStart`).
- [x] 415. With Automatic and no server, GRID opens on the Library with the first connected Backup drive selected (its folder open, its row focused) (src: README § Decided Where GRID opens; tvmode.js `ACT.noServer`, `showDrive`).
- [x] 416. Open GRID on set to Server › Discover or Library forces that place; forced Discover with no server opens the Server tab's not-connected panel (src: README § Decided Where GRID opens).
- [x] 417. Opening on Discover lands focus on Discover's first cover (src: README § Discover; tvmode.js `render`).

## 15. Game backups in TV mode

### General

- [ ] 418. TV mode uses the Game backups design's drives, words, rules, states and notifications; only the controls change for a controller, and anything not listed here matches the GB card (src: README § Game backups).
- [x] 419. Backup is written as one word in all copy, even as a verb (src: README § Game backups; project memory "UI word Backup").

### Library › Backup drives

- [x] 420. The Backup drives folder has one row per connected, set-up drive with its pack name and game count, or "No Backup drives connected" (src: README § Game backups › Library › Backup drives; tvmode.js `libRailHTML`).
- [x] 421. A drive's header shows the pack name, "<n> games · <size> · on <volume label>" and chips "Emulators: <list>" and "Firmware: <n> systems", then RT "Select" (src: README § Library › Backup drives; tvmode.js `libPaint`).
- [x] 422. A drive's grid shows its games with platform tags; search (Y) and sort (LT) work as usual (src: README § Library › Backup drives).
- [x] 423. A drive that went away while selected shows "<name> is not connected" and "Plug it in to see its games." (src: tvmode.js `libPaint`).
- [x] 424. X on a drive cover is Play when the game is installed, else "Install from drive", which copies the game from the drive with no network (src: README § Library › Backup drives; tvmode.js `quickLabel`, `installFromDrive`).
- [x] 425. Install from drive queues a Downloads row reading "Copying from <volume label> · <n>%" ("Queued · from <label>" while waiting; "Installed from <label>" when done) (src: README § Library › Backup drives; tvmode.js `TABS.downloads.render`, tick).
- [x] 426. Install from drive notifies "<title> added to download queue" / "Copying from <label>" with the action Open Downloads (src: tvmode.js `installFromDrive`).
- [x] 427. When the game's system has no emulator, the drive also brings one, but only an emulator that plays that platform (as `platform-emulators.json` and the emulator profiles decide), and makes it the system's emulator (src: README § Library › Backup drives; GB card § Library › Backup drives).
- [x] 428. When the drive has no emulator for that system, the notice says so ("<pack> has no emulator for <system>") (src: README § Library › Backup drives; tvmode.js `installFromDrive`).
- [x] 429. The drive's firmware for the system installs with the game when the system needs it, and Windows games bring their Proton version the same way, with the GB card's notices (src: GB card § Library › Backup drives).

### Details from a drive

- [x] 430. Details opened from a drive's grid leads the header line with "From <pack> · " (src: README § Details from a drive; tvmode.js `details`).
- [x] 431. Its buttons are Install from drive (or Play when installed, or Cancel while installing) and Remove from drive… (src: README § Details from a drive; tvmode.js `details`).
- [x] 432. The left column says where the copy is: "Installed on this computer." when installed, "From <pack> · <size>" and "Installing copies it from the drive, no network needed." (src: README § Details from a drive; tvmode.js `details`).
- [x] 433. A copy of an installed game adds the GB card's lines ("A copy of an installed game · v1.1", "(the server had newer files)", and the Wine prefix lines for Windows games) (src: GB card § Library › Backup drives).
- [x] 434. Remove from drive… asks "Remove <title> from <pack>?" / "Its files leave the drive. The copy installed on this computer stays." ("You can back it up again later." when not installed) with Remove from drive (danger), notifies "<title> removed from <pack>" and goes back (src: README § Details from a drive; tvmode.js `ACT.removeFromDrive`). (desktop wording, intro rule)

### Details › Backup to drive…

- [x] 435. Backup to drive… sits after Uninstall in Details' buttons (not on unlinked games or games viewed from a drive) and opens the Backup page titled "Backup <title>" (src: README § Details › Backup to drive; tvmode.js `details`, `ACT.backup`).
- [x] 436. When a connected drive already has the game, the left column adds "On <pack>" (src: README § Details › Backup to drive; tvmode.js `details`).
- [x] 437. Offline, Backup to drive… is disabled and A on it shows "Backing up needs a server." (src: README § Details › Backup to drive; tvmode.js `details`).
- [x] 438. From the time a game is queued for backup until it finishes, Uninstall, Update and Locate… on it are refused with "This game is being backed up. Wait for it to finish." (src: GB card § Backups run in Downloads).

### Select mode

- [x] 439. RT turns select mode on and off on the Library grids (filters, platforms and drives) and Server platform grids, like the desktop's Select / Done; entering it starts with nothing picked (src: README § Decided RT; README § Select mode; tvmode.js `selToggle`).
- [x] 440. In select mode, A picks or unpicks a cover instead of opening Details (src: README § Select mode; tvmode.js `press`).
- [x] 441. A picked cover shows a filled `primary` check (1.6rem) at its top left, a `primary` ring and shrinks to 94%; focused, it returns to full size with the focus ring; an UPDATE or MISSING tag moves right of the check (src: README § Select mode; tvmode.css `.cv.pk`, `.pick`).
- [x] 442. X selects all the grid's games, or clears them when all are picked (src: README § Select mode; tvmode.js `selAll`).
- [x] 443. Y acts on the picked games: Backup to drive… on Library and Server grids (src: README § Select mode; tvmode.js `selAct`).
- [x] 444. RT or B leaves select mode (src: README § Select mode; tvmode.js `press`).
- [x] 445. A bar pinned under the grid reads "<n> selected · <size>", then X "Select all <n>" (or "All <n> selected · Clear"), Y "Backup to drive…" (on a drive "Install or remove") and RT "Done" (src: README § Select mode; tvmode.js `selPaint`).
- [x] 446. In select mode the hints are A "Pick" / "Unpick", X "Select all" / "Clear all", Y "Backup to drive…" (or "Install or remove"), RT "Done" (src: tvmode.js `selHints`).
- [x] 447. Y with nothing picked notifies "Pick games first" / "A picks the focused game; X picks them all." (src: tvmode.js `selAct`).
- [x] 448. Y offline notifies the error "Backing up needs a server." (src: tvmode.js `selAct`).
- [x] 449. On a drive, Y opens the list "<n> games on <pack>" with Install <n> games and Remove <n> from drive… (danger) (src: README § Select mode; tvmode.js `selAct`).
- [x] 450. Install <n> games queues every picked game not installed and notifies "<n> games added to download queue" / "Copying from <label>" (src: tvmode.js `selAct`).
- [x] 451. Remove <n> from drive… asks "Remove <n> games from <pack>?" / "Their files leave the drive. Games installed on this computer stay." with Remove from drive (danger), then notifies "<n> games removed from <pack>" (src: tvmode.js `selAct`). (desktop wording, intro rule)
- [x] 452. Acting on the picked games, or the drive going away, ends select mode (src: tvmode.js `selAct`, `startBackup`, `unplugCouch`). Also ends on a rail row change (consultant).
- [x] 453. There is no range pick; Select all covers that case (src: README § Select mode). (final audit: verified in code)

### A whole platform

- [x] 454. A Server platform header has "Backup all <n>…" (hint "Backup"), counting all the platform's games, which opens the Backup page titled "Backup <Platform> · <n> games" (src: README § A whole platform; tvmode.js `srvPaint`, `ACT.backupPlat`).
- [x] 455. On a Library platform row, X is "Backup all <n>…", counting installed games only (src: README § A whole platform; tvmode.js `libRailHTML`, `ACT.backupPlat`).
- [x] 456. Offline, Backup all is disabled and says "Backing up needs a server." (src: tvmode.js `srvPaint`, `ACT.backupPlat`).

### The Backup page

- [x] 457. The Backup page is the desktop's backup dialog as a pushed page that scrolls like every list, titled "Backup <n> games", "Backup <title>" or "Backup <Platform> · <n> games" (src: README § The Backup page; tvmode.js `openBackup`).
- [x] 458. The lede reads "GRID copies games installed on this computer and downloads the rest from <host>, then writes each one, with its art and details, to the drive's gridpack folder.", or "GRID downloads each game from <host>, then writes …" when nothing is copied (src: README § The Backup page; GB card § The backup dialog; tvmode.js `openBackup`). (desktop wording, intro rule)
- [x] 459. The page opens on its first row with the title in view (src: README § The Backup page).
- [x] 460. Drive lists one radio row per connected, set-up drive: "<pack> · <label> · <file system>", "<n> GB left after this backup", and a bar where this backup's share is hatched (src: README § The Backup page; tvmode.js `openBackup`; tvmode.css `.ubar i`).
- [x] 461. A drive's space left counts what queued backups to it still hold: "<n> GB available" with "<n> GB set aside for queued backups" under the bar, which shows that share in a muted disk colour (src: GB card § The backup dialog).
- [x] 462. A drive without room reads "Needs <n> GB · <n> GB free" ("… available" while space is set aside) in `danger`, and choosing it makes the main button "Not enough space" (src: README § The Backup page; GB card § The backup dialog).
- [x] 463. A on a drive row (hint "Choose") selects it (src: tvmode.js `openBackup`).
- [x] 464. With no set-up drive connected, Drive reads "No Backup drive is connected." plus "<label> is connected but not set up for GRID." when one is, and offers Set up a drive…, which shows Settings › Backup drives (src: README § The Backup page; tvmode.js `openBackup`).
- [x] 465. Source shows only when at least one requested game can be copied from this computer (src: README § The Backup page; GB card § The backup dialog).
- [x] 466. Source starts with the switch "Copy installed games from this computer" (on by default) and its note "<n> games from this computer (<size>) · <n> downloaded · <n> to choose"; off, it reads "Every game downloads from <host>" and the rows below hide (src: README § The Backup page; tvmode.js `openBackup`).
- [x] 467. Each out-of-date game has a warning-edged row "<title>: the server has newer files" ("v1.2 on the server; this computer has v1.1") whose value reads "Choose" in the warning colour until picked (src: README § The Backup page; tvmode.js `openBackup`; tvmode.css `.srow.warnrow`).
- [x] 468. Its choice opens a list titled with the game, "The server has newer files (v1.2; this computer has v1.1)", with Use this computer's copy and Download v1.2 from the server; the row then reads "Use this computer's copy" or "Download v1.2" and loses its warning edge (src: README § The Backup page; tvmode.js `openBackup`).
- [x] 469. The GB card's version wording applies ("an older copy" when the installed copy has no version tag; "Download the newer version from the server" when the server's file has none) (src: GB card § The backup dialog).
- [x] 470. With more than one out-of-date game, a warning note "<n> games have newer files on the server. Choose a copy for each." and the bulk rows "Use this computer's copies" and "Download all" show (src: README § The Backup page; tvmode.js `openBackup`).
- [x] 471. "Show each game" (with the count) opens the per-game rows (it then reads "Hide each game"); each shows the title, the platform with the reason a game downloads although installed, and a tag: From this computer, Download or Choose below (src: README § The Backup page; tvmode.js `openBackup`).
- [x] 472. The per-game reasons are the GB card's: "Its files weren't found on this computer", "Installed outside the library folder", "Its install contains links", a name that can't go on a Windows pack, "It was installed from another server", "It's running right now; close it to copy it" (src: GB card § The backup dialog).
- [x] 473. Also copy has Emulators, always on with a fixed check: "<emulators> for Linux and Windows · <size>" (or "for Linux" / "for Windows" per the pack), or "Already on the drive" (src: README § The Backup page; tvmode.js `openBackup`).
- [x] 474. Also copy has the Firmware switch ("<n> systems · <n> files"), starting from Settings › Backup drives › Every backup also copies › Firmware, disabled with the reason when the drive already has the firmware or the server has none (src: README § The Backup page; GB card § The backup dialog).
- [x] 475. When the pack is for Linux and the selection has Windows games, Also copy adds "Make Windows games work offline" (off by default): "Adds the Steam Linux Runtime (about 0.3 GB) so the first launch needs no network" (src: README § The Backup page; tvmode.js `openBackup`). (desktop wording, intro rule)
- [x] 476. When a Windows game is in the backup, the page says "Windows games are copied without their Wine prefix. The save folders GRID knows about are copied from it." (src: GB card § The backup dialog).
- [x] 477. Games already on the chosen drive are listed in the note "<n> games are already on <pack> and will be skipped: <titles>" ("1 game is") (src: README § The Backup page; tvmode.js `openBackup`).
- [x] 478. On a FAT32 drive, games that download and have a file over 4 GB are listed as not possible (src: GB card § The backup dialog).
- [x] 479. The footer shows "<size> to write · <size> to download" (the second half only when some games come from this computer and something downloads) and the main button (src: README § The Backup page; tvmode.js `openBackup`).
- [x] 480. The main button reads "Backup <n> games" ("1 game") with the Menu glyph, and Start presses it from anywhere on the page; the hint reads Menu "Backup" (src: README § The Backup page; README § Decided Start; tvmode.js `rowsPage`).
- [x] 481. Disabled, it reads "Nothing to backup" (why: "Every game is already on <pack>."), "Choose for <n> games" (why: "Choose a copy for each game the server has newer files for.") or "Not enough space" (why: "<pack> doesn't have room for this backup."), and Start or A on it shows the why as an error (src: README § The Backup page; tvmode.js `openBackup`, `rowsPage`).
- [x] 482. Starting the backup goes back, ends select mode and notifies "<n> games added to the backup queue for <pack>" with the action Open Downloads (src: README § Downloads; tvmode.js `startBackup`).

### Backups in Downloads

- [x] 483. Each game, emulator and firmware row of a backup carries the Backup badge; game rows queue first, then emulators ("<emulator> for Linux and Windows"), then firmware ("Firmware · <n> systems") (src: README § Downloads; GB card § Backups run in Downloads; tvmode.js `startBackup`).
- [x] 484. A backup row reads "Downloading from server <n>%" or "Copying from this computer to <label> <n>%", then "Writing to <label> <n>%" (src: README § Downloads; tvmode.js `TABS.downloads.render`).
- [x] 485. A game copied from this computer first reads "Getting details from server" (src: GB card § Backups run in Downloads).
- [x] 486. A queued backup row reads "Waiting · to <pack>"; a finished one "Written to <pack>", or "Copied from this computer · Written to <pack>" (src: README § Downloads; tvmode.js `backupRowDone`).
- [x] 487. A backup row's sparkline is network-coloured while downloading and disk-coloured while writing, and each row has Cancel (src: GB card § Backups run in Downloads).
- [x] 488. One notification per backup: "<n> games added to the backup queue for <pack>", then "<n> games and <m> emulator backed up to <pack>" (the emulator part only when emulators were written) (src: README § Downloads; tvmode.js `backupRowDone`).
- [x] 489. A finished backup updates the drive's Last backup (src: tvmode.js `backupRowDone`).

### A drive removed mid-backup

- [x] 490. When a drive is removed mid-backup, the row being written fails: "Failed — <label> was removed" (src: README § A drive removed mid-backup; tvmode.js `unplugCouch`).
- [x] 491. The drive's remaining queue becomes one Completed row "<n> more items" reading "Cancelled — <label> was removed · backup again to finish" (src: README § A drive removed mid-backup; tvmode.js `unplugCouch`).
- [x] 492. One error card says how many items were written: "<label> was removed during the backup" / "<n> items were written. Backup again to finish." (src: README § A drive removed mid-backup; tvmode.js `unplugCouch`). (desktop wording, intro rule)
- [x] 493. The drive leaves the Library rail; if it was selected, the rail falls back to All games (src: README § A drive removed mid-backup; tvmode.js `unplugCouch`).
- [x] 494. A drive appears in the rail within a few seconds of being plugged in, and leaves when removed, with focus kept per the redraw rules (src: GB card intro; tvmode.js `unplugCouch`).

### Settings › Backup drives

- [x] 495. The pane lists Connected drives (set up or not), then Not connected (remembered) drives, then the group "Every backup also copies" with the Firmware switch ("The server's firmware for the systems backed up. The Backup page starts with this.") (src: README § Settings › Backup drives; tvmode.js `drivesPane`).
- [x] 496. A drive row shows the pack name (or volume label), "<label> · <file system> · <size>" (plus " · <n> games" when set up) and a status pill: "Ready for backups" (green dot), "Not set up" (warning) or "Not connected" (src: GB card § Settings › Backup drives; tvmode.js `drivesPane`).
- [x] 497. A remembered drive's row reads "Last seen <when> · <n> games · <size>" (src: tvmode.js `drivesPane`).
- [x] 498. With no drive connected, Connected reads "No drives connected. Plug in a USB drive or SSD." (src: tvmode.js `drivesPane`).
- [x] 499. A on a set-up drive (hint "Open") pushes its page: the pack name over "<label> · <file system> · <size> · Ready for backups" (src: README § Settings › Backup drives; tvmode.js `drivePage`).
- [x] 500. The drive page's rows: "For Linux and Windows" (or "For Linux" / "For Windows") with Change…; the space bar; Games; Emulators; Firmware ("<n> systems"); Last backup; Folder; Show in Library; Open folder; Rename…; Remove GRID files… (src: README § Settings › Backup drives; tvmode.js `drivePage`).
- [x] 501. The space bar is split into other files, GRID and free, with "Other files <n> GB · GRID <n> GB · <n> GB free" (src: README § Settings › Backup drives; tvmode.js `drivePage`).
- [x] 502. Change… pushes "Computers this pack is for" ("Emulators go on the drive built for each one. At least one stays ticked.") with Linux and Windows switches (src: README § Settings › Backup drives; tvmode.js `osPage`).
- [x] 503. Turning off the last ticked computer is refused with the error "At least one computer stays ticked" (src: README § Settings › Backup drives; tvmode.js `osPage`).
- [x] 504. Adding a computer notifies "<OS> builds of <n> emulators added to the backup queue for <pack>"; removing one notifies "<OS> builds removed from <pack>" / "Games and firmware stay" (src: GB card § Settings › Backup drives; tvmode.js `osPage`). (desktop wording, intro rule)
- [x] 505. Show in Library shows the Library on that drive (its folder open, its row selected) (src: tvmode.js `showDrive`).
- [x] 506. Open folder ("Opens the file manager in desktop mode") switches to desktop mode to open the folder (src: README § Settings › Backup drives; tvmode.js `drivePage`).
- [x] 507. Rename… opens the keyboard "Pack name"; a non-empty name renames the pack and notifies "Renamed to <name>" (src: README § Settings › Backup drives; tvmode.js `drivePage`).
- [x] 508. Remove GRID files… (danger, "Deletes only the gridpack folder; games installed on this computer stay") asks twice: "Remove GRID files from <pack>?" / "The gridpack folder (<size>) is deleted from <label>. Nothing else on the drive is touched, and the games installed on this computer stay." with Continue (danger), then "Delete <size> from <label>?" / "This can't be undone." with Remove GRID files (danger) (src: README § Settings › Backup drives; tvmode.js `drivePage`).
- [x] 509. Removing the GRID files notifies "GRID files removed from <label>" / "<size> freed", goes back, and the drive shows as Not set up (src: tvmode.js `drivePage`).
- [x] 510. A drive that isn't set up opens "Set up <label> for GRID" ("<file system> · <size> · Not set up") with Pack name (keyboard, the volume label by default), the heading "Computers this pack is for", Linux and Windows switches with this computer's OS on (src: README § Settings › Backup drives; GB card § Settings › Backup drives; tvmode.js `setupPage`).
- [x] 511. The setup page warns on FAT32: "This drive is FAT32: files over 4 GB can't go on it. exFAT avoids that. GRID never formats a drive." (src: README § Settings › Backup drives; tvmode.js `setupPage`).
- [x] 512. The setup page notes "GRID makes a gridpack folder at the drive's root and never touches anything outside it." (src: README § Settings › Backup drives; tvmode.js `setupPage`).
- [x] 513. The setup page's main button is "Set up for GRID" with the Menu glyph; Start presses it (hint Menu "Set up"); it notifies "<pack> is ready for backups" / "<label> · <file system>" and goes back (src: README § Decided Start; tvmode.js `setupPage`).
- [x] 514. A remembered drive opens a list titled with its name ("Last seen <when> · <n> games · <size>") offering "Forget <name>" (danger), which notifies "<name> forgotten" (src: README § Settings › Backup drives; tvmode.js `drivePage`).

### Use without a server

- [x] 515. Connect offers "Use without a server" after an "or", beside Connect, enabled once a library folder is set, with the note "Without a server, GRID needs only the library folder. Your Library and Backup drives work offline." (src: README § Use without a server; tvmode.js `connectPage`).
- [x] 516. Use without a server notifies "Using GRID without a server" / "Your Library and Backup drives work offline" and opens per "Where GRID opens" (src: README § Use without a server; tvmode.js `ACT.noServer`).
- [x] 517. Without a server the top bar reads "Offline · no server" with a grey dot, and the hint bar says the Library and Backup drives work without one (src: README § Use without a server).
- [x] 518. Without a server the Server tab shows one panel: "Not connected to a server", "Your Library and Backup drives work without one.", Connect to a server… (primary) and Open Library (src: README § Use without a server; tvmode.js `TABS.server.render`).
- [x] 519. Connect to a server… opens Connect (src: README § Use without a server; tvmode.js `ACT.toConnect`).
- [x] 520. Library, Backup drives, Downloads, Settings and play all work without a server; backing up is disabled with "Backing up needs a server." (src: GB card § Use without a server).

## 16. Desktop changes

- [x] 521. The desktop top bar gets a TV button that opens TV mode (src: README § Decided Entering and leaving).
- [x] 522. Starting GRID with `--tv` opens straight into TV mode (src: README § Decided Entering and leaving).
- [x] 523. Menu › Switch to desktop mode and Settings › TV mode › Switch to desktop mode leave TV mode for the desktop window, on the same view (src: README § Decided Entering and leaving; tvmode.js `toDesktop`).
- [x] 524. There is no other way into or out of TV mode (src: README § Decided Entering and leaving).
- [x] 525. The desktop opens on Server › Discover when connected to a RomM server and on the Library when not (unreachable, or without a server, on the first connected Backup drive), replacing today's "always Discover" (src: README § Decided Where GRID opens).
- [x] 526. Settings › Appearance › Open GRID on (Automatic, Server › Discover, Library) is one setting shared by the desktop and TV mode and governs both (src: README § Decided Where GRID opens; tvmode.js `paneRows('appearance')`).
- [x] 527. Card size, game labels and card display stay desktop-only settings and don't affect TV mode (src: README § Decided Implementation).
- [x] 528. The TV mode work, the Game backups TV screens and these desktop changes ship together on `feat/tv-mode` (src: README § Decided Implementation). (P14: built and verified together on feat/tv-mode)

## Not to copy

Prototype-only stand-ins. The real app takes these from real data or real behaviour.

1. All sample data: the 20 games and their colours, summaries, release offsets and states (tvmode.js `G`, `SUM`); systems, emulators, versions and statuses (`SYS`, `EMUS`); the download rows (`DL`); the three drives Couch kit / USB-1TB / Travel stick and their sizes (`DRIVES`); the fake file system (`FS`); `CFG` values (romm.home.lan, user sam, /home/sam/Games/GRID).
2. Made-up figures: 1.4 / 1.6 / 1.66 GB per game, "1.2 GB", "1.7 GB", "38.2 MB/s · 2 min left", the static sparkline points, "120 MB" per emulator, "2 systems · 5 files", "84 GB", "1.2 TB free", "v1.2 / v1.1", "GE-Proton9-20", "umu 1.2.6", "about 200 MB", GRID 0.9.12 / 0.9.13, "Today at 09:14".
3. Placeholder Details content: "Lighthouse Works", "Racing, Adventure", "8.4", "Players 1–2", the chips "Identified" and "USA · EN", the eight achievement names and points, the four sample saves, "slot" numbers, and the setup tip text.
4. The fixed "today" of 9 Nov 2026 used by `relDate` and `ago`'s inputs.
5. The covers drawn as coloured blocks and the background "art" drawn as two blurred colour blobs (`cover`, `hero`); the real app uses the game's cover and art.
6. The Menu's **PROTOTYPE ONLY** section and everything it triggers: Show: first run (Connect), Show: token rejected (and its reason text), Show: server unreachable / back online, Show: unreachable library, Show: restart GRID (and its "GRID started" notice), Show: unplug / plug in Couch kit (and its "Couch kit connected" / "Couch kit removed" notices), Show: a failed install.
7. Simulated timings: the 900 ms progress tick and its random increments, the setup card running installs one after another with random 1.2–2.4 s / 0.7–1.4 s phases, reconnect after 900 ms, upload after 1200 ms, firmware after 1800 ms, emulator install after 2200 ms, update check after 1200 ms, save upload after Stop after 1400 ms, the 600 ms delayed welcome card.
8. The desktop-mode placeholder screen ("Desktop mode … Prototype: press A or click to return") and the quit screen ("GRID has quit … press A to start it again") (tvmode.js `toDesktop`, `quitBox`).
9. Reading controllers through the browser Gamepad API (`pollPads`, `PADB`, `requestAnimationFrame` polling), the "Controller: press any button on it" line and the "This page isn't allowed to read controllers…" line; Decided puts a native gilrs reader in their place.
10. The text-glyph fallback table `GL` (letters and symbols in discs, `.gl`), including "Select" / "Start" for 8BitDo and "⌂" for Guide; the real prompts are the Kenney glyphs, with the text only as accessible labels.
11. The keyboard keys G (Guide) and the Space-as-A mapping are preview conveniences beyond the README's table.
12. "Pasted from the clipboard" as a notice with nothing pasted; the real View pastes into the field.
13. The Locate… success test (a folder named "Velvet Static" or "storage/Games") and the fixed starting folders of each folder browser.
14. The Discover offline shelf texts ("Needs the server. It comes back when GRID reconnects.") are unreachable in the prototype, because offline Server shows the offline panel instead.
15. The Updates pane's release link "github.com/…/releases/tag/v0.9.13".
16. `CFG.exitBtn` and `TAB_NAME.home`, left over from earlier rounds and unused.
17. "Check for updates" result "1 update available · PCSX2 2.3.4" and "Sent to 3 emulators and your server account".

## Conflicts

Places where the prototype disagrees with the README, or where the sources leave a gap. README Decided wins.

1. **Root font size.** tvmode.css sets `min(100vh / 54, 100vw / 80)`; the README says 1/54 of the screen height. Follow the README (the width cap only matters on screens narrower than 80:54).
2. **Minimum text size.** The README says no text under 0.85rem; tvmode.css uses 0.7–0.8rem for the TV tag, cover tags, kind badges, LATEST, facts labels, rail and group headings, the hero platform line, source tags and key glyph text. Raise them to at least 0.85rem.
3. **Sideways-row margin.** The README says a sideways row keeps the focused cover 2.4rem inside its edges; inside the Server tab (Discover) and in Details (Related, Media), the prototype's rows pad 1.2rem, so the rule there keeps 1.2rem. Decide which applies inside panels.
4. **8BitDo View / Menu labels.** The prototype's text labels are "Select" / "Start"; the README and map.json use − / +. Use − / +.
5. **Guide button.** The prototype maps the Guide button (and key G) to the Menu and gives it a "⌂" text glyph. The README gives Guide no glyph and no role. Leave it unbound unless decided otherwise.
6. **Start on the startup screens.** Decided says Start opens the Menu wherever a page has no main button, but the prototype makes the Menu unavailable during startup, so Start does nothing on Connect. Confirm that no Menu during startup is intended.
7. **Playing page vs. ignored input.** Decided says GRID ignores the controller while a game runs, so Back to the game and Stop game on Playing can't be reached with a controller while the game is running. The keyboard and mouse can still reach them. Confirm this is intended.
8. **Cancel in Downloads.** The README says a destructive action always asks first, and Details' Cancel install asks, but the Downloads row's Cancel cancels at once. Add the confirmation.
9. **Other unconfirmed removals.** Removing a computer from a pack (which deletes its emulator builds from the drive) and RetroAchievements › Clear act without confirmation in the prototype. They fall under "a destructive action always asks first" unless the desktop doesn't confirm them.
10. **Details tab with nothing to focus.** The prototype's comment says focus stays where it was; its code and the README put focus on the first button. Follow the README.
11. **Show each game.** The prototype always collapses the per-game rows; the GB card shows them inline and collapses only above 6 games. Follow the GB card.
12. **Space set aside for queued backups**, the **Firmware switch disabled with a reason**, the **Wine prefix note** and the **FAT32 over-4 GB list** are in the GB card but missing from the TV prototype. The GB card applies.
13. **Install-from-drive notices.** The prototype says "<emulator> from <label> is now the <system> emulator" and "… · <emulator> from <pack> installs with it"; the GB card says "DuckStation from Couch kit added to download queue" and "PlayStation firmware installed from Couch kit". Use the GB card's wording.
14. **Set up for GRID defaults.** The prototype always turns Linux on; the GB card turns on this computer's OS.
15. **Welcome card.** The prototype shows "Welcome to TV mode" / "Press (Menu) for the menu. Desktop mode is one press away there." at start. The README says notifications are the desktop's events, and the desktop has no such event. Decide whether TV mode gets this card.
16. **Notification history cap.** The prototype keeps only the last 20 notices for Menu › Notifications; the README sets no cap.
17. **Search as you type.** The README says search updates results as you type, but Search everything only shows results after Done (the Library, Server and Link searches do update live). Decide whether Search everything goes live.
18. **Unreachable library › Close.** The prototype's "Close without changing anything" opens the "Quit GRID?" confirmation. GRID can't run without the library, so this should close GRID as the desktop does, without a second question.
19. **Open <emulator>.** The prototype's System row says GRID "Switches to desktop mode while it runs"; README § Notes says opening an emulator from TV mode works and its window needs a mouse, with no mode switch. Decide which happens.
20. **On-screen keyboard and the hint bar.** The README says the keyboard slides up "over the hint bar's area"; the prototype stops overlays above the hint bar and keeps it visible with the keyboard's hints.
21. **Backup progress in the top bar.** The GB card's strip reads "Backing up Velvet Static to Couch kit · 45% · downloading"; TV's top bar shows only title, bar and percent. TV's README top bar governs; confirm a backup shows there with its plain title.
22. **Theme default.** The README says TV mode is dark by default, but Settings › Appearance › Theme is shared with the desktop. The prototype forces dark at start, whatever the setting. Decide whether TV mode has its own default or follows the shared Theme.
23. **Open GRID on in the desktop's Settings.** Decided says the setting changes the desktop too, but doesn't say the desktop's Settings › Appearance shows the row. Item 526 assumes it does.
24. **No-controller hint line.** With the browser lines removed (Not to copy 9), the sources give no text for the hint bar's left side when no controller is connected. The TV mode pane's "None found · the keyboard works too" is the nearest wording.


## Rulings on the Conflicts (2026-11-10)

The Fable consultant ruled on every conflict above; the rulings are recorded in docs/design/system/components/TVMode/README.md § Consultant decisions and override any checklist item they touch (conflict n → ruling n, in order).

## Added after P9 (consultant)

- [x] 529. Install or Play refused for umu-run opens a confirm "Install umu-launcher?" / "<title> runs through Proton, which needs umu-run. umu-launcher downloads as a Downloads row; launch <title> again when it finishes." / "Install umu-launcher" (primary), Cancel focused; while a umu row is live the card "umu-launcher is installing (see Downloads). Launch the game again when it finishes." with Open Downloads. (src: README § Consultant decisions)
- [x] 530. Install refused for no library folder opens "Set a library folder" / "GRID installs games into a library folder, and none is set. Choose one, then press Install again." / "Choose a folder…" → folder browser "Library folder" at home, Y saves; success card "Library folder set" with the path; refusal = error card with the reason; Install is pressed again by the user. (src: README § Consultant decisions)
- [x] 531. Details › Saves (kind Saves) on a native game shows "Save locations" with the desktop's status line, rows (raw path over expanded path; PCGamingWiki rows first), A on a row confirms "Remove this save location?" (danger "Remove"), focus to the row in its place; "Add a save folder…" opens "Save folder for <title>", Y adds ("Save folder added"); Upload latest keeps "Add a save location to enable uploads." until a row exists. (src: README § Consultant decisions)
- [x] 532. Settings › TV mode › Button assignments with no controller: disabled, line "For the controller in use", value "Xbox style", A says "Connect a controller to choose its buttons. Each controller keeps its own choice."; re-enables when a pad connects. (src: README § Consultant decisions)
- [x] 533. Settings › RetroAchievements rows: Username, Token, Save, Password (masked, opens empty, sub "Write-only: used once to log in, never kept"), Log In (sub "Signs in with your password and saves the token it gets"), Clear. Save disabled until username + token ("Enter a username and a token first." / "Enter a token first." / "Enter a username first."); Log In disabled until username + password (same pattern with "password"); Log In runs raLogin, blanks the password, refreshes status. (src: README § Consultant decisions)
- [x] 534. Downloads: a live firmware row is disabled, A says "Installing firmware — this step cannot be cancelled" (library move: "Updating paths — this step cannot be cancelled"); an extracting install's A hint reads "Details" and its list offers only "Open <title>". (src: README § Consultant decisions)

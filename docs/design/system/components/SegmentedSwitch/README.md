A two-way switch that picks which list a pane shows, such as Saves | States.

Hand-written from `app/src/lib/details/SavesTab.svelte (.switch)`. Static rendition: the app is Svelte.

- One side is always selected (`primary` fill), so it reads as a choice of view, not an action.
- The track is `surface` with a `line` edge at `r-pill`.
- Show it only when both sides have content. With one side, show that list without a switch.
- Labels name the content ("Saves", "States", or "Memory cards").

The game Details overlay: cover and actions on the left, title, chips and tabs on the right.

Hand-written from `app/src/lib/Details.svelte`. Static rendition: the app is Svelte.

- An `overlay` at `r-dialog` with `shadow-overlay`, over `scrim` with a 6px blur. At most 1040×680.
- The left column is 240px: the whole cover (at most 320px tall), then the action group (one primary, the rest secondary), then meta lines. Only the right column's tab panel scrolls.
- The header has the `view-title`, a muted header line (platform · year · genre · rating with a `primary` star) and fact chips.
- Close is an icon button in the top-right corner. Escape or a click on the scrim closes it.

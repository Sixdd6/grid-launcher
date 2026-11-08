The 220px left rail shared by Library, Server, Emulators and Settings: labelled rows with optional counts and section headings.

Hand-written from `app/src/lib/RailPane.svelte`. Static rendition: the app is Svelte.

- A floating `panel` at `r-panel`, a `gutter` away from the view.
- Rows are `body` in `text-muted`. Hover fills `field`. The selected row is `selected` with a 3px inset `primary` bar, in `text-h` at 600.
- Counts are `small`, right-aligned. Headings (PLATFORMS) are `rail-heading`, uppercase.
- The consumer supplies the rows (key, label, count, selected, heading) and the select handler.

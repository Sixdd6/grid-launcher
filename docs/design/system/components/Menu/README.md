A small overlay menu that drops from a trigger, such as the server menu under the session chip.

Hand-written from `app/src/lib/Shell.svelte (.server-menu)`. Static rendition: the app is Svelte.

- `overlay` fill, `line` edge, `r-row` corners, `shadow-overlay`.
- Rows are 13px `text-h`. Hover fills `surface`.
- A muted footer line (`small`, over a `border` rule) can name the context, such as the server host.
- Escape or a click outside closes it.

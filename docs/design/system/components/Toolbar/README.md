A view's header panel: the title, counts, link chips, and a toolbar of search and selects.

Hand-written from `app/src/lib/Server.svelte (.head)`. Static rendition: the app is Svelte.

- One `panel` holds the header and the toolbar, because they belong together.
- The title is `view-title`. Counts are `meta` in `text-muted`, as "48 games · 6 installed".
- Link chips (`g-chip-outline`) sit on the right and open the related view. Hover turns the edge `primary`.
- Every toolbar control is `control-h` tall, on `field` with a `line-strong` edge. Captions (Size, Sort) are `small`, before the control.

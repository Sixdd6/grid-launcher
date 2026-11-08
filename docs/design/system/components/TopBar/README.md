The 58px bar across the top of the window: brandmark, the five view pills and the session cluster.

Hand-written from `app/src/lib/Shell.svelte`. Static rendition: the app is Svelte.

- Left: the `grid` brandmark in `primary` and the GRID wordmark. Centre: the pill group (Library, Server, Downloads, Emulators, Settings) on a `field` track. Right: the Update badge when a release is out, the status dot (`success` online, `danger` offline) and the session chip, which opens the server menu.
- The active pill is a `primary` fill. Quiet pills are `text-muted` and turn `text-h` on hover.
- The bar is a `panel` with a `line` bottom edge. It never shrinks.
- Ctrl+1 to Ctrl+5 switch views.

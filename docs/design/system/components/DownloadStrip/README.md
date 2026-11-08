The 28px strip at the bottom of the window that follows the current transfer.

Hand-written from `app/src/lib/DownloadsFooter.svelte`. Static rendition: the app is Svelte.

- Hidden while nothing transfers.
- Line: "<title> · <percent> · <speed>", with a 120×18 sparkline (network in `primary`, disk in `graph-disk`) and an "Open Downloads" link in `primary`.
- The whole strip is one button that opens Downloads.
- `surface-2` fill and a `border` top edge.

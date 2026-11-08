A short message that floats above the download strip and leaves on its own.

Hand-written from `app/src/lib/Toast.svelte`. Static rendition: the app is Svelte.

- `surface-2` fill, `border` edge, `r-row`, 13px at 600, centred, at most 480px wide.
- `error` toasts use `danger` text. Success toasts stay `text-h`.
- Use toasts for results that have no other place to show: auto uploads, firmware installs, launches.
- Start with the game title when there is one: "Starfall Circuit — Cloud save uploaded".

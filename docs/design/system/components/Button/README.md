A text button for the actions in Details, dialogs and forms.

Hand-written from `app/src/lib/Details.svelte (.action button)`. Static rendition: the app is Svelte.

- **Primary** (`g-btn`): the one thing the panel is for — Play, Install, Locate…. At most one per group. `primary` fill, `on-primary` text, `r-control` corners.
- **Secondary** (`g-btn secondary`): everything else. Transparent with a `border` edge; hover fills `surface`.
- **Destructive** (`secondary destructive`): `danger` text only. Use it for removals that keep files.
- **Confirm** (`confirm`): the second click of a two-click destructive action. The label changes to "Confirm …".
- **Update confirm** (`update-confirm`): a caution, not a destruction. It uses `warning` because the label says nothing is lost.
- Disabled buttons drop to 60% opacity. Give the reason in a visible meta line, not only a tooltip.
- The consumer supplies the label (verb first, sentence case) and the state.

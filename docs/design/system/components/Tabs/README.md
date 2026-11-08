The underline tabs at the top of the Details panel: Overview, Media, Saves, Achievements.

Hand-written from `app/src/lib/Details.svelte (.tabs)`. Static rendition: the app is Svelte.

- Inactive tabs are `text-muted`. The active tab is `text-h` with a 2px `primary` underline over a `border` rule.
- A mouse click does not draw the focus ring. The Tab key still does.
- Tabs that have nothing to show are left out, not disabled.

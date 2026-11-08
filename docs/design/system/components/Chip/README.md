Small pill labels for facts and states: header chips in Details and outline link chips in view headers.

Hand-written from `app/src/lib/Details.svelte (.chip), app/src/lib/Server.svelte (.chip)`. Static rendition: the app is Svelte.

- **Fact chip** (`g-chip`): `surface` fill, `border` edge, `chip` type. Identified, version, flags.
- **Playing** (`g-chip playing`): a `primary` fill, uppercase. Only while the game runs.
- **Status** (`g-chip status`): a 7px dot before the words. The dot is `warning` for a state to resolve and `danger` for missing files. The words carry the meaning.
- **Outline link** (`g-chip-outline`): a button that opens another view (firmware, default emulator).

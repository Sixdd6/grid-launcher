The Overview tab layout: a readable description beside a panel of facts.

Hand-written from `app/src/lib/details/OverviewTab.svelte`. Static rendition: the app is Svelte.

- The description is `description` type, at most 68ch, split into paragraphs.
- Facts are a `dl` on `surface` with a `line` edge at `r-row`. Each fact is a `small` uppercase label over a `body` value.
- Leave out facts that have no value. Never show an empty label.

A game's cover, shown whole at its own shape, with corner badges, a hover action bar and a title label.

Hand-written from `app/src/lib/GameCard.svelte`. Static rendition: the app is Svelte.

- No frame and no fill: the card is the size of its art. Corners are `r-card`. At rest the edge is `shadow-card`.
- Hover or keyboard focus lifts the cover 4px, adds `shadow-lift`, fades in a dark shade and the action bar (Play or Install, Cloud sync, Details), and hides the corner badges.
- Badges: `success` installed dot top-right, MISSING and UPDATE tags stacked top-left, a cloud badge bottom-right, and a platform tag bottom-left in the Library only.
- Labels: "Always visible" puts a 24px `label` pill under the cover. "Visible on hover" floats it on `overlay`. Either way, hover shows the full title, wrapped.
- A missing game's primary action is Details, not Play.
- The consumer supplies the title, cover URL, states, label mode and handlers. The grid places the card.

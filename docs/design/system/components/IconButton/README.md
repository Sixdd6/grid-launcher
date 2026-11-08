A 28×28 button that holds one icon: close, back, dismiss, carousel arrows.

Hand-written from `app/src/app.css (.icon-btn)`. Static rendition: the app is Svelte.

- 28×28 is the minimum pointer target. The icon inside is 20 (close, navigation) or 14 (dismiss).
- Hover and focus fill `surface` and turn the icon `primary`.
- Always give an `aria-label`: the icon is `aria-hidden`.
- The consumer supplies the icon name, the label and the placement. The box, radius and reset come from the class.

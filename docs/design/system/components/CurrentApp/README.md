A clickable prototype of the app as it is today, built from the components in this system.

Hand-written from `app/src/lib/Shell.svelte and the five views`. Static rendition: the app is Svelte.

- Switch views with the top-bar pills or Ctrl+1 to Ctrl+5.
- Server and Library: pick platforms or filters in the rail, search, change the card size, and hover a card for its actions. Install queues a placeholder download: the strip and the Downloads view follow it, and a toast reports the result.
- Click a card to open Details. Try each tab, the Saves | States switch, and the two-click Uninstall.
- Emulators: Installed shows the firmware states (Install firmware, Requested…, Installed, Not needed), and Edit opens the Manual form. Add from catalog searches and installs. Platform defaults changes the Server header's emulator chip and shows the RetroArch core select. Compat tools has the umu-run line, the default tool and the install list.
- Settings: Connection with its edit form, Library with the Change… dialog (move or start fresh), Cloud saves, RetroAchievements, Updates, and a live Appearance pane: theme, background fade, label mode and card size change the whole prototype.
- The frame fills whatever height it is given; the rail and grids scroll inside it.
- Covers, titles and counts are placeholders. Real covers have their own shapes, and the app lays them out in equal-height rows or equal-width columns.

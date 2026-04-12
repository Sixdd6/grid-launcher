# TV Mode Idea - not ready to implement!
Fullscreen experience that works like a "emulation station" "ES-DE" or "bigbox".


## Views
The home view shows at startup by default but can be changed to another view from the theme settings.
The settings view can be reached by pressing the back button on keyboard (Esc) or controller (B/Cross/East).


- [ ] Home - shows rows of game items in a netflix style layout with fanart from metadata fading slowly in/out between different images in the background.
	- [ ] "continue playing"
	- [ ] "favorites"
	- [ ] "library"
	- [ ] "server"
	- [ ] "new additions"

- [ ] Settings - various submenu items
	- [ ] General
	- [ ] Theme
	- [ ] Sound
	- [ ] Video
	- [ ] Controller Mapping
	- [ ] Keybinds

- [ ] Server - reached from the main home view or can be set as the home view from theme settings.
	- [ ] Contains a horizontal image carousel of the available platforms, displayed as images by their cover art.
		- [ ] Each platform contains a "wall" view of the games available on the server displayed by their cover art.
			- [ ] Each game per platform links to the details view for that game.

- [ ] Library - reached from the main home view or can be set as the home view from theme settings.
	- [ ] Contains a "wall" view of all currently installed games.

- [ ] Details View - reached from either server or library views.
	- [ ] Shows the metadata, cover art, screenshots and buttons for "play/install", "Uninstall", "Cloud Saves", "Cloud States", "Achievements" in a layout similar to the desktop mode game details view.
		- [ ] Launching a game initiates the standard cloud sync workflow to allow the user to pick up where they left off



## Misc Features
- [ ] Ability to change which view to open on startup from theme settings
- [ ] Fullscreen pause: when user presses the Guide button on their controller display a fullscreen menu with options to "Resume Game", "View Manual" and "Quit Game". (This fullscreen view should be limited to emulators that do not already have a built-in Guide button function.)
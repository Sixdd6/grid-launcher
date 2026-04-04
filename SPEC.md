The application is a game launcher and manager which connects to a server via the RomM api specified in 'openapi.json'.
The application is built using Python + PySide6 and is designed to be run via a selfcontained exe file on Windows and a selfcontained bash wrapper on Linux.

The top bar includes the buttons to navigate between the main sections of the application with the current logged in user displayed on the right.

There are several main sections to the application with buttons across the top to navigate between them:
- Library
- Server
- Downloads
- Emulators
- Settings

- The Library section contains a grid layout of all currently installed games, represented by their cover art.
- The Server section contains a vertical list of the platforms supported by the server on the left, and a grid layout of the games for the selected platform filling the remaining space on the right. All downloads from the server should be queued and downloaded in the background, with a progress bar showing the download progress for each game.
- The Downloads section contains a list of all downloads that are currently in progress, with a progress bar showing the download progress for each game. The completed downloads should show as completed and wait for the user to dismiss it from the list via a button. Any failed downloads should show as failed with a button to retry or cancel the download.
- The Emulators section contains a list of emulators that can be used to launch games. It should include the ability to add, edit, and remove emulators, as well as set a default emulator for each platform. The emulators should be organized by name. Each emulator should have a name, path to executable, and arguments to pass to the executable, including the placeholder %rom% which will be replaced with the path to the rom file.
- The Settings section contains the various settings for the application arranged by frames per section.


## Sub views

- Game Details View
> Clicking on a game in the Library or Server sections should open a sub view with more information about the game, including buttons to install/launch/uninstall the game depending on the context. The sub view should also include a button to go back to the previous view. The sub view should have a larger cover art image and more detailed information about the game including description, ratings icons based on client region, companies, genres, star ratings. The right side of the sub view should include any available screenshots in a vertical scrollable area. Specifically for Windows games, the sub view should include a buttons for a "game settings" dialog where the player can configure the executable and arguments to use when launching the game, valid executables should be scanned for from the game's directory and include exe, bat, cmd, ps1, sh.

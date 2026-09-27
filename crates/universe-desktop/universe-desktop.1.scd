universe-desktop(1)

# NAME

universe-desktop - the Universe game library for mouse and keyboard

# SYNOPSIS

*universe-desktop* [_OPTIONS_]

# DESCRIPTION

Shows the library (GTK 4, libadwaita) over the Universe core linked
in-process: the games of every source and platform, their pages, the Store,
screenshots, recordings and the journal, and the preferences of the launch,
the runners, the controller, the stores, the modules and the artwork.

Games launch as transient systemd units bound to the app's own scope: quitting
it (SIGINT and SIGTERM included) stops the running game first, and asks before
it does. A download pauses and resumes from the Store the next time. A game,
a screenshot, a recording or a journal entry removed goes once its toast
does, unless Undo brings it back; one still waiting is removed on quit.

GNOME Shell's search finds the library's games when the search provider is
installed; picking one opens its page and starts it. *Open Big Screen* in the
main menu starts *universe-ui*(1), the couch launcher, and leaves the window
open.

# OPTIONS

*--gapplication-service*
	Start as a D-Bus service: how GNOME Shell starts the app for a search or
	from the app grid. No window opens until one is asked for.

*-h*, *--help*
	Print help.

# KEYS

|[ Ctrl+F
:< search the library
|  Ctrl+H
:  show the hidden games
|  Ctrl+N
:  add a game
|  Ctrl+Z
:  undo the last hide, favourite or removal still showing its toast
|  Delete
:  remove the game whose page is open
|  F9
:  show or hide the sidebar
|  F10
:  the main menu
|  Ctrl+,
:  preferences
|  Ctrl+?
:  every shortcut
|  Ctrl+W, Ctrl+Q
:  close the window, quit

# ENVIRONMENT

*UNIVERSE_BIN*
	The *universe*(1) the session hooks call back. Unset, the one on PATH.

*UNIVERSE_MODULES_PATH*, *UNIVERSE_SOURCES_PATH*
	Where the modules and the sources are looked for, as for *universe*(1).

*UNIVERSE_CONFIG_HOME*, *UNIVERSE_DATA_HOME*, *UNIVERSE_STATE_HOME*, *UNIVERSE_CACHE_HOME*
	Another profile's config, library, state and cache.

# FILES

_$XDG_STATE_HOME/universe/desktop.json_
	The window's size, the sidebar's pick, the sort, whether hidden games
	show and whether the first run was seen.

_$XDG_CACHE_HOME/universe/remote/_
	The store pictures shown, shrunk.

# SEE ALSO

*universe*(1), *universe-ui*(1)

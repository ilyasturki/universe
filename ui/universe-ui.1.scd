universe-ui(1)

# NAME

universe-ui - gamepad-first game library on the Universe core

# SYNOPSIS

*universe-ui* [_OPTIONS_]

# DESCRIPTION

Renders the Reprise interface (Qt 6, PySide6) over the Universe core linked
in-process. Fullscreen by default. Games launch as transient systemd units
bound to the UI's own scope: quitting the UI (SIGINT and SIGTERM included)
stops the running game first, and *universe session-end* closes the session
when its cgroup empties. While a game runs the launch poster stays as the
running view; holding A for a second quits the game.

# OPTIONS

*--windowed*
	A window instead of fullscreen. *--size* and *--screenshot* imply it.

*--size* _WxH_
	Window size (default 1920x1080).

*--theme* _ID_
	The look for this run: *reprise*, *switch2-white* or *switch2-black*. Without it
	the host starts in the look last picked in Settings › Themes, Reprise at first.

*--no-gamepad*
	Do not open the SDL2 gamepad; keyboard only.

*--fake*
	A fixture library instead of the core, for UI work.

*--fake-launch*
	Fake sessions run *sleep 2*. Implies *--fake*.

*--screenshot* _PATH_
	Grab the window to _PATH_, then quit.

*--after* _MS_
	Delay before *--screenshot* (default 3000).

*--quit-after* _MS_
	Quit after _MS_ milliseconds; 0 never (default).

*--keys* _LIST_
	Key names posted once loaded, e.g. 'Right Right Return'. *Wait* idles one gap;
	with *--fake*, *Press:slot*, *Unpress:slot* and *Axis:lx=0.6* play the fake pad.

*--key-gap* _MS_
	Gap between posted keys (default 120).

*--key-delay* _MS_
	Delay before the first posted key (default 1200).

*-h*, *--help*
	Print help.

# KEYS

Pegasus's default bindings; the gamepad posts the same keys.

|[ Return
:< accept
|  Escape
:  back
|  I
:  details
|  F
:  filters
|  Q / E
:  previous / next page
|  PageUp / PageDown
:  scroll
|  F1
:  menu

# ENVIRONMENT

*UNIVERSE_BIN*
	The *universe* CLI hooks and systemd's ExecStopPost run; set by the Nix wrapper.

*UNIVERSE_MODULES_PATH*
	Extra module roots, colon-separated.

*UNIVERSE_CONFIG_HOME*, *UNIVERSE_DATA_HOME*, *UNIVERSE_STATE_HOME*, *UNIVERSE_CACHE_HOME*
	Replace the XDG directories, as for *universe*(1).

# SEE ALSO

*universe*(1)

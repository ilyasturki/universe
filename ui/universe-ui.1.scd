universe-ui(1)

# NAME

universe-ui - gamepad-first game library on the Universe core

# SYNOPSIS

*universe-ui* [_OPTIONS_]

# DESCRIPTION

Renders the launcher (Qt 6, PySide6) in the Reprise or Switch 2 look over the
Universe core linked in-process. Fullscreen by default. Games launch as
transient systemd units bound to the UI's own scope: quitting the UI (SIGINT
and SIGTERM included) stops the running game first, and *universe session-end*
closes the session when its cgroup empties. While a game runs the launcher
stays home with the game pinned first: A resumes it, its menu quits it.
B held asks to quit the launcher; Settings › About shows its build.

# OPTIONS

*--windowed*
	A window instead of fullscreen. *--size* implies it.

*--size* _WxH_
	Window size (default 1920x1080).

*--theme* _ID_
	The look for this run: *reprise* or *switch2*. Without it
	the host starts in the look last picked in Settings › Themes, Reprise at first.

*--no-gamepad*
	Do not open the SDL2 gamepad; keyboard only.

*--fake*
	A fixture library instead of the core, for UI work.

*--fake-launch*
	Fake sessions run *sleep 2*. Implies *--fake*.

*--quit-after*, *--keys*, *--key-gap*, *--key-delay*
	The test harness: see *--help*.

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

*UNIVERSE_MODULES_PATH*, *UNIVERSE_SOURCES_PATH*
	Extra module and source roots, colon-separated.

*UNIVERSE_CONFIG_HOME*, *UNIVERSE_DATA_HOME*, *UNIVERSE_STATE_HOME*
	Replace the XDG directories, as for *universe*(1).

# SEE ALSO

*universe*(1)

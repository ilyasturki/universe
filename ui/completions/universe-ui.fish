complete -c universe-ui -f
complete -c universe-ui -s h -l help -d 'Print help'
complete -c universe-ui -l fake -d 'Fixture library, no core'
complete -c universe-ui -l fake-launch -d 'Fake session runs `sleep 2` (implies --fake)'
complete -c universe-ui -l windowed -d 'A window instead of fullscreen'
complete -c universe-ui -l screenshot -r -F -d 'Grab the window to PATH, then quit'
complete -c universe-ui -l after -x -d 'Delay in ms before --screenshot (3000)'
complete -c universe-ui -l quit-after -x -d 'Quit after MS (0 = never)'
complete -c universe-ui -l no-gamepad -d 'Do not open the SDL2 gamepad'
complete -c universe-ui -l keys -x -d "Key names to post once loaded, e.g. 'Right Right Return'"
complete -c universe-ui -l key-gap -x -d 'Ms between posted keys (120)'
complete -c universe-ui -l key-delay -x -d 'Ms before the first posted key (1200)'
complete -c universe-ui -l size -x -a '1920x1080 1280x720 2560x1440' -d 'Window size WxH, implies --windowed'
complete -c universe-ui -l theme -x -a 'reprise switch2-white switch2-black' -d 'The look for this run'

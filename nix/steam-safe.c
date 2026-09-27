// universe-ui's entry point, linked statically: no dynamic loader runs here, so Steam's LD_PRELOAD of its overlay
// (gameoverlayrenderer.so, which needs a libGL.so.1 no Nix binary can resolve) cannot kill the launcher before it starts.
// It drops that overlay from LD_PRELOAD, keeps anything else there, and execs the Qt wrapper.
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#ifndef TARGET
#error "TARGET: the wrapper to exec"
#endif

int main(int argc, char **argv) {
    (void)argc;
    const char *preload = getenv("LD_PRELOAD");
    if (preload != NULL) {
        size_t n = strlen(preload);
        char *kept = malloc(n + 1);
        char *copy = strdup(preload);
        if (kept == NULL || copy == NULL)
            return 127;
        kept[0] = '\0';
        // ld.so splits LD_PRELOAD on colons and spaces.
        for (char *tok = strtok(copy, ": "); tok != NULL; tok = strtok(NULL, ": ")) {
            if (strstr(tok, "gameoverlayrenderer.so") != NULL)
                continue;
            if (kept[0] != '\0')
                strcat(kept, ":");
            strcat(kept, tok);
        }
        if (kept[0] == '\0')
            unsetenv("LD_PRELOAD");
        else
            setenv("LD_PRELOAD", kept, 1);
    }
    argv[0] = TARGET;
    execv(TARGET, argv);
    return 127;
}

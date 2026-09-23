import contextlib
import os
import subprocess
import time

from _common import (
    active_outputs,
    first_frame_at,
    flag_value,
    game_frozen,
    gsr_cli,
    log,
    now_rfc3339,
    part_path,
    probe_size,
    show_osd,
    timeline,
    wait_recorder,
    with_flag,
)

# A monitor that just came up has no CRTC for a few seconds: gsr exits at once until it does.
RESTART_TRIES = 10
RESTART_WAIT_S = 2.0
RESTART_GRACE_S = 15.0
FIRST_FRAME_WAIT_S = 5.0
STOP_WAIT_S = 30


def open_pause(state):
    return state["pauses"][-1] if state["pauses"] and state["pauses"][-1][1] is None else None


class Recorder:
    poll_s = 1.0

    def __init__(self, argv, session_id, data_dir, game_unit, drm_dir=None):
        self.argv = list(argv)
        self.session_id = session_id
        self.data_dir = data_dir
        self.game_unit = game_unit
        self.drm_dir = drm_dir
        self.output = flag_value(argv, "-o") or ""
        self.screen = flag_value(argv, "-w") or ""
        self.child: subprocess.Popen[bytes] | None = None
        self.started = 0.0
        self.tries = 0
        self.switched = False
        self.stopping = False

    def spawn(self):
        self.child = subprocess.Popen(self.argv, stdin=subprocess.DEVNULL)
        self.started = time.monotonic()

    @property
    def proc(self) -> subprocess.Popen[bytes]:
        assert self.child is not None
        return self.child

    def alive(self):
        return self.proc.poll() is None

    def session_stopping(self):
        if self.stopping:
            return True
        with timeline(self.data_dir, self.session_id) as state:
            return state is None or bool(state.get("stopping"))

    def run(self):
        self.spawn()
        while True:
            time.sleep(self.poll_s)
            code = self.proc.poll()
            lost = self.screen not in active_outputs(self.drm_dir)
            if code is None and not lost:
                continue
            if self.session_stopping():
                return self.proc.wait()
            if code is None:
                self.save()
                code = self.proc.returncode
            elif not lost:
                if self.retry_due():
                    self.discard_output()
                    self.tries += 1
                    log(f"gpu-screen-recorder exited {code} right after moving to {self.screen}, try {self.tries}/{RESTART_TRIES}")
                    time.sleep(RESTART_WAIT_S)
                    self.attempt()
                    continue
                log(f"gpu-screen-recorder exited {code} on {self.screen}")
                return code
            if not self.park():
                return code
            screen = self.wait_screen()
            if screen is None:
                return 0
            self.restart(screen)

    def retry_due(self):
        return self.switched and self.tries < RESTART_TRIES and time.monotonic() - self.started < RESTART_GRACE_S and not os.path.exists(self.output + ".ts")

    def discard_output(self):
        for p in (self.output, self.output + ".ts"):
            with contextlib.suppress(OSError):
                os.remove(p)

    def save(self):
        try:
            r = gsr_cli(self.session_id, "stop", timeout=STOP_WAIT_S)
            if r.returncode != 0:
                log(f"gsr-cli stop: {(r.stderr or r.stdout).strip()}")
        except (OSError, subprocess.SubprocessError) as e:
            log(f"gsr-cli stop: {e}")
        try:
            self.proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.proc.terminate()
            self.proc.wait()

    def park(self):
        """The file so far becomes the next part; a pause opens until the next monitor's first frame."""
        with timeline(self.data_dir, self.session_id) as state:
            if state is None:
                log(f"{self.screen} went away before the timeline existed, not following")
                return False
            if os.path.exists(self.output):
                parts = state.setdefault("parts", [])
                dest = part_path(self.output, len(parts) + 1)
                os.replace(self.output, dest)
                if os.path.exists(self.output + ".ts"):
                    os.replace(self.output + ".ts", dest + ".ts")
                parts.append(dest)
                log(f"{self.screen} went away, {os.path.basename(dest)} saved")
            if not open_pause(state):
                state["pauses"].append([now_rfc3339(), None])
        return True

    def wait_screen(self):
        """The connector to record next: the same one back, else the first being drawn on; None once the session stops."""
        while True:
            outs = active_outputs(self.drm_dir)
            if outs:
                return self.screen if self.screen in outs else outs[0]
            time.sleep(self.poll_s)
            if self.session_stopping():
                return None

    def restart(self, screen):
        self.screen = screen
        self.argv = with_flag(self.argv, "-w", screen)
        size = self.first_part_size()
        if size:
            self.argv = with_flag(self.argv, "-s", f"{size[0]}x{size[1]}")
        self.switched = True
        self.tries = 0
        log(f"recording {screen}: {' '.join(self.argv)}")
        self.attempt()

    def attempt(self):
        """One run on the new monitor: the gap closes with its first frame, a frozen game pauses it right away."""
        self.spawn()
        wait_recorder(self.session_id, alive=self.alive)
        if not self.alive():
            return
        first = self.wait_first_frame() or now_rfc3339()
        with timeline(self.data_dir, self.session_id) as state:
            if state is None:
                return
            state["screen"] = self.screen
            if game_frozen(self.game_unit):
                # set_paused would skip a recorder the timeline already counts as paused; this one is new.
                gsr_cli(self.session_id, "set-paused", "true")
                state["paused"] = True
                if not open_pause(state):
                    state["pauses"].append([first, None])
            else:
                state["paused"] = False
                if pause := open_pause(state):
                    pause[1] = first
        show_osd(f"Recording {self.screen}")

    def first_part_size(self):
        with timeline(self.data_dir, self.session_id) as state:
            parts = (state or {}).get("parts") or []
        return probe_size(parts[0]) if parts else None

    def wait_first_frame(self):
        deadline = time.monotonic() + FIRST_FRAME_WAIT_S
        while time.monotonic() < deadline and self.alive():
            if at := first_frame_at(self.output):
                return at
            time.sleep(0.2)
        return None

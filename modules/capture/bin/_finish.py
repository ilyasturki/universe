import contextlib
import os
import subprocess

from _common import drop_timeline, first_frame_at, log, now_rfc3339, probe_duration, probe_size, remove_sidecar, timeline, timeline_path


def stitch(files, output):
    """`-c copy` over the parts in order, into `output`; the longest part alone when ffmpeg cannot."""
    sizes = {probe_size(p) for p in files}
    if len(sizes) > 1:
        log(f"parts of different sizes ({', '.join(f'{w}x{h}' for w, h in sorted(s for s in sizes if s))}): the file changes size mid-way")
    listing = output + ".parts"
    with open(listing, "w") as f:
        f.writelines(f"file '{p}'\n" for p in files)
    stem, ext = os.path.splitext(output)
    tmp = f"{stem}.stitch{ext}"
    try:
        r = subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "concat", "-safe", "0", "-i", listing, "-c", "copy", tmp], capture_output=True, text=True, check=False)
    except OSError as e:
        r = subprocess.CompletedProcess([], 1, "", str(e))
    finally:
        os.remove(listing)
    if r.returncode == 0 and probe_duration(tmp) is not None:
        os.replace(tmp, output)
        for p in files:
            if p != output:
                os.remove(p)
        log(f"stitched {len(files)} parts into {os.path.basename(output)}")
        return output
    log(f"ffmpeg concat failed: {r.stderr.strip()} — filing the longest part, the others stay in pending/")
    with contextlib.suppress(OSError):
        os.remove(tmp)
    return max(files, key=lambda p: probe_duration(p) or 0)


def finish(session_id, data_dir, current, min_duration_s):
    with timeline(data_dir, session_id) as state:
        parts = list((state or {}).get("parts") or [])
    files = [p for p in [*parts, current] if p and os.path.exists(p)]
    started_at = first_frame_at(files[0]) if files else None
    with timeline(data_dir, session_id) as state:
        if state is not None and state["paused"]:
            state["paused"] = False
            state["pauses"][-1][1] = now_rfc3339()
        if state is not None and started_at:
            # The file starts at its first frame, not when the unit did; a pause opened before it (HOME while the picker was up) starts with the file
            state["started_at"] = started_at
            state["pauses"] = [[max(a, started_at), b] for a, b in state["pauses"] if b is None or b > started_at]

    if not files:
        log(f"no recording file for session {session_id}, nothing to do")
        drop_timeline(data_dir, session_id)
        return 0
    for p in files:
        remove_sidecar(p)
    path = stitch(files, current or files[-1]) if len(files) > 1 else files[0]

    duration = probe_duration(path)
    if duration is None:
        log(f"unreadable recording {path}, discarding")
        subprocess.run(["trash", path], check=False)
        drop_timeline(data_dir, session_id)
        return 0
    log(f"recording duration: {duration:.1f}s (min {min_duration_s}s)")

    if duration < min_duration_s:
        log(f"recording shorter than {min_duration_s}s, discarding")
        subprocess.run(["trash", path], check=False)
        drop_timeline(data_dir, session_id)
        return 0

    cmd = [os.environ.get("UNIVERSE_BIN") or "universe", "recording-file", session_id, path]
    if os.path.exists(timeline_path(data_dir, session_id)):
        cmd += ["--timeline", timeline_path(data_dir, session_id)]
    result = subprocess.run(cmd, capture_output=True, text=True, check=False)
    if result.returncode != 0:
        log(f"universe recording-file failed ({result.returncode}): {result.stderr.strip()} — left in pending/: {path}")
        return 1

    log(f"filed: {result.stdout.strip()}")
    drop_timeline(data_dir, session_id)
    return 0


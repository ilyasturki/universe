import json
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
TOOL = REPO / "tools" / "promo"

PROMO = """
[[take]]
name = "home"
look = "reprise"
kind = "clip"
keys = "Wait:2 Mark:browse Right Wait:2 Mark:end"

[[take]]
name = "game"
look = "reprise"
kind = "gamescope-clip"
keys = "Expect:dead-cells Return Wait:5 Mark:game"

[[video]]
name = "post"
cuts = ["home:start..browse", "home:browse..end", "game:game..+0.5"]
"""


def promo(root, *args):
    done = subprocess.run([sys.executable, str(TOOL), "--root", str(root), *args], capture_output=True, text=True, check=False)
    return done.returncode, done.stdout + done.stderr


def recorded(root, name, seconds, marks):
    takes = root / "target/promo/takes"
    takes.mkdir(parents=True, exist_ok=True)
    source = f"testsrc2=size=320x180:rate=60:duration={seconds}"
    tone = f"sine=frequency=440:sample_rate=48000:duration={seconds}"
    lavfi = ["-f", "lavfi", "-i", source, "-f", "lavfi", "-i", tone]
    encode = ["-c:v", "libx264", "-preset", "ultrafast", "-c:a", "aac", "-shortest", str(takes / f"{name}.mp4")]
    subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", *lavfi, *encode], check=True)
    (takes / f"{name}.marks").write_text("".join(f"{mark} {at}\n" for mark, at in marks.items()))


def streams(video):
    out = subprocess.run(
        ["ffprobe", "-v", "error", "-show_entries", "stream=codec_type,duration", "-of", "json", str(video)], check=True, capture_output=True, text=True
    )
    return {s["codec_type"]: float(s["duration"]) for s in json.loads(out.stdout)["streams"]}


def test_a_video_plays_its_cuts_end_to_end_with_their_sound(tmp_path):
    code, out = promo(REPO, "check")
    assert code == 0, out
    (tmp_path / "tools").mkdir()
    (tmp_path / "tools/promo.toml").write_text(PROMO)
    recorded(tmp_path, "home", 1.5, {"browse": 0.4, "end": 1.0})

    code, out = promo(tmp_path, "cut", "post")
    assert code == 1
    assert "no target/promo/takes/game.mp4: just promo record game" in out, "a cut of a take not recorded yet"

    recorded(tmp_path, "game", 1.0, {"game": 0.2})
    code, out = promo(tmp_path, "cut", "post")
    assert code == 0, out
    lengths = streams(tmp_path / "target/promo/post.mp4")
    assert lengths.keys() == {"video", "audio"}
    assert abs(lengths["video"] - 1.5) < 0.05, "0.4 s from the start, 0.6 s on to the end, half a second of the game"
    assert abs(lengths["audio"] - lengths["video"]) < 0.05

    (tmp_path / "tools/promo.toml").write_text(PROMO.replace("game:game..+0.5", "game:menu..+0.5").replace('name = "game"', 'name = "game"\ngap = 0'))
    code, out = promo(tmp_path, "check")
    assert code == 1
    assert "take game: gap is the ms between keys" in out
    assert "video post: cut 'game:menu..+0.5': no Mark:menu in game's keys" in out

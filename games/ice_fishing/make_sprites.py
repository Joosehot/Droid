"""Make ice_fishing's sprites with Piksu (C:/Users/joose/Piksu, `cargo build --release`).

    python games/ice_fishing/make_sprites.py [path/to/piksu.exe]

Each row of SPRITES below is one Piksu sentence and the sprite names it makes in
game/sprites/: `<name>_<anim>_<i>.txt` for every frame, and `<name>.txt` (the base
sprite the scene uses) is the first frame of the first row of that name. Frames of
one name are cropped to the same box (the union of every frame's pixels), so the
swim and struggle frames line up and keep one size on screen. The script prints
each box and the entity scale that shows it at PIXEL units per sprite pixel
(scene.toml uses these scales).
"""

import pathlib
import subprocess
import sys
import tempfile

GAME = pathlib.Path(__file__).resolve().parent
SPRITES_DIR = GAME / "game" / "sprites"
PIKSU = sys.argv[1] if len(sys.argv) > 1 else r"C:\Users\joose\Piksu\target\release\piksu.exe"

# 36 screen pixels per unit at zoom 20 in a 720-pixel-high window: 1/18 unit = 2 screen pixels.
PIXEL = 1.0 / 18.0

# (sprite name, animation name in the files, Piksu sentence)
SPRITES = [
    ("perch", "swim", "a perch, swimming, 32px"),
    ("perch", "struggle", "a perch, struggling, 32px"),
    ("roach", "swim", "a roach, swimming, 28px"),
    ("roach", "struggle", "a roach, struggling, 28px"),
    ("pike", "swim", "a pike, swimming, 56px"),
    ("pike", "struggle", "a pike, struggling, 56px"),
    ("hook", "still", "a silver hook, 16px"),
    ("fisherman", "idle", "an ice fisherman, idle, 48px"),
]


def read_sprite(path):
    head, rows = path.read_text().split("---\n", 1)
    return head, [r for r in rows.splitlines() if r]


def frame_index(path):
    return int(path.stem.rsplit("_", 1)[1])


def main():
    made = {}  # name -> [(anim, i, head, rows)]
    with tempfile.TemporaryDirectory() as tmp:
        for name, anim, sentence in SPRITES:
            out = pathlib.Path(tmp) / f"{name}_{anim}"
            subprocess.run([PIKSU, sentence, "-o", str(out)], check=True, stdout=subprocess.DEVNULL)
            files = sorted((out / "droid").glob("*.txt"), key=frame_index)
            for i, f in enumerate(files, 1):
                head, rows = read_sprite(f)
                made.setdefault(name, []).append((anim, i, head, rows))
    for name, frames in made.items():
        cells = [(y, x) for _, _, _, rows in frames for y, r in enumerate(rows) for x, ch in enumerate(r) if ch != "."]
        y0, y1 = min(c[0] for c in cells), max(c[0] for c in cells) + 1
        x0, x1 = min(c[1] for c in cells), max(c[1] for c in cells) + 1
        for old in SPRITES_DIR.glob(f"{name}_*.txt"):
            old.unlink()
        for k, (anim, i, head, rows) in enumerate(frames):
            text = head + "---\n" + "".join(r[x0:x1] + "\n" for r in rows[y0:y1])
            (SPRITES_DIR / f"{name}_{anim}_{i}.txt").write_text(text, newline="\n")
            if k == 0:
                (SPRITES_DIR / f"{name}.txt").write_text(text, newline="\n")
        w, h = x1 - x0, y1 - y0
        print(f"{name}: {len(frames)} frames, {w} x {h} px -> scale [{w * PIXEL:.3f}, {h * PIXEL:.3f}, 1.0]")


if __name__ == "__main__":
    main()

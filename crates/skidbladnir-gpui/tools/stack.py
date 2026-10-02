"""Old above new, for the same region of both screenshots, enlarged."""
import sys
from PIL import Image
old, new, out = sys.argv[1:4]
box = tuple(int(v) for v in sys.argv[4].split(","))
scale = float(sys.argv[5]) if len(sys.argv) > 5 else 2
a, b = Image.open(old).crop(box), Image.open(new).crop(box)
w, h = a.size
canvas = Image.new("RGB", (w, h * 2 + 6), (255, 0, 255))
canvas.paste(a, (0, 0)); canvas.paste(b, (0, h + 6))
canvas.resize((int(w * scale), int((h * 2 + 6) * scale)), Image.NEAREST).save(out)

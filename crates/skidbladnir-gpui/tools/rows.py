"""Ink runs (rows containing dark pixels) inside a column band of a screenshot."""
import sys
import numpy as np
from PIL import Image
path, x0, x1 = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
y0, y1 = (int(sys.argv[4]), int(sys.argv[5])) if len(sys.argv) > 5 else (0, None)
a = np.asarray(Image.open(path).convert("L"), dtype=np.int16)[y0:y1, x0:x1]
ink = (a < 150).any(axis=1)
runs, start = [], None
for y, on in enumerate(ink):
    if on and start is None: start = y
    if not on and start is not None: runs.append((start + y0, y - 1 + y0)); start = None
print(" ".join(f"{s}-{e}" for s, e in runs))

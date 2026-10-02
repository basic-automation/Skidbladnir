"""Compare two window screenshots: a red/cyan overlay and the rows/columns where ink differs."""
import sys
import numpy as np
from PIL import Image
old, new, out = sys.argv[1:4]
a = np.asarray(Image.open(old).convert("L"), dtype=np.int16)
b = np.asarray(Image.open(new).convert("L"), dtype=np.int16)
h, w = min(a.shape[0], b.shape[0]), min(a.shape[1], b.shape[1])
a, b = a[:h, :w], b[:h, :w]
overlay = np.stack([a, b, b], axis=-1).astype(np.uint8)  # old in red channel, new in cyan
Image.fromarray(overlay).save(out)
d = np.abs(a - b) > 40
print(f"differing pixels: {d.mean()*100:.2f}%")

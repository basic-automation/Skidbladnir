"""Raise the baseline gpui computes by SHIFT em units, keeping the content height.

gpui centres ascent+descent in the line box, as CSS does, yet WebKitGTK draws Fira Code
1-3 device px higher than that formula puts it. Moving the split between ascent and descent
moves gpui's baseline the same way without changing any line box.
"""
import sys
from fontTools.ttLib import TTFont
shift = int(sys.argv[1])
for path in sys.argv[2:]:
    f = TTFont(path)
    h, o = f["hhea"], f["OS/2"]
    base = 1980  # the published ascent; the descent follows so the sum is unchanged
    h.ascent, h.descent = base - shift, -(644 + shift)
    o.sTypoAscender, o.sTypoDescender = base - shift, -(644 + shift)
    f.save(path)
    print(path.split("/")[-1], h.ascent, h.descent)

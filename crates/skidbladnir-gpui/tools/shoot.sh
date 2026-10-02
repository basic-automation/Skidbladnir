# usage: shoot.sh <comm-name> <out.png>: screenshot the window owned by a process named <comm-name>
G=$(hyprctl clients -j | python3 -c "
import json,sys
for c in json.load(sys.stdin):
    try: exe=open(f\"/proc/{c['pid']}/comm\").read().strip()
    except: continue
    if exe=='$1': print(f\"{c['at'][0]},{c['at'][1]} {c['size'][0]}x{c['size'][1]}\"); break")
[ -n "$G" ] && grim -g "$G" $2 && echo "$G"

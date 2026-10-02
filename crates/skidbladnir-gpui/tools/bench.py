"""Benchmark the 1.1.0 Tauri window against the gpui spike, on identical UI and identical work."""
import json, os, statistics, subprocess, sys, time
# The work directory: target/, libheif-gpl/, venv/, xdg/, out/, test.png and batch-*.png.
S = os.environ.get("SKID_WORK", os.path.dirname(os.path.abspath(__file__)))
OLD = os.path.expanduser("~/.local/bin/skidbladnir")
NEW = f"{S}/target/release/skidbladnir-gpuikit-spike"
BASE = {**os.environ, "SKIDBLADNIR_NO_UPDATE_CHECK": "1", "XDG_CONFIG_HOME": f"{S}/xdg"}
RUNS = int(sys.argv[1]) if len(sys.argv) > 1 else 5

def kill():
    subprocess.run(["pkill", "-x", "skidbladnir-gpu"]); subprocess.run(["pkill", "-x", "skidbladnir"]); time.sleep(1.0)

def roots(kind):
    """The app's own processes: the AppImage forks its app away from the process launched."""
    name = "skidbladnir" if kind == "old" else "skidbladnir-gpu"
    return [int(p) for p in subprocess.run(["pgrep", "-x", name], capture_output=True, text=True).stdout.split()]

def tree(pid):
    out = [pid]
    for child in subprocess.run(["pgrep", "-P", str(pid)], capture_output=True, text=True).stdout.split():
        out += tree(int(child))
    return out

def pss_mb(kind):
    total = 0
    for pid in {p for r in roots(kind) for p in tree(r)}:
        try:
            for line in open(f"/proc/{pid}/smaps_rollup"):
                if line.startswith("Pss:"): total += int(line.split()[1])
        except OSError: pass
    return total / 1024

def cpu_s(kind):
    tick = os.sysconf("SC_CLK_TCK"); total = 0
    for pid in {p for r in roots(kind) for p in tree(r)}:
        try:
            f = open(f"/proc/{pid}/stat").read().rsplit(")", 1)[1].split()
            total += int(f[11]) + int(f[12])
        except OSError: pass
    return total / tick

def window_geometry(kind):
    pids = {p for r in roots(kind) for p in tree(r)}
    for c in json.loads(subprocess.run(["hyprctl", "clients", "-j"], capture_output=True, text=True).stdout):
        if c["pid"] in pids and c["mapped"]:
            return c["at"], c["size"]
    return None

def convert_drawn(kind):
    """The Convert button's violet is on screen: the window has drawn its UI."""
    g = window_geometry(kind)
    if not g: return False
    (x, y), (w, h) = g
    # Inside the Convert button's padding: 145 css px left of the right edge, 68 down.
    px = subprocess.run(["grim", "-g", f"{x + w - 145},{y + 68} 1x1", "-t", "ppm", "-"], capture_output=True).stdout
    r, g_, b = px[-3], px[-2], px[-1]
    return b > 200 and r > 120 and g_ < 150

def launch(kind, extra=None):
    env = {**BASE, **(extra or {})}
    if kind == "new": env["LD_LIBRARY_PATH"] = f"{S}/libheif-gpl/lib"
    if kind == "old": env["WEBKIT_INSPECTOR_HTTP_SERVER"] = "127.0.0.1:9333"
    return subprocess.Popen([OLD if kind == "old" else NEW], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)

def wk(js):
    return subprocess.run([f"{S}/venv/bin/python", f"{S}/wk.py"], input=js, capture_output=True, text=True, timeout=60).stdout.strip()

def startup(kind):
    kill(); t0 = time.perf_counter(); p = launch(kind)
    while not convert_drawn(kind):
        if time.perf_counter() - t0 > 20: return None, p
        time.sleep(0.01)
    return (time.perf_counter() - t0) * 1000, p

DROP = "window.__TAURI_INTERNALS__.invoke('plugin:event|emit', {event:'tauri://drag-drop', payload:{paths:%s, position:{x:10,y:10}}});"
PREVIEW_JS = """(() => { window.__bench = null; %s
 setTimeout(async () => { const t0 = performance.now(); [...document.querySelectorAll('button')].find(b => b.textContent.trim() === 'Preview').click();
   const wait = () => new Promise(r => requestAnimationFrame(r));
   for (;;) { await wait(); const imgs = [...document.querySelectorAll('figure img')]; if (imgs.length === 2 && imgs.every(i => i.complete && i.naturalWidth)) { await Promise.all(imgs.map(i => i.decode())); await wait(); await wait(); window.__bench = performance.now() - t0; return } } }, 1500); return 1 })()"""
CONVERT_JS = """(() => { window.__bench = null; %s
 setTimeout(async () => { const t0 = performance.now(); [...document.querySelectorAll('button')].find(b => b.textContent.includes('Convert')).click();
   const wait = () => new Promise(r => requestAnimationFrame(r));
   for (;;) { await wait(); const rows = document.querySelectorAll('table tbody tr'); if (rows.length === %d && !document.body.innerText.includes('Cancel')) { await wait(); window.__bench = performance.now() - t0; return } } }, 1500); return 1 })()"""

def old_timed(js, timeout=120):
    wk(js); t = time.time()
    while time.time() - t < timeout:
        value = wk("String(window.__bench)")
        if value not in ("null", "", "undefined"): return float(value)
        time.sleep(0.25)
    return None

def new_timed(proc, key, timeout=120):
    t = time.time()
    while time.time() - t < timeout:
        line = proc.stderr.readline()
        if line.startswith(f"bench {key}_ms"): return float(line.split()[-1])
    return None

def clear_out():
    for f in os.listdir(f"{S}/out"): os.remove(f"{S}/out/{f}")

results = {k: {"old": [], "new": []} for k in ("startup_ms", "idle_pss_mb", "idle_cpu_s_10s", "preview_ms", "preview_pss_mb", "convert6_ms")}
image = f"{S}/test.png"; batch = [f"{S}/batch-{i}.png" for i in range(1, 7)]
for run in range(RUNS):
    for kind in ("old", "new"):
        ms, p = startup(kind); results["startup_ms"][kind].append(ms)
        time.sleep(6); c0 = cpu_s(kind); time.sleep(10); results["idle_cpu_s_10s"][kind].append(round(cpu_s(kind) - c0, 3))
        results["idle_pss_mb"][kind].append(round(pss_mb(kind), 1))
        # Preview: queue one image, press Preview, until both sides are decoded and drawn.
        if kind == "old":
            ms = old_timed(PREVIEW_JS % (DROP % json.dumps([image])))
        else:
            kill(); p = launch("new", {"SKID_INPUTS": image, "SKID_PREVIEW": "1", "SKID_BENCH": "1"}); ms = new_timed(p, "preview")
        results["preview_ms"][kind].append(ms); time.sleep(3)
        results["preview_pss_mb"][kind].append(round(pss_mb(kind), 1))
        # Convert six 1600x1000 PNGs to WebP at the stored settings.
        clear_out()
        if kind == "old":
            kill(); p = launch("old"); time.sleep(5); ms = old_timed(CONVERT_JS % (DROP % json.dumps(batch), len(batch)), 300)
        else:
            kill(); p = launch("new", {"SKID_INPUTS": ":".join(batch), "SKID_CONVERT": "1", "SKID_BENCH": "1"}); ms = new_timed(p, "convert", 300)
        results["convert6_ms"][kind].append(ms)
        kill()
    print(f"run {run + 1}/{RUNS} done", flush=True)

summary = {k: {kind: {"median": statistics.median([x for x in v if x is not None]) if any(x is not None for x in v) else None, "runs": v} for kind, v in d.items()} for k, d in results.items()}
json.dump(summary, open(f"{S}/bench.json", "w"), indent=1)
for k, d in summary.items():
    print(f"{k:16} old {d['old']['median']!s:>10}   new {d['new']['median']!s:>10}   old runs {d['old']['runs']}   new runs {d['new']['runs']}")

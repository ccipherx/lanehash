#!/usr/bin/env python3
"""Render docs/bench/charts/lanehash.svg from the harness CSVs (no dependencies).

    bench/chart.py [data-dir] [out.svg]   (default docs/bench/lanehash docs/bench/charts/lanehash.svg)

Reads throughput.csv, latency.csv (bench --hashes ... --sizes ...) and workloads.csv
(the workloads binary) and draws four panels: throughput by size, latency by size,
throughput at 64 KiB, HashMap<&str> lookup.
"""
import csv
import sys

DATA = sys.argv[1] if len(sys.argv) > 1 else "docs/bench/lanehash"
OUT = sys.argv[2] if len(sys.argv) > 2 else "docs/bench/charts/lanehash.svg"

# harness row name -> (label, colour); order is the legend order
SERIES = [
    ("lanehash", "lanehash", "#2a78d6"),
    ("aes", "lanehash::aes", "#eb6834"),
    ("gxhash", "gxhash", "#1baf7a"),
    ("xxh3", "xxh3", "#eda100"),
    ("rapidhash-v3", "rapidhash", "#e87ba4"),
    ("foldhash", "foldhash", "#008300"),
    ("ahash", "ahash", "#4a3aa7"),
]
COLOUR = {k: c for k, _, c in SERIES}
LABEL = {k: l for k, l, _ in SERIES}
# workloads.csv hasher names -> series key (std has no line series)
MAP_ROWS = {"lanehash": "lanehash", "aes": "aes", "gxhash": "gxhash", "rapidhash-fast": "rapidhash-v3",
            "foldhash-fast": "foldhash", "ahash": "ahash", "xxh3": "xxh3", "std-siphash": "std"}
FONT = "-apple-system,Segoe UI,Helvetica,Arial,sans-serif"
INK, MUTED, GRID = "#0b0b0b", "#52514e", "#e8e7e4"


def read_bench(path, col):
    """{hash: {size: value}} from a bench CSV; `col` is the value column name."""
    out, head = {}, None
    with open(path) as f:
        for r in csv.reader(f):
            if not r:
                continue
            if r[0].startswith("# hash"):
                head = [r[0][2:]] + r[1:]
            elif head and not r[0].startswith("#"):
                d = dict(zip(head, r))
                out.setdefault(d["hash"], {})[int(d["size"])] = float(d[col])
    return out


def read_workloads(path):
    """{series key: words_lookup cycles}."""
    out = {}
    with open(path) as f:
        for r in f:
            if r.startswith("#") or "," not in r:
                continue
            p = r.strip().split(",")
            name = p[0]
            key = MAP_ROWS.get(name) or MAP_ROWS.get(name.split(" ")[0].lower())
            if key and key not in out:
                out[key] = float(p[2])
    return out


def fmt_size(n):
    return f"{n} B" if n < 1024 else (f"{n // 1024} KiB" if n < 1 << 20 else f"{n >> 20} MiB")


def nice_max(v):
    for step in (5, 10, 20, 25, 50, 100, 200, 250, 500, 1000):
        if v <= step * 4:
            return step * 4, step
    return v, v / 4


def text(x, y, s, size=11, fill=MUTED, anchor="start", weight="normal"):
    s = s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
    return (f"<text x='{x:.1f}' y='{y:.1f}' font-size='{size}' fill='{fill}' text-anchor='{anchor}' "
            f"font-weight='{weight}' font-family='{FONT}'>{s}</text>")


def line_panel(o, x0, y0, w, h, title, sub, data, sizes, keys):
    """Category x axis (sizes), linear y from 0; one polyline per series."""
    ymax, step = nice_max(max(v for k in keys for s, v in data.get(k, {}).items() if s in sizes))
    px, py, pw, ph = x0 + 40, y0 + 40, w - 50, h - 70
    o.append(text(x0, y0 + 12, title, 13, INK, weight="bold"))
    o.append(text(x0, y0 + 27, sub))
    t = 0
    while t <= ymax + 1e-9:
        y = py + ph - ph * t / ymax
        o.append(f"<line x1='{px}' y1='{y:.1f}' x2='{px + pw}' y2='{y:.1f}' stroke='{GRID}'/>")
        o.append(text(px - 6, y + 4, f"{t:g}", anchor="end"))
        t += step
    xs = {s: px + pw * i / (len(sizes) - 1) for i, s in enumerate(sizes)}
    for s in sizes:
        o.append(text(xs[s], py + ph + 16, fmt_size(s), anchor="middle"))
    for k in keys:
        pts = [(xs[s], py + ph - ph * data[k][s] / ymax) for s in sizes if s in data.get(k, {})]
        if not pts:
            continue
        o.append("<polyline points='" + " ".join(f"{x:.1f},{y:.1f}" for x, y in pts)
                 + f"' fill='none' stroke='{COLOUR[k]}' stroke-width='2' stroke-linejoin='round'/>")
        for x, y in pts:
            o.append(f"<circle cx='{x:.1f}' cy='{y:.1f}' r='3.5' fill='{COLOUR[k]}' stroke='#fff' stroke-width='1.5'/>")


def bar_panel(o, x0, y0, w, h, title, sub, items, unit_fmt):
    """Horizontal bars, `items` = [(key, label, value)] already sorted."""
    ymax, step = nice_max(max(v for _, _, v in items))
    px, py, pw = x0 + 96, y0 + 40, w - 110
    o.append(text(x0, y0 + 12, title, 13, INK, weight="bold"))
    o.append(text(x0, y0 + 27, sub))
    bh = (h - 60) / len(items)
    t = 0
    while t <= ymax + 1e-9:
        x = px + pw * t / ymax
        o.append(f"<line x1='{x:.1f}' y1='{py - 4}' x2='{x:.1f}' y2='{py + bh * len(items)}' stroke='{GRID}'/>")
        o.append(text(x, py + bh * len(items) + 14, f"{t:g}", anchor="middle"))
        t += step
    for i, (k, label, v) in enumerate(items):
        y = py + i * bh
        bw = pw * v / ymax
        o.append(f"<rect x='{px}' y='{y + 3:.1f}' width='{bw:.1f}' height='{bh - 6:.1f}' rx='2' fill='{COLOUR.get(k, '#9a9a9a')}'/>")
        o.append(text(px - 8, y + bh / 2 + 4, label, 11, INK, anchor="end"))
        o.append(text(px + bw + 5, y + bh / 2 + 4, unit_fmt(v), 11, INK))


def main():
    thr = read_bench(f"{DATA}/throughput.csv", "bytes_per_cycle_median")
    lat = read_bench(f"{DATA}/latency.csv", "cycles_per_hash_median")
    maps = read_workloads(f"{DATA}/workloads.csv")
    keys = [k for k, _, _ in SERIES if k in thr]
    W, H = 960, 700
    o = [f"<svg xmlns='http://www.w3.org/2000/svg' width='{W}' height='{H}' viewBox='0 0 {W} {H}'>",
         f"<rect width='{W}' height='{H}' fill='#fff'/>"]
    x = 20
    for k in keys:
        o.append(f"<rect x='{x}' y='18' width='14' height='4' rx='2' fill='{COLOUR[k]}'/>")
        o.append(text(x + 19, 24, LABEL[k], 12, INK))
        x += 26 + 7 * len(LABEL[k]) + 14
    line_panel(o, 20, 50, 440, 290, "Throughput, independent inputs (cache-resident)",
               "bytes per cycle, higher is better", thr, [16, 256, 1024, 4096, 65536, 1 << 20], keys)
    line_panel(o, 500, 50, 440, 290, "Latency, one hash after another (dependent chain)",
               "cycles per hash, lower is better", lat, [4, 8, 16, 32, 64, 256, 1024], keys)
    bars = sorted(((k, LABEL[k], thr[k][65536]) for k in keys if 65536 in thr[k]), key=lambda t: -t[2])
    bar_panel(o, 20, 370, 440, 280, "Throughput at 64 KiB", "bytes per cycle, higher is better", bars, lambda v: f"{v:.1f}")
    mb = sorted(((k, "std SipHash" if k == "std" else LABEL[k], v) for k, v in maps.items()), key=lambda t: t[2])
    bar_panel(o, 500, 370, 440, 280, "HashMap<&str, _> lookup, 104k English words",
              "cycles per lookup, lower is better", mb, lambda v: f"{v:.1f}")
    o.append(text(20, H - 12, "One core (Zen 4), -C target-cpu=native; medians of 5 interleaved rounds; "
                  f"cycles via RDPRU/APERF. Data: {DATA}", 10))
    o.append("</svg>")
    with open(OUT, "w") as f:
        f.write("\n".join(o) + "\n")
    print(OUT, "series:", keys, "map rows:", sorted(maps))


if __name__ == "__main__":
    main()

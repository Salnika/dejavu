#!/usr/bin/env python3
"""Render the README benchmark chart from real `dejavu bench` output.

Usage (from the repo root):

    cargo build --release
    python3 scripts/render-benchmark-chart.py

Runs `dejavu bench --json --check` (deterministic, no latency micro-bench)
and writes docs/assets/benchmark-light.svg and benchmark-dark.svg, which the
README embeds via a <picture> element so GitHub serves the right theme.
"""

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / "docs" / "assets"

THEMES = {
    "light": {
        "text": "#1f2328",
        "muted": "#59636e",
        "track": "#eaeef2",
        "bar": "#2da44e",
        "accent": "#0969da",
    },
    "dark": {
        "text": "#e6edf3",
        "muted": "#9198a1",
        "track": "#30363d",
        "bar": "#3fb950",
        "accent": "#58a6ff",
    },
}

WIDTH = 780
LEFT = 190       # label column
RIGHT_PAD = 150  # room for the % + token annotations
ROW_H = 44
BAR_H = 14


def fmt(n: int) -> str:
    return f"{n:,}"


def bench_json() -> dict:
    binary = ROOT / "target" / "release" / "dejavu"
    if not binary.exists():
        sys.exit("build first: cargo build --release")
    out = subprocess.run(
        [str(binary), "bench", "--json", "--check"],
        capture_output=True,
        text=True,
        check=True,
    )
    return json.loads(out.stdout)


def esc(s: str) -> str:
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def render(theme: dict, data: dict) -> str:
    scenarios = data["scenarios"]
    totals = data["totals"]
    bar_w = WIDTH - LEFT - RIGHT_PAD

    rows = []
    y = 64
    for sc in scenarios:
        name = sc["name"]
        if sc["all_passthrough"]:
            # Safety row: passthrough by design, no bar.
            rows.append(
                f'<text x="{LEFT - 12}" y="{y + 11}" text-anchor="end" class="lbl">{esc(name)}</text>'
                f'<text x="{LEFT}" y="{y + 11}" class="mut">passthrough by design — machine-readable git is never reduced ✓</text>'
            )
        else:
            pct = sc["reduction_pct"]
            filled = max(3, round(bar_w * min(pct, 100.0) / 100.0))
            detail = f'{fmt(sc["raw_tokens"])} → {fmt(sc["emitted_tokens"])} tokens'
            rows.append(
                f'<text x="{LEFT - 12}" y="{y + 11}" text-anchor="end" class="lbl">{esc(name)}</text>'
                f'<rect x="{LEFT}" y="{y}" width="{bar_w}" height="{BAR_H}" rx="7" class="track"/>'
                f'<rect x="{LEFT}" y="{y}" width="{filled}" height="{BAR_H}" rx="7" class="bar"/>'
                f'<text x="{LEFT + bar_w + 10}" y="{y + 11}" class="pct">{pct:.1f}%</text>'
                f'<text x="{LEFT}" y="{y + 28}" class="mut sm">{detail}</text>'
            )
        y += ROW_H

    y += 8
    overall = totals["reduction_pct"]
    filled = max(3, round(bar_w * min(overall, 100.0) / 100.0))
    rows.append(
        f'<line x1="24" y1="{y - 10}" x2="{WIDTH - 24}" y2="{y - 10}" class="rule"/>'
        f'<text x="{LEFT - 12}" y="{y + 11}" text-anchor="end" class="lbl b">overall</text>'
        f'<rect x="{LEFT}" y="{y}" width="{bar_w}" height="{BAR_H}" rx="7" class="track"/>'
        f'<rect x="{LEFT}" y="{y}" width="{filled}" height="{BAR_H}" rx="7" class="bar"/>'
        f'<text x="{LEFT + bar_w + 10}" y="{y + 12}" class="pct b">{overall:.1f}%</text>'
        f'<text x="{LEFT}" y="{y + 28}" class="mut sm">{fmt(totals["saved_tokens"])} tokens saved across the suite</text>'
    )
    height = y + 48

    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{height}" viewBox="0 0 {WIDTH} {height}" role="img" aria-label="Dejavu benchmark: token reduction by scenario">
  <style>
    text {{ font-family: -apple-system, 'Segoe UI', Helvetica, Arial, sans-serif; font-size: 13px; fill: {theme['text']}; }}
    .lbl {{ font-size: 13px; }}
    .b   {{ font-weight: 600; }}
    .mut {{ fill: {theme['muted']}; }}
    .sm  {{ font-size: 11px; }}
    .pct {{ font-weight: 600; fill: {theme['bar']}; font-variant-numeric: tabular-nums; }}
    .track {{ fill: {theme['track']}; }}
    .bar {{ fill: {theme['bar']}; }}
    .rule {{ stroke: {theme['track']}; stroke-width: 1; }}
    .title {{ font-size: 15px; font-weight: 600; }}
  </style>
  <text x="24" y="28" class="title">dejavu bench — token reduction by scenario</text>
  <text x="24" y="46" class="mut sm">deterministic suite through the real classify + reduce pipeline · reproduce with `dejavu bench`</text>
  {''.join(rows)}
</svg>
"""


def main() -> None:
    data = bench_json()
    if not data["check"]["passed"]:
        sys.exit(f"bench --check failed: {data['check']['violations']}")
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for name, theme in THEMES.items():
        path = OUT_DIR / f"benchmark-{name}.svg"
        path.write_text(render(theme, data))
        print(f"wrote {path.relative_to(ROOT)}")


if __name__ == "__main__":
    main()

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
        "without": "#8c959f",
        "with": "#2da44e",
        "grid": "#eaeef2",
    },
    "dark": {
        "text": "#e6edf3",
        "muted": "#9198a1",
        "without": "#6e7681",
        "with": "#3fb950",
        "grid": "#30363d",
    },
}

WIDTH = 820
CHART_TOP = 108      # below title + legend
BASELINE = 330       # bars grow up from here
MAX_BAR_H = 190
BAR_W = 30
BAR_GAP = 10
GROUP_PAD_X = 40


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
    n = len(scenarios)
    group_w = (WIDTH - 2 * GROUP_PAD_X) / n
    pair_w = 2 * BAR_W + BAR_GAP

    parts = []
    # Faint gridline at the baseline.
    parts.append(
        f'<line x1="{GROUP_PAD_X - 10}" y1="{BASELINE}" x2="{WIDTH - GROUP_PAD_X + 10}" y2="{BASELINE}" class="rule"/>'
    )

    for i, sc in enumerate(scenarios):
        cx = GROUP_PAD_X + group_w * i + group_w / 2
        x_without = cx - pair_w / 2
        x_with = x_without + BAR_W + BAR_GAP

        raw = sc["raw_tokens"]
        emitted = sc["emitted_tokens"]
        # Bars are scaled per scenario ("without" = full height) so every
        # comparison is readable; absolute token counts are labeled on top.
        h_without = MAX_BAR_H
        h_with = max(3, round(MAX_BAR_H * (emitted / raw))) if raw > 0 else 3

        # Badge above the pair: reduction, or the safety statement. Floor to
        # 0.1% so 99.98% shows as −99.9%, never a misleading −100%.
        if sc["all_passthrough"]:
            badge = '<tspan class="mutb">passthrough ✓</tspan>'
        else:
            floored = int(sc["reduction_pct"] * 10) / 10
            badge = f"−{floored:.1f}%"
        parts.append(
            f'<text x="{cx}" y="{BASELINE - MAX_BAR_H - 34}" text-anchor="middle" class="pct">{badge}</text>'
        )

        # Token counts above each bar.
        parts.append(
            f'<text x="{x_without + BAR_W / 2}" y="{BASELINE - h_without - 6}" text-anchor="middle" class="cnt mut">{fmt(raw)}</text>'
        )
        parts.append(
            f'<text x="{x_with + BAR_W / 2}" y="{BASELINE - h_with - 6}" text-anchor="middle" class="cnt withc">{fmt(emitted)}</text>'
        )

        # The bars.
        parts.append(
            f'<rect x="{x_without}" y="{BASELINE - h_without}" width="{BAR_W}" height="{h_without}" rx="4" class="without"/>'
        )
        parts.append(
            f'<rect x="{x_with}" y="{BASELINE - h_with}" width="{BAR_W}" height="{h_with}" rx="4" class="with"/>'
        )

        # Scenario label under the group.
        parts.append(
            f'<text x="{cx}" y="{BASELINE + 20}" text-anchor="middle" class="lbl">{esc(sc["name"])}</text>'
        )

    # Legend (top right, kept well inside the canvas).
    lx = WIDTH - 320
    parts.append(
        f'<rect x="{lx}" y="60" width="12" height="12" rx="3" class="without"/>'
        f'<text x="{lx + 18}" y="71" class="sm">without Dejavu</text>'
        f'<rect x="{lx + 140}" y="60" width="12" height="12" rx="3" class="with"/>'
        f'<text x="{lx + 158}" y="71" class="sm">with Dejavu</text>'
    )

    totals = data["totals"]
    footer = (
        f'{fmt(totals["saved_tokens"])} tokens saved across the suite '
        f'({totals["reduction_pct"]:.1f}% overall) · bars scaled per scenario, real token counts labeled'
    )
    height = BASELINE + 56

    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{height}" viewBox="0 0 {WIDTH} {height}" role="img" aria-label="Dejavu benchmark: tokens an agent reads per scenario, with and without Dejavu">
  <style>
    text {{ font-family: -apple-system, 'Segoe UI', Helvetica, Arial, sans-serif; font-size: 13px; fill: {theme['text']}; }}
    .lbl {{ font-size: 12.5px; }}
    .sm  {{ font-size: 11.5px; fill: {theme['muted']}; }}
    .cnt {{ font-size: 11px; font-variant-numeric: tabular-nums; }}
    .mut {{ fill: {theme['muted']}; }}
    .mutb {{ fill: {theme['muted']}; font-weight: 600; font-size: 12px; }}
    .withc {{ fill: {theme['with']}; font-weight: 600; }}
    .pct {{ font-weight: 700; font-size: 15px; fill: {theme['with']}; font-variant-numeric: tabular-nums; }}
    .without {{ fill: {theme['without']}; }}
    .with {{ fill: {theme['with']}; }}
    .rule {{ stroke: {theme['grid']}; stroke-width: 1; }}
    .title {{ font-size: 15px; font-weight: 600; }}
  </style>
  <text x="{GROUP_PAD_X - 10}" y="28" class="title">Tokens the agent reads — with vs without Dejavu</text>
  <text x="{GROUP_PAD_X - 10}" y="46" class="sm">deterministic suite through the real pipeline · reproduce with `dejavu bench`</text>
  {''.join(parts)}
  <text x="{GROUP_PAD_X - 10}" y="{BASELINE + 44}" class="sm">{footer}</text>
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

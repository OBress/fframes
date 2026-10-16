"""Regenerate the original numeric title shards and reference ink texture."""
from pathlib import Path
import re
import shutil

ROOT = Path(__file__).resolve().parents[1]
CSS = ROOT / "output/composition-review/reference-main.css"

# The site's thirty scattered pieces, preserving their identity and vertex order.
css = CSS.read_text()
paths = []
for i in range(1, 31):
    rule = re.search(
        rf"\.all-animals \.level-one \.shard-wrap:nth-child\({i}\) \.shard, "
        rf"\.preloader.start-up-seq .*?\{{(.*?)\}}", css, re.S
    ).group(1)
    points = re.findall(r"([\d.]+)% ([\d.]+)%", rule)
    assert len(points) == 3
    points = ", ".join(f"({float(x):.3f}, {float(y):.3f})" for x, y in points)
    paths.append(f"    Path {{ points: [{points}] }},")
(ROOT / "src/title_shards.rs").write_text(
    "//! Scattered navigation shards from the pinned In Pieces CSS.\n"
    "use crate::geometry::Path;\n\n"
    "pub(crate) const SHARDS: [Path; 30] = [\n" + "\n".join(paths) + "\n];\n"
)

# Keep the exact reference's white ink texture at its CSS tile size.
shutil.copyfile(ROOT / "art/source/repeat-white.png", ROOT / "media/title-ink.png")

print("Prepared 30 original numeric shards and the reference title ink.")

"""Convert the stored Natural Earth coastline into numeric SVG path geometry."""

import hashlib
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "art/source/ne_110m_land.geojson"


def simplify(points, tolerance=0.95):
    if len(points) < 3:
        return points
    ax, ay = points[0]
    bx, by = points[-1]
    dx, dy = bx - ax, by - ay
    length = dx * dx + dy * dy
    distances = []
    for x, y in points[1:-1]:
        t = max(0, min(1, ((x - ax) * dx + (y - ay) * dy) / length)) if length else 0
        distances.append((x - ax - t * dx) ** 2 + (y - ay - t * dy) ** 2)
    index = max(range(len(distances)), key=distances.__getitem__) + 1
    if distances[index - 1] <= tolerance * tolerance:
        return [points[0], points[-1]]
    return simplify(points[: index + 1], tolerance)[:-1] + simplify(points[index:], tolerance)


def project(lon, lat):
    """Natural Earth polynomial projection, normalized to 360 SVG units."""
    phi = math.radians(lat)
    phi2 = phi * phi
    phi4 = phi2 * phi2
    x = math.radians(lon) * (0.8707 + phi2 * (-0.131979 + phi2 * (-0.013791 + phi4 * phi2 * (0.003971 - 0.001529 * phi2))))
    y = phi * (1.007226 + phi2 * (0.015085 + phi4 * (-0.044475 + 0.028874 * phi2 - 0.005916 * phi4)))
    return 180 + x * 65.8042718653, 90.9640998927 - y * 65.8042718653


def main():
    geography = json.loads(SOURCE.read_text())
    contours = []
    for feature in geography["features"]:
        geometry = feature["geometry"]
        polygons = [geometry["coordinates"]] if geometry["type"] == "Polygon" else geometry["coordinates"]
        for polygon in polygons:
            if max(p[1] for p in polygon[0]) < -60:
                continue
            for ring in polygon:
                points = simplify([project(lon, lat) for lon, lat in ring])
                if len(points) < 4:
                    points = [project(lon, lat) for lon, lat in ring]
                contours.append(points[:-1])
    lines = ["//! Simplified Natural Earth coastlines for the independently composed habitat atlas.",
             "//! Natural Earth projection, 360 × 160 units. Antarctica is outside the view.",
             "use crate::map::Path;", "", "pub(crate) const LAND: Path<'static> = Path {",
             "    closed: true,", "    contours: &["]
    for contour in contours:
        lines.append("        &[" + ", ".join(f"({x:.3f}, {y:.3f})" for x, y in contour) + "],")
    lines.extend(["    ],", "};", ""])
    (ROOT / "src/map_data.rs").write_text("\n".join(lines))
    manifest = {
        "source": "https://github.com/nvkelso/natural-earth-vector/blob/master/geojson/ne_110m_land.geojson",
        "sha256": hashlib.sha256(SOURCE.read_bytes()).hexdigest(),
        "projection": "Natural Earth polynomial, 360 SVG units wide, latitude 84..-60",
        "projection_reference": "https://github.com/OSGeo/PROJ/blob/master/src/projections/natearth.cpp",
        "geometry": "Numeric Path structs, no SVG path strings or raster map assets",
        "contours": len(contours),
        "vertices": sum(len(p) for p in contours),
        "marker_note": "Representative illustrated habitat, not a complete species range",
    }
    (ROOT / "art/source/map-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"Prepared {manifest['contours']} coastlines and {manifest['vertices']} numeric vertices")


if __name__ == "__main__":
    main()

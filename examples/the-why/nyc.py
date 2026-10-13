#!/usr/bin/env python3
"""Builds dynamic_media/nyc/manhattan.bin, the Manhattan of shot 4 (src/city.rs).

Sources (all free, from the City of New York):
- the NYC 3D Building Model 2014 (CityGML, LOD 1.5, EPSG:2263 US feet) in
  dynamic_media/nyc/DA_WISE_GML.zip, from https://maps.nyc.gov/download/3dmodel/,
- the street centerlines (CSCL), the borough boundaries, the park properties and the
  construction years of the building footprints, from data.cityofnewyork.us.

The script uses the Python standard library only. It downloads the small JSON files when
they are missing, streams the GML out of the zip, keeps the buildings near Midtown with
their roofs and walls as triangles, all other buildings of the five boroughs within 20 km
as rectangles, and writes one binary file. New Jersey comes from OpenStreetMap.

Frame: meters, Y up, origin at the Empire State Building. +Z runs uptown along the avenues
(28.97 degrees east of north), +X runs to the west (toward the Hudson). Each building stands
on Y = 0.

File layout, little endian:
  b"NYC2"
  u32 building count, then for each building:
    u32 BIN, u16 construction year (0 = unknown), u16 flags (1 = far ring)
    f32 height in m, f32 x, f32 z of the footprint center
    u32 roof triangles, u32 wall triangles
    the roof triangles, then the wall triangles: 9 f32 each (x, y, z of three corners),
    counterclockwise seen from outside
  u32 street count, then for each street segment:
    u8 class (0 street, 1 avenue, 2 Broadway, 3 highway), u8 lanes,
    u8 traffic direction (0 two way, 1 along the points, 2 against them), u8 0,
    f32 roadway width in m, u32 point count, 2 f32 (x, z) per point
  u32 route count, then for each avenue route (a chain of avenue segments):
    u8 class, u8 lanes, u8 direction, u8 0, f32 width, u32 point count, 2 f32 per point
  u32 bridge count, then for each bridge: u32 point count, 2 f32 per point
  u32 land triangle count, 6 f32 (x, z of three corners) per triangle
  u32 park triangle count, 6 f32 per triangle
  u32 outer building count, then for each building of the other boroughs, of Manhattan
    outside the near box and of New Jersey, as a rectangle: 6 f32 (x, z of the center,
    half length, half width, angle of the length from +X in radians, height)
  u32 light line count, then for each: u8 kind (0 street, 1 highway, 2 bridge,
    3 park path, 4 ramp), u32 point count, 2 f32 (x, z) per point

Run: ./fetch-nyc.sh, or python3 nyc.py when the zip is there (about one minute)
"""

import json
import math
import struct
import sys
import urllib.parse
import urllib.request
import zipfile
import xml.etree.ElementTree as ET
from pathlib import Path

DIR = Path(__file__).resolve().parent / "dynamic_media" / "nyc"
ZIP = DIR / "DA_WISE_GML.zip"
OUT = DIR / "manhattan.bin"
API = "https://data.cityofnewyork.us/resource/"

# The district areas of the model that the camera sees: Midtown and Lower Manhattan,
# Upper Manhattan, Long Island City and the South Bronx.
AREAS = ["DA12", "DA13", "DA10", "DA14"]

FT = 1200.0 / 3937.0  # one US survey foot in meters
ORIGIN = (988222.21, 211954.22)  # the Empire State Building, EPSG:2263 feet
GRID = math.radians(28.97)  # the avenues run this far east of north
COS, SIN = math.cos(GRID), math.sin(GRID)

# All buildings in the near box. In the far ring only the tall ones, as boxes.
NEAR = (-1500.0, 2200.0, -1300.0, 3000.0)  # x min, x max, z min, z max
OUTER = 20000.0  # the outer buildings reach this far
BOX = 20000.0  # land and parks are clipped to this half size

# --- projection -------------------------------------------------------------------------

A = 6378137.0
E = math.sqrt(2 / 298.257222101 - (1 / 298.257222101) ** 2)


def _m(p):
    return math.cos(p) / math.sqrt(1 - (E * math.sin(p)) ** 2)


def _t(p):
    s = E * math.sin(p)
    return math.tan(math.pi / 4 - p / 2) / ((1 - s) / (1 + s)) ** (E / 2)


P1, P2, P0, L0 = (math.radians(v) for v in (40 + 2 / 3, 41 + 1 / 30, 40 + 1 / 6, -74.0))
N = (math.log(_m(P1)) - math.log(_m(P2))) / (math.log(_t(P1)) - math.log(_t(P2)))
F = _m(P1) / (N * _t(P1) ** N)
R0 = A * F * _t(P0) ** N


def lonlat_to_feet(lon, lat):
    """EPSG:2263 (Lambert conformal conic, NAD83, US feet) of a longitude and latitude."""
    r = A * F * _t(math.radians(lat)) ** N
    th = N * (math.radians(lon) - L0)
    return (300000.0 + r * math.sin(th)) / FT, (R0 - r * math.cos(th)) / FT


def local(e_ft, n_ft):
    """The local x, z in meters of a point in EPSG:2263 feet."""
    de = (e_ft - ORIGIN[0]) * FT
    dn = (n_ft - ORIGIN[1]) * FT
    return -de * COS + dn * SIN, de * SIN + dn * COS


def local_lonlat(lon, lat):
    return local(*lonlat_to_feet(lon, lat))


# --- downloads --------------------------------------------------------------------------


def fetch(name, query):
    path = DIR / name
    if not path.exists():
        print("fetch", name)
        req = urllib.request.Request(API + query, headers={"User-Agent": "fframes-the-why"})
        with urllib.request.urlopen(req, timeout=300) as r:
            path.write_bytes(r.read())
    return json.loads(path.read_text())


# --- triangulation ----------------------------------------------------------------------


def newell(ring):
    nx = ny = nz = 0.0
    for i in range(len(ring)):
        x0, y0, z0 = ring[i]
        x1, y1, z1 = ring[(i + 1) % len(ring)]
        nx += (y0 - y1) * (z0 + z1)
        ny += (z0 - z1) * (x0 + x1)
        nz += (x0 - x1) * (y0 + y1)
    return nx, ny, nz


def area2(pts):
    return sum(pts[i][0] * pts[i - 1][1] - pts[i - 1][0] * pts[i][1] for i in range(len(pts)))


def clean(ring):
    """The ring without the closing point and without repeated points."""
    out = []
    for p in ring:
        if not out or max(abs(p[k] - out[-1][k]) for k in range(len(p))) > 1e-4:
            out.append(p)
    while len(out) > 1 and max(abs(out[0][k] - out[-1][k]) for k in range(len(out[0]))) <= 1e-4:
        out.pop()
    return out


def inside(p, a, b, c):
    def side(u, v, w):
        return (v[0] - u[0]) * (w[1] - u[1]) - (v[1] - u[1]) * (w[0] - u[0])

    return side(a, b, p) >= 0 and side(b, c, p) >= 0 and side(c, a, p) >= 0


def segments_cross(a, b, c, d):
    def orient(p, q, r):
        return (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])

    return (orient(a, b, c) * orient(a, b, d) < 0) and (orient(c, d, a) * orient(c, d, b) < 0)


def bridge_holes(pts, outer, holes):
    """One ring of indices into `pts` (2D) from a counterclockwise outer ring and clockwise
    holes, joined by bridges."""
    poly = list(outer)
    for hole in sorted(holes, key=lambda h: -max(pts[i][0] for i in h)):
        m = max(range(len(hole)), key=lambda i: pts[hole[i]][0])
        hm = pts[hole[m]]

        def visible(j):
            p = pts[poly[j]]
            return all(
                not segments_cross(hm, p, pts[poly[k]], pts[poly[(k + 1) % len(poly)]])
                for k in range(len(poly))
            )

        near = sorted(
            range(len(poly)),
            key=lambda j: (pts[poly[j]][0] - hm[0]) ** 2 + (pts[poly[j]][1] - hm[1]) ** 2,
        )
        best = next((j for j in near if visible(j)), None)
        if best is None:
            continue
        loop = hole[m:] + hole[: m + 1]
        poly = poly[: best + 1] + loop + [poly[best]] + poly[best + 1 :]
    return poly


def ear_clip(pts):
    """Indices of triangles that cover the counterclockwise polygon `pts`."""
    idx = list(range(len(pts)))
    tris = []
    guard = 0
    while len(idx) > 3 and guard < 4 * len(pts) * len(pts):
        guard += 1
        n = len(idx)
        for k in range(n):
            i0, i1, i2 = idx[k - 1], idx[k], idx[(k + 1) % n]
            a, b, c = pts[i0], pts[i1], pts[i2]
            if (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) <= 1e-12:
                continue
            if any(
                inside(pts[j], a, b, c)
                for j in idx
                if j not in (i0, i1, i2) and pts[j] not in (a, b, c)
            ):
                continue
            tris.append((i0, i1, i2))
            del idx[k]
            break
        else:
            break
    if len(idx) == 3:
        tris.append(tuple(idx))
    elif len(idx) > 3:
        # A degenerate rest: a fan is better than a hole.
        tris += [(idx[0], idx[i], idx[i + 1]) for i in range(1, len(idx) - 1)]
    return tris


def triangulate(rings):
    """Triangles (three 3D points) of a planar polygon with holes, facing its normal."""
    rings = [clean(r) for r in rings]
    outer = rings[0]
    if len(outer) < 3:
        return []
    n = newell(outer)
    length = math.sqrt(sum(c * c for c in n))
    if length < 1e-9:
        return []
    n = tuple(c / length for c in n)
    if len(outer) == 4 and len(rings) == 1:
        tris = [(outer[0], outer[1], outer[2]), (outer[0], outer[2], outer[3])]
    else:
        # Project on the plane where the normal has the largest component. In 2D the outer
        # ring goes counterclockwise and the holes clockwise.
        k = max(range(3), key=lambda i: abs(n[i]))
        u, v = [(1, 2), (2, 0), (0, 1)][k]

        def flat(ring):
            return [(p[u], p[v]) for p in ring]

        if area2(flat(outer)) < 0:
            outer = outer[::-1]
        holes = [h if area2(flat(h)) < 0 else h[::-1] for h in rings[1:] if len(h) >= 3]
        points = outer + [p for h in holes for p in h]
        ring = list(range(len(outer)))
        if holes:
            starts = [len(outer)]
            for h in holes[:-1]:
                starts.append(starts[-1] + len(h))
            hole_rings = [list(range(st, st + len(h))) for st, h in zip(starts, holes)]
            ring = bridge_holes(flat(points), ring, hole_rings)
        flat_points = flat(points)
        pts2 = [flat_points[i] for i in ring]
        tris = [(points[ring[a]], points[ring[b]], points[ring[c]]) for a, b, c in ear_clip(pts2)]
    out = []
    for a, b, c in tris:
        tn = newell([a, b, c])
        if sum(tn[i] * n[i] for i in range(3)) < 0:
            b, c = c, b
        out.append((a, b, c))
    return out


# --- buildings --------------------------------------------------------------------------

NS_GML = "{http://www.opengis.net/gml}"
NS_BLDG = "{http://www.opengis.net/citygml/building/1.0}"
NS_GEN = "{http://www.opengis.net/citygml/generics/1.0}"


def polygons(building):
    """(kind, rings) for every polygon of a building. Kind is the surface name."""
    for bounded in building.iter(NS_BLDG + "boundedBy"):
        for surface in bounded:
            kind = surface.tag.replace(NS_BLDG, "")
            for poly in surface.iter(NS_GML + "Polygon"):
                rings = []
                for part in poly:
                    for pl in part.iter(NS_GML + "posList"):
                        v = list(map(float, pl.text.split()))
                        rings.append([(v[i], v[i + 1], v[i + 2]) for i in range(0, len(v) - 2, 3)])
                if rings:
                    yield kind, rings


def buildings(years):
    z = zipfile.ZipFile(ZIP)
    seen = set()
    for area in AREAS:
        name = "DA_WISE_GMLs/%s_3D_Buildings_Merged.gml" % area
        count = 0
        for _, el in ET.iterparse(z.open(name), events=("end",)):
            if el.tag != NS_BLDG + "Building":
                continue
            item = building(el, years, seen)
            el.clear()
            if item:
                count += 1
                yield item
        print(area, count, "buildings", file=sys.stderr)


def building(el, years, seen):
    bin_ = 0
    for attr in el.findall(NS_GEN + "stringAttribute"):
        if attr.get("name") == "BIN":
            value = attr.find(NS_GEN + "value")
            if value is not None and value.text and value.text.isdigit():
                bin_ = int(value.text)
    polys = list(polygons(el))
    pts = [p for _, rings in polys for p in rings[0]]
    if not pts:
        return None
    zmin = min(p[2] for p in pts)
    height = (max(p[2] for p in pts) - zmin) * FT
    ex = sum(p[0] for p in pts) / len(pts)
    ny = sum(p[1] for p in pts) / len(pts)
    cx, cz = local(ex, ny)
    near = NEAR[0] < cx < NEAR[1] and NEAR[2] < cz < NEAR[3]
    if height < 2.0:
        return None
    # The same building can be in two district areas.
    key = (bin_, round(cx), round(cz))
    if key in seen:
        return None
    seen.add(key)
    if not near:
        return None
    roofs, walls = [], []
    for kind, rings in polys:
        if kind == "GroundSurface":
            continue
        rings3 = [[(*local(p[0], p[1]), (p[2] - zmin) * FT) for p in r] for r in rings]
        # (x, z, y) to (x, y, z)
        rings3 = [[(x, y, zz) for x, zz, y in r] for r in rings3]
        for tri in triangulate(rings3):
            n = newell(list(tri))
            nl = math.sqrt(sum(c * c for c in n)) or 1.0
            # Steep roof faces, as on the crowns of the towers, get windows like walls.
            if n[1] / nl > 0.6:
                roofs.append(tri)
            else:
                walls.append(tri)
    if not roofs and not walls:
        return None
    flags = 0 if near else 1
    return bin_, years.get(bin_, 0), flags, height, cx, cz, roofs, walls


def hull(pts):
    """The convex hull of points (x, z), counterclockwise (Andrew's monotone chain)."""
    pts = sorted(set(pts))
    if len(pts) < 3:
        return pts
    cross = lambda o, a, b: (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    lower, upper = [], []
    for p in pts:
        while len(lower) >= 2 and cross(lower[-2], lower[-1], p) <= 0:
            lower.pop()
        lower.append(p)
    for p in reversed(pts):
        while len(upper) >= 2 and cross(upper[-2], upper[-1], p) <= 0:
            upper.pop()
        upper.append(p)
    return lower[:-1] + upper[:-1]


def min_rect(pts):
    """The smallest rectangle around points (x, z): center, half sizes and angle."""
    h = hull(pts)
    if len(h) < 3:
        x, z = h[0] if h else (0.0, 0.0)
        return x, z, 1.0, 1.0, 0.0
    best = None
    for i in range(len(h)):
        (ax, az), (bx, bz) = h[i], h[(i + 1) % len(h)]
        a = math.atan2(bz - az, bx - ax)
        c, s = math.cos(a), math.sin(a)
        us = [x * c + z * s for x, z in h]
        vs = [-x * s + z * c for x, z in h]
        area = (max(us) - min(us)) * (max(vs) - min(vs))
        if best is None or area < best[0]:
            best = (area, a, (max(us) + min(us)) / 2, (max(vs) + min(vs)) / 2,
                    (max(us) - min(us)) / 2, (max(vs) - min(vs)) / 2)
    _, a, u, v, hu, hv = best
    c, s = math.cos(a), math.sin(a)
    return u * c - v * s, u * s + v * c, max(hu, 1.0), max(hv, 1.0), a


def outer_area(area):
    """The buildings of one district area outside the near box and within OUTER meters, as
    rectangles: (x, z, half size along, half size across, angle, height)."""
    z = zipfile.ZipFile(ZIP)
    name = "DA_WISE_GMLs/%s_3D_Buildings_Merged.gml" % area
    out = []
    for _, el in ET.iterparse(z.open(name), events=("end",)):
        # The areas use CityGML 1.0 or 2.0, so the tags are matched by their local name.
        if not el.tag.endswith("}Building"):
            continue
        ground, zs = [], []
        for surface in el.iter():
            kind = surface.tag.rsplit("}", 1)[-1]
            if not kind.endswith("Surface") or kind == "MultiSurface":
                continue
            for pl in surface.iter():
                if not pl.tag.endswith("}posList"):
                    continue
                v = list(map(float, pl.text.split()))
                zs += v[2::3]
                if kind == "GroundSurface":
                    ground += [local(v[i], v[i + 1]) for i in range(0, len(v) - 2, 3)]
        el.clear()
        if not ground or not zs:
            continue
        height = (max(zs) - min(zs)) * FT
        cx = sum(p[0] for p in ground) / len(ground)
        cz = sum(p[1] for p in ground) / len(ground)
        if height < 3.0 or math.hypot(cx, cz) > OUTER:
            continue
        if NEAR[0] < cx < NEAR[1] and NEAR[2] < cz < NEAR[3]:
            continue
        x, zz, hu, hv, a = min_rect(ground)
        out.append((round(x, 1), round(zz, 1), round(hu, 1), round(hv, 1), round(a, 4), round(height, 1)))
    print(area, len(out), "outer buildings", file=sys.stderr)
    return out


def outer():
    """The buildings of all five boroughs around the near box, read in parallel. The result
    is kept in outer.json, so later runs skip the 15 GB of GML."""
    from concurrent.futures import ProcessPoolExecutor

    cache = DIR / "outer.json"
    if cache.exists():
        return [tuple(item) for item in json.loads(cache.read_text())]
    areas_all = ["DA%d" % i for i in range(1, 21)]
    seen, out = set(), []
    with ProcessPoolExecutor(max_workers=8) as pool:
        for part in pool.map(outer_area, areas_all):
            for item in part:
                key = (round(item[0]), round(item[1]))
                if key not in seen:
                    seen.add(key)
                    out.append(item)
    cache.write_text(json.dumps(out))
    return out


def land_grid(tris, cell=40.0):
    """The cells of a grid of `cell` meters that lie on the land of the city, as a set."""
    grid = set()
    for (ax, az), (bx, bz), (cx, cz) in tris:
        x0, x1 = int(min(ax, bx, cx) // cell), int(max(ax, bx, cx) // cell)
        z0, z1 = int(min(az, bz, cz) // cell), int(max(az, bz, cz) // cell)
        for i in range(x0, x1 + 1):
            for j in range(z0, z1 + 1):
                p = ((i + 0.5) * cell, (j + 0.5) * cell)
                if inside(p, (ax, az), (bx, bz), (cx, cz)) or inside(p, (ax, az), (cx, cz), (bx, bz)):
                    grid.add((i, j))
    return grid


def height_tag(tags):
    """The height of an OSM building in meters, from its height or its number of floors."""
    text = (tags.get("height") or "").replace("m", "").strip()
    try:
        return float(text)
    except ValueError:
        pass
    try:
        return float(tags.get("building:levels", "")) * 3.3 + 2.0
    except ValueError:
        return 0.0


def new_jersey(land_tris):
    """The buildings of Jersey City, Hoboken, Weehawken, Edgewater and Fort Lee from
    OpenStreetMap, as rectangles like the outer buildings. The city model ends at the
    Hudson, so they come from OSM (ODbL): only the buildings with a height or floors."""
    path = DIR / "nj_osm.json"
    if not path.exists():
        print("fetch", path.name)
        query = (
            '[out:json][timeout:280];(way["building"]["height"](40.68,-74.10,40.88,-73.95);'
            'way["building"]["building:levels"](40.68,-74.10,40.88,-73.95););out geom qt;'
        )
        data = urllib.parse.urlencode({"data": query}).encode()
        req = urllib.request.Request(
            "https://overpass-api.de/api/interpreter", data, {"User-Agent": "fframes-the-why"}
        )
        with urllib.request.urlopen(req, timeout=300) as r:
            path.write_bytes(r.read())
    grid = land_grid(land_tris)
    out = []
    for el in json.loads(path.read_text())["elements"]:
        pts = [local_lonlat(g["lon"], g["lat"]) for g in el.get("geometry", [])]
        height = height_tag(el.get("tags", {}))
        if len(pts) < 3 or height < 3.0:
            continue
        cx = sum(p[0] for p in pts) / len(pts)
        cz = sum(p[1] for p in pts) / len(pts)
        # The city of New York is in the model already.
        if (int(cx // 40.0), int(cz // 40.0)) in grid:
            continue
        x, z, hu, hv, a = min_rect(pts)
        out.append((round(x, 1), round(z, 1), round(hu, 1), round(hv, 1), round(a, 4), round(height, 1)))
    print("New Jersey", len(out), "buildings", file=sys.stderr)
    return out


def light_lines():
    """The streets, highways, bridges, ramps and park paths of all five boroughs, for the
    street lights of the far city and the lamps of the parks."""
    rows = fetch(
        "centerline_all.json",
        "inkn-q76z.json?$select=the_geom,rw_type,boroughcode"
        "&$where=rw_type%20in('1','2','3','6','9')&$limit=300000",
    )
    kinds = {"1": 0, "2": 1, "3": 2, "6": 3, "9": 4}
    out = []
    for r in rows:
        geom = r.get("the_geom")
        if not geom or r.get("rw_type") not in kinds:
            continue
        for line in geom["coordinates"]:
            pts = [local_lonlat(lon, lat) for lon, lat in line]
            if any(math.hypot(x, z) < OUTER for x, z in pts):
                out.append((kinds[r["rw_type"]], simplify_line(pts, 1.0)))
    return out


def simplify_line(pts, tol):
    """Douglas-Peucker on an open line."""
    if len(pts) < 3:
        return pts
    (ax, az), (bx, bz) = pts[0], pts[-1]
    dx, dz = bx - ax, bz - az
    length = math.hypot(dx, dz) or 1.0
    best, index = 0.0, 0
    for i in range(1, len(pts) - 1):
        d = abs((pts[i][0] - ax) * dz - (pts[i][1] - az) * dx) / length
        if d > best:
            best, index = d, i
    if best < tol:
        return [pts[0], pts[-1]]
    return simplify_line(pts[: index + 1], tol)[:-1] + simplify_line(pts[index:], tol)


# --- streets, land, parks ---------------------------------------------------------------


def street_class(name, rw_type):
    if rw_type == "2":
        return 3
    if name == "BROADWAY":
        return 2
    if " AVE" in " " + name or name.endswith("AVE") or "AVE " in name:
        return 1
    return 0


def streets():
    rows = fetch(
        "centerline.json",
        "inkn-q76z.json?$select=the_geom,full_street_name,streetwidth,trafdir,rw_type,"
        "number_travel_lanes,from_level_code,to_level_code&boroughcode=1&$limit=50000",
    )
    out = []
    for r in rows:
        if r.get("rw_type") not in ("1", "2") or r.get("from_level_code") != "13":
            continue
        name = (r.get("full_street_name") or "").strip()
        cls = street_class(name, r.get("rw_type"))
        width = float(r.get("streetwidth") or 0) * FT or (20.0 if cls else 10.0)
        lanes = int(r.get("number_travel_lanes") or 1)
        direction = {"FT": 1, "TF": 2}.get(r.get("trafdir"), 0)
        for line in r["the_geom"]["coordinates"]:
            pts = [local_lonlat(lon, lat) for lon, lat in line]
            if all(abs(x) > BOX * 0.5 or abs(z) > BOX * 0.5 for x, z in pts):
                continue
            out.append((cls, lanes, direction, width, name, pts))
    return out


def routes(segments):
    """Chains of avenue segments with the same name into long routes."""
    groups = {}
    for seg in segments:
        if seg[0] in (1, 2):
            groups.setdefault(seg[4], []).append(seg)
    out = []
    key = lambda p: (round(p[0]), round(p[1]))
    for name, segs in groups.items():
        # Orient each segment in its direction of traffic.
        lines = []
        for cls, lanes, direction, width, _, pts in segs:
            lines.append((cls, lanes, direction, width, pts[::-1] if direction == 2 else pts))
        ends = {}
        for i, line in enumerate(lines):
            ends.setdefault(key(line[4][0]), []).append(i)
        used = set()
        starts = {key(line[4][-1]) for line in lines}
        order = sorted(range(len(lines)), key=lambda i: key(lines[i][4][0]) in starts)
        for i in order:
            if i in used:
                continue
            chain = list(lines[i][4])
            used.add(i)
            cls, lanes, direction, width = lines[i][:4]
            while True:
                nxt = [j for j in ends.get(key(chain[-1]), []) if j not in used]
                if not nxt:
                    break
                used.add(nxt[0])
                chain += lines[nxt[0]][4][1:]
            if len(chain) > 2:
                out.append((cls, lanes, 1 if direction else 0, width, chain))
    return out


def bridges():
    rows = fetch(
        "bridges.json",
        "inkn-q76z.json?$select=the_geom,full_street_name&rw_type=3&$limit=5000",
    )
    out = []
    for r in rows:
        for line in r["the_geom"]["coordinates"]:
            pts = [local_lonlat(lon, lat) for lon, lat in line]
            if any(math.hypot(x, z) < BOX for x, z in pts):
                out.append(pts)
    return out


def simplify(pts, tol):
    """Douglas-Peucker on a closed ring."""
    if len(pts) < 4:
        return pts

    def dp(a, b):
        if b - a < 2:
            return [pts[a]]
        (x0, z0), (x1, z1) = pts[a], pts[b]
        dx, dz = x1 - x0, z1 - z0
        length = math.hypot(dx, dz) or 1e-9
        far, worst = 0.0, a
        for i in range(a + 1, b):
            d = abs((pts[i][0] - x0) * dz - (pts[i][1] - z0) * dx) / length
            if d > far:
                far, worst = d, i
        if far < tol:
            return [pts[a]]
        return dp(a, worst) + dp(worst, b)

    sys.setrecursionlimit(100000)
    # A closed ring: split it at the point farthest from the first point.
    k = max(range(len(pts)), key=lambda i: math.hypot(pts[i][0] - pts[0][0], pts[i][1] - pts[0][1]))
    return dp(0, k) + dp(k, len(pts) - 1)


def clip(ring, half):
    """The ring clipped to the square -half..half (Sutherland-Hodgman)."""
    for axis, sign in ((0, 1), (0, -1), (1, 1), (1, -1)):
        out = []
        for i in range(len(ring)):
            p, q = ring[i - 1], ring[i]
            pin, qin = p[axis] * sign <= half, q[axis] * sign <= half
            if pin != qin:
                t = (half * sign - p[axis]) / (q[axis] - p[axis])
                out.append((p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t))
            if qin:
                out.append(q)
        ring = out
        if not ring:
            break
    return ring


def fill(rings):
    """Triangles that cover the area inside `rings` (closed rings of x, z) by the even-odd
    rule. Each band between two vertex heights becomes trapezoids. It is fast enough for
    shorelines with thousands of points, and holes and islands need no special care."""
    edges = [
        (p, q)
        for ring in rings
        for p, q in zip(ring, ring[1:] + ring[:1])
        if p[1] != q[1]
    ]
    edges.sort(key=lambda e: min(e[0][1], e[1][1]))
    ys = sorted({p[1] for ring in rings for p in ring})
    tris = []
    active = []
    next_edge = 0
    for y0, y1 in zip(ys, ys[1:]):
        while next_edge < len(edges) and min(edges[next_edge][0][1], edges[next_edge][1][1]) < y1:
            active.append(edges[next_edge])
            next_edge += 1
        active = [e for e in active if max(e[0][1], e[1][1]) > y0]
        ym = (y0 + y1) / 2
        xs = []
        for p, q in active:
            if (p[1] - ym) * (q[1] - ym) < 0:
                a = p[0] + (q[0] - p[0]) * (y0 - p[1]) / (q[1] - p[1])
                b = p[0] + (q[0] - p[0]) * (y1 - p[1]) / (q[1] - p[1])
                xs.append(((a + b) / 2, a, b))
        xs.sort()
        for i in range(0, len(xs) - 1, 2):
            (_, a0, a1), (_, b0, b1) = xs[i], xs[i + 1]
            tris.append(((a0, y0), (b0, y0), (b1, y1)))
            tris.append(((a0, y0), (b1, y1), (a1, y1)))
    return tris


def areas(multipolygons, tol):
    """Triangles (x, z) of polygons given as GeoJSON multipolygon coordinates."""
    tris = []
    for mp in multipolygons:
        for poly in mp:
            rings = []
            for ring in poly:
                pts = [local_lonlat(lon, lat) for lon, lat in ring]
                pts = clip(simplify(pts, tol), BOX)
                if len(pts) >= 3:
                    rings.append(pts)
            tris += fill(rings)
    return tris


def land():
    rows = fetch("boroughs.json", "gthc-hcne.json")
    return areas([r["the_geom"]["coordinates"] for r in rows], 6.0)


def parks():
    rows = fetch("parks.json", "enfh-gkve.json?$select=signname,multipolygon&borough=M&$limit=5000")
    return areas([r["multipolygon"]["coordinates"] for r in rows if r.get("multipolygon")], 2.0)


# --- output -----------------------------------------------------------------------------


def main():
    rows = fetch(
        "years.json",
        "5zhs-2jue.json?$select=bin,construction_year"
        "&$where=bin%20between%20'1000000'%20and%20'1999999'&$limit=60000",
    )
    years = {int(r["bin"]): int(r["construction_year"]) for r in rows if r.get("construction_year")}
    out = bytearray(b"NYC2")
    items = list(buildings(years))
    out += struct.pack("<I", len(items))
    tris = 0
    for bin_, year, flags, height, cx, cz, roofs, walls in items:
        out += struct.pack("<IHHfffII", bin_, min(year, 65535), flags, height, cx, cz, len(roofs), len(walls))
        for tri in roofs + walls:
            out += struct.pack("<9f", *(c for p in tri for c in p))
        tris += len(roofs) + len(walls)
    print("buildings", len(items), "triangles", tris, file=sys.stderr)
    segments = streets()
    out += struct.pack("<I", len(segments))
    for cls, lanes, direction, width, _, pts in segments:
        out += struct.pack("<BBBBfI", cls, min(lanes, 255), direction, 0, width, len(pts))
        out += struct.pack("<%df" % (2 * len(pts)), *(c for p in pts for c in p))
    chains = routes(segments)
    out += struct.pack("<I", len(chains))
    for cls, lanes, direction, width, pts in chains:
        out += struct.pack("<BBBBfI", cls, min(lanes, 255), direction, 0, width, len(pts))
        out += struct.pack("<%df" % (2 * len(pts)), *(c for p in pts for c in p))
    print("streets", len(segments), "routes", len(chains), file=sys.stderr)
    spans = bridges()
    out += struct.pack("<I", len(spans))
    for pts in spans:
        out += struct.pack("<I", len(pts)) + struct.pack("<%df" % (2 * len(pts)), *(c for p in pts for c in p))
    land_tris = land()
    for name, tri_list in (("land", land_tris), ("parks", parks())):
        out += struct.pack("<I", len(tri_list))
        for tri in tri_list:
            out += struct.pack("<6f", *(c for p in tri for c in p))
        print(name, len(tri_list), "triangles", file=sys.stderr)
    blocks = outer() + new_jersey(land_tris)
    out += struct.pack("<I", len(blocks))
    for block in blocks:
        out += struct.pack("<6f", *block)
    lines = light_lines()
    out += struct.pack("<I", len(lines))
    for kind, pts in lines:
        out += struct.pack("<BI", kind, len(pts))
        out += struct.pack("<%df" % (2 * len(pts)), *(c for p in pts for c in p))
    print("outer", len(blocks), "light lines", len(lines), file=sys.stderr)
    OUT.write_bytes(bytes(out))
    print("wrote", OUT, len(out) // 1000000, "MB", file=sys.stderr)


if __name__ == "__main__":
    main()

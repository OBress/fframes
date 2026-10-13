#!/usr/bin/env bash
# Downloads the San Francisco data of shot 8 (the Golden Gate) into dynamic_media/sf and
# reduces it to three small binary files that the montage reads:
#
# - terrain.bin: the USGS 3DEP elevation of the Golden Gate, Marin and San Francisco.
# - towers.bin: the buildings of San Francisco taller than 12 m, as boxes that can taper.
# - lights.bin: the center of each building of San Francisco, for the lights of the city.
#
# Sources: USGS 3DEP (public domain) and DataSF "Building Footprints" (ynuv-fyni, PDDL).
# The world space of the montage: X to the east, Y up, Z to the south, in meters. The
# origin is the middle of the main span of the bridge at sea level.
set -euo pipefail
cd "$(dirname "$0")"
out=dynamic_media/sf
mkdir -p "$out"

# The elevation service answers only small images, so it comes in 4 x 4 tiles of 512 x 512
# pixels with square pixels in degrees, from the north-west corner -122.53, 37.87.
dem="https://elevation.nationalmap.gov/arcgis/rest/services/3DEPElevation/ImageServer/exportImage"
for j in 0 1 2 3; do
    for i in 0 1 2 3; do
        file="$out/dem_${i}_${j}.tif"
        if [ ! -f "$file" ]; then
            echo "elevation, tile $i $j..."
            bbox=$(python3 -c "p = 0.15 / 2048 * 512; print(f'{-122.53 + $i * p},{37.87 - ($j + 1) * p},{-122.53 + ($i + 1) * p},{37.87 - $j * p}')")
            curl -sSf --retry 4 -o "$file" "$dem?bbox=$bbox&bboxSR=4326&imageSR=4326&size=512,512&format=tiff&pixelType=F32&noData=-9999&compression=None&f=image"
        fi
    done
done

buildings="https://data.sf.gov/resource/ynuv-fyni.json"
for page in 0 1 2 3; do
    file="$out/buildings_$page.json"
    if [ ! -f "$file" ]; then
        echo "buildings, page $page..."
        curl -sSf -G "$buildings" \
            --data-urlencode '$select=hgt_median_m,gnd_min_m,peak_1st_m,shape' \
            --data-urlencode '$order=:id' \
            --data-urlencode '$limit=50000' \
            --data-urlencode "\$offset=$((page * 50000))" \
            -o "$file"
    fi
done

python3 - "$out" <<'PY'
import json, math, struct, sys

out = sys.argv[1]
# the middle of the main span (OpenStreetMap), the origin of the world
LAT0, LON0 = 37.819719, -122.478610
EAST = math.cos(math.radians(LAT0)) * 111320.0
NORTH = 110540.0

def world(lon, lat):
    return (lon - LON0) * EAST, -(lat - LAT0) * NORTH

# --- terrain: uncompressed float TIFF tiles ------------------------------------------------
def tiff(path):
    """The width, the height, the values, the pixel size and the tie point of a TIFF."""
    d = open(path, "rb").read()
    ifd = struct.unpack_from("<I", d, 4)[0]
    tags = {}
    for i in range(struct.unpack_from("<H", d, ifd)[0]):
        tag, typ, cnt, val = struct.unpack_from("<HHII", d, ifd + 2 + i * 12)
        tags[tag] = (typ, cnt, val)

    def values(tag):
        typ, cnt, val = tags[tag]
        size, fmt = {3: (2, "H"), 4: (4, "I"), 12: (8, "d")}[typ]
        if cnt * size <= 4:
            return list(struct.unpack_from("<" + fmt * cnt, struct.pack("<I", val)))
        return list(struct.unpack_from("<" + fmt * cnt, d, val))

    w, h = tags[256][2], tags[257][2]
    assert tags[259][2] == 1, "the elevation must be uncompressed"
    pixels = [0.0] * (w * h)
    tw, th = tags[322][2], tags[323][2]
    across = (w + tw - 1) // tw
    for t, o in enumerate(values(324)):
        tx, ty = t % across, t // across
        tile = struct.unpack_from("<%df" % (tw * th), d, o)
        for j in range(th):
            y = ty * th + j
            if y >= h:
                break
            n = min(tw, w - tx * tw)
            pixels[y * w + tx * tw:y * w + tx * tw + n] = tile[j * tw:j * tw + n]
    return w, h, pixels, values(33550), values(33922)

p = 0.15 / 2048
w, h = 2048, 2048
heights = [0.0] * (w * h)
for j in range(4):
    for i in range(4):
        tw, th, pixels, scale, tie = tiff(f"{out}/dem_{i}_{j}.tif")
        ox = round((tie[3] + 122.53) / p)
        oy = round((37.87 - tie[4]) / p)
        for y in range(th):
            if 0 <= oy + y < h:
                for x in range(tw):
                    if 0 <= ox + x < w:
                        heights[(oy + y) * w + ox + x] = pixels[y * tw + x]
x0, z0 = world(-122.53, 37.87)
dx, dz = p * EAST, p * NORTH
with open(f"{out}/terrain.bin", "wb") as f:
    f.write(b"SFT1" + struct.pack("<II4f", w, h, x0, z0, dx, dz))
    f.write(struct.pack("<%dh" % (w * h), *(max(-32000, min(32000, round(v * 10))) for v in heights)))
print(f"terrain: {w} x {h}, {dx:.1f} m")

# --- buildings ------------------------------------------------------------------------------
def outer_ring(shape):
    polygons = shape["coordinates"]
    return max((p[0] for p in polygons), key=len)

def number(row, key):
    try:
        return float(row[key])
    except (KeyError, TypeError, ValueError):
        return None

def box(points):
    """The rectangle of the least area around the points: center, half sizes, angle."""
    best = None
    for k in range(45):
        a = math.radians(k * 2.0)
        c, s = math.cos(a), math.sin(a)
        us = [x * c + z * s for x, z in points]
        vs = [-x * s + z * c for x, z in points]
        area = (max(us) - min(us)) * (max(vs) - min(vs))
        if best is None or area < best[0]:
            u, v = (max(us) + min(us)) / 2, (max(vs) + min(vs)) / 2
            best = (area, u * c - v * s, u * s + v * c,
                    (max(us) - min(us)) / 2, (max(vs) - min(vs)) / 2, a)
    return best[1:]

towers, lights = [], []
page = 0
while True:
    try:
        rows = json.load(open(f"{out}/buildings_{page}.json"))
    except FileNotFoundError:
        break
    page += 1
    for row in rows:
        if "shape" not in row:
            continue
        ring = outer_ring(row["shape"])
        points = [world(lon, lat) for lon, lat in ring]
        cx = sum(p[0] for p in points) / len(points)
        cz = sum(p[1] for p in points) / len(points)
        ground = number(row, "gnd_min_m") or 0.0
        median = number(row, "hgt_median_m") or 0.0
        peak = number(row, "peak_1st_m")
        top = (peak - ground) if peak else median
        if top - median > 150:
            continue  # Sutro Tower: the montage builds it as a mast
        lights.append((cx, cz, ground, median))
        if median > 12.0:
            bx, bz, hw, hd, a = box(points)
            # the taper: a box when the median roof is near the peak, a point (a pyramid)
            # when the median is near a third of the peak
            taper = max(0.0, min(1.0, (median / max(top, 1.0) - 0.3) / 0.55))
            towers.append((bx, bz, hw, hd, a, ground, max(top, median), taper))

with open(f"{out}/towers.bin", "wb") as f:
    f.write(b"SFB1" + struct.pack("<I", len(towers)))
    for t in towers:
        f.write(struct.pack("<8f", *t))
with open(f"{out}/lights.bin", "wb") as f:
    f.write(b"SFL1" + struct.pack("<I", len(lights)))
    for cx, cz, ground, median in lights:
        f.write(struct.pack("<4f", cx, cz, ground, median))
print(f"buildings: {len(towers)} towers, {len(lights)} lights")
PY

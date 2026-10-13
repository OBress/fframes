#!/usr/bin/env bash
# Downloads the New York data of shot 4 (the perfect city) into dynamic_media/nyc and
# reduces it to dynamic_media/nyc/manhattan.bin, which src/city.rs reads.
#
# Sources: the NYC 3D Building Model (CityGML, 920 MB) of the NYC Department of City
# Planning, and the street centerlines, borough boundaries, parks and building footprints
# of NYC Open Data. nyc.py downloads the open data and documents the file layout.
# Safe to re-run: it skips the download when the zip is already there.
set -euo pipefail
cd "$(dirname "$0")"
out=dynamic_media/nyc
mkdir -p "$out"

if [ ! -f "$out/DA_WISE_GML.zip" ]; then
    echo "NYC 3D Building Model (920 MB)..."
    curl -fL --retry 4 -o "$out/DA_WISE_GML.zip.part" \
        "https://maps.nyc.gov/download/3dmodel/DA_WISE_GML.zip"
    mv "$out/DA_WISE_GML.zip.part" "$out/DA_WISE_GML.zip"
fi

python3 nyc.py

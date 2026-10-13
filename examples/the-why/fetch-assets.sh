#!/bin/sh
# Downloads the CC0 Poly Haven assets used by this film into dynamic_media/.
# Safe to re-run: it skips any file that already exists. Stops on the first error.
set -e

UA="fframes-the-why (dmtr.kovalenko@outlook.com)"
DIR="$(cd "$(dirname "$0")" && pwd)/dynamic_media"
mkdir -p "$DIR"

fetch() {
  out="$DIR/$1"
  url="$2"
  if [ -f "$out" ]; then
    echo "skip $1 (already exists)"
    return 0
  fi
  echo "fetch $1"
  curl -fsSL -A "$UA" -o "$out" "$url"
}

# HDRIs (2k, Radiance HDR)
fetch sky_sunrise.hdr      "https://dl.polyhaven.org/file/ph-assets/HDRIs/hdr/2k/rogland_sunset_2k.hdr"
fetch sky_dusk.hdr         "https://dl.polyhaven.org/file/ph-assets/HDRIs/hdr/2k/qwantani_dusk_2_puresky_2k.hdr"
fetch factory_interior.hdr "https://dl.polyhaven.org/file/ph-assets/HDRIs/hdr/2k/machine_shop_02_2k.hdr"

# sand / dunes (dense_sand)
fetch sand_diff.jpg  "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/dense_sand/dense_sand_diff_2k.jpg"
fetch sand_rough.jpg "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/dense_sand/dense_sand_rough_2k.jpg"
fetch sand_nor.jpg   "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/dense_sand/dense_sand_nor_gl_2k.jpg"
fetch sand_ao.jpg    "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/dense_sand/dense_sand_ao_2k.jpg"

# dry cracked ground / dusty soil (dry_ground_01)
fetch dirt_diff.jpg  "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/dry_ground_01/dry_ground_01_diff_2k.jpg"
fetch dirt_rough.jpg "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/dry_ground_01/dry_ground_01_rough_2k.jpg"
fetch dirt_nor.jpg   "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/dry_ground_01/dry_ground_01_nor_gl_2k.jpg"
fetch dirt_ao.jpg    "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/dry_ground_01/dry_ground_01_ao_2k.jpg"

# canyon rock (cliff_side)
fetch canyon_rock_diff.jpg  "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/cliff_side/cliff_side_diff_2k.jpg"
fetch canyon_rock_rough.jpg "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/cliff_side/cliff_side_rough_2k.jpg"
fetch canyon_rock_nor.jpg   "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/cliff_side/cliff_side_nor_gl_2k.jpg"
fetch canyon_rock_ao.jpg    "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/cliff_side/cliff_side_ao_2k.jpg"

# worn concrete floor (concrete_floor_worn_001)
fetch concrete_diff.jpg  "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/concrete_floor_worn_001/concrete_floor_worn_001_diff_2k.jpg"
fetch concrete_rough.jpg "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/concrete_floor_worn_001/concrete_floor_worn_001_rough_2k.jpg"
fetch concrete_nor.jpg   "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/concrete_floor_worn_001/concrete_floor_worn_001_nor_gl_2k.jpg"
fetch concrete_ao.jpg    "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/concrete_floor_worn_001/concrete_floor_worn_001_ao_2k.jpg"

# painted industrial metal (blue_metal_plate)
fetch painted_metal_diff.jpg  "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/blue_metal_plate/blue_metal_plate_diff_2k.jpg"
fetch painted_metal_rough.jpg "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/blue_metal_plate/blue_metal_plate_rough_2k.jpg"
fetch painted_metal_nor.jpg   "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/blue_metal_plate/blue_metal_plate_nor_gl_2k.jpg"
fetch painted_metal_ao.jpg    "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/blue_metal_plate/blue_metal_plate_ao_2k.jpg"

# bare worn steel (metal_plate_02)
fetch steel_diff.jpg  "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/metal_plate_02/metal_plate_02_diff_2k.jpg"
fetch steel_rough.jpg "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/metal_plate_02/metal_plate_02_rough_2k.jpg"
fetch steel_nor.jpg   "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/metal_plate_02/metal_plate_02_nor_gl_2k.jpg"
fetch steel_ao.jpg    "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/metal_plate_02/metal_plate_02_ao_2k.jpg"
fetch steel_metal.jpg "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/metal_plate_02/metal_plate_02_metal_2k.jpg"

# corrugated metal sheet (corrugated_iron_02)
fetch corrugated_metal_diff.jpg  "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/corrugated_iron_02/corrugated_iron_02_diff_2k.jpg"
fetch corrugated_metal_rough.jpg "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/corrugated_iron_02/corrugated_iron_02_rough_2k.jpg"
fetch corrugated_metal_nor.jpg   "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/corrugated_iron_02/corrugated_iron_02_nor_gl_2k.jpg"
fetch corrugated_metal_ao.jpg    "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/corrugated_iron_02/corrugated_iron_02_ao_2k.jpg"
fetch corrugated_metal_metal.jpg "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/corrugated_iron_02/corrugated_iron_02_metal_2k.jpg"

# asphalt / road, for the city seen from above (aerial_asphalt_01)
fetch asphalt_diff.jpg  "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/aerial_asphalt_01/aerial_asphalt_01_diff_2k.jpg"
fetch asphalt_rough.jpg "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/aerial_asphalt_01/aerial_asphalt_01_rough_2k.jpg"
fetch asphalt_nor.jpg   "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/aerial_asphalt_01/aerial_asphalt_01_nor_gl_2k.jpg"
fetch asphalt_ao.jpg    "https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/aerial_asphalt_01/aerial_asphalt_01_ao_2k.jpg"

echo "done: $DIR"

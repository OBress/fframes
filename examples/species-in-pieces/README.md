# Living Pieces

An fframes adaptation of [Bryan James’s In Pieces](https://species-in-pieces.com/), following the first eighteen animals in the original order. Thirty persistent numeric SVG paths change into each animal. Eighteen distinct native habitats are constructed from additional numeric triangle paths, with articulated foliage, flowing water, light and atmospheric motion.

**196 seconds at 1920 × 1080 and 60 fps.** An eight-second opening leads into eighteen ten-second animal scenes and an eight-second ending. The film uses the original polygons, colours, per-piece transition delays and CSS easing. Each species has its own connected joint rig, a distinct action and an illustrated habitat. The morph begins at the actual final pose of the previous action, including the turtle’s position at the right edge after swimming across the screen. There is no whole-animal squash or stretch. The earlier six-animal experiment is preserved in `output/previous-six-species-source/` and `output/wild-in-pieces.mp4`.

```sh
cd /Users/dkovalenko/dev/fframes
cargo run --release -p species-in-pieces -- preview 0s --backend metal
cargo run --release -p species-in-pieces -- preview 58s --backend metal
cargo run --release -p species-in-pieces -- render --renderer gpu -o examples/species-in-pieces/output/living-pieces.mp4
```

Space pauses; `h` / `l` seek one second; `j` / `k` step one frame; `q` closes the native player.

## What comes from the reference

The [original source](https://github.com/bryanjamesinteractive/in-pieces/tree/d81b692dc3aca4b3e4e7f5e9f8d9635094faea05) is pinned in `source-manifest.json`, including source URLs and SHA-256 hashes. Artwork and original motion are by Bryan James. This example is a reference adaptation, not a claim of original illustration authorship.

- Forward morph, one-based piece `n`: clip duration `0.30 + 0.04*n` seconds, delay `0.20 + 0.02*n`, cubic Bézier `(0.7, 0.3, 0, 1)`. Colour starts 0.2 seconds earlier and uses CSS `ease`. Species-specific overrides are retained.
- The original base poses, blink polygons and anatomical painter order remain the starting artwork. The original flat backgrounds and three peripheral decorations are replaced by the illustrated environments.
- `src/choreography.rs` retains the imported four-state CSS evaluator, including interrupted/reversed transitions, for reference. The final film uses `src/performance.rs` for habitat-aware actions instead of replaying the website’s independent idle shards.

The interactive website has **no automatic sequential dwell time**. This film chooses ten seconds per animal. Most original staggered morphs finish by 2.3 seconds; the sloth's species-specific transition lasts 6.3 seconds. The evaluator waits for each species' actual final path and colour transition before switching to its action, so the sloth no longer snaps at 2.4 seconds. Its reach follows the completed assembly. The ten-second dwell is an editorial timing choice, not a claimed website constant. The site's separate 2.9-second random slideshow is not the sequential animal experience.

The video omits interactive website controls, the loader, statistics panels and cast-shadow ellipses. The opening and ending assemble a large eighteen from the animal paths, using the reference charcoal and coral palette. Larger flat SVG habitat maps have independent placements that balance each animal. It adapts the framing to 16:9 and adds the requested population captions and an independently synthesized gentle score.

## Actions and contact

The action rig repairs small shared-vertex rounding differences and retains triangle corners that meet another triangle’s edge. Joint rotations use the source’s 100:70 physical aspect ratio. Toes are constrained individually to the visible perch or ground. The penguin has connected ankles and two planted webbed feet; it is not held up by shifting the lowest point of its entire silhouette. The bear and echidna stand on the forest floor. The misplaced vaquita slab and the floating support logs are removed.

| Species | Action within its ten-second scene |
|---|---|
| Helmeted hornbill | Reaches for a fruit cluster, then raises its head |
| Vaquita | Beats its tail and glides forward into a shallow dive |
| Golden lion tamarin | Leans toward fruit while its paws stay on the branch and its tail curls |
| Golden poison frog | Tracks a small insect and catches it with a tongue flick |
| Forest owlet | Scans in both directions with a head tilt and blink |
| Kemp’s ridley turtle | Alternates power strokes and recovery while swimming from the left side to the right, then morphs into the oryx |
| Scimitar oryx | Lowers its neck to browse a shrub, then raises it |
| Fiji crested iguana | Lifts its head to survey the canopy and curls its tail |
| Knysna seahorse | Holds an eelgrass stem with its tail, sways and takes a passing morsel |
| Brazilian three-banded armadillo | Probes the soil, tucks its head and paws into a closed shell, then rolls along the ground |
| Pygmy three-toed sloth | Completes its original slow morph, then keeps two grips and reaches toward a leafy sprig |
| Kākāpō | Takes alternating steps, stops at a berry plant and takes three bites |
| Long-beaked echidna | Probes the litter and scrapes with a forefoot |
| African penguin | Bends its neck to preen its breast, then lifts its flipper |
| Greek red damsel | Flutters its wings, briefly leaves its reed and lands again |
| Sun bear | Sniffs the air and scratches its chest with a forepaw |
| Rainbow parrotfish | Approaches a coral branch, nibbles and swims onward |
| Wild Bactrian camel | Stretches its neck to browse a desert shrub and chews |

The kākāpō’s stance cancels its forward body travel, so a planted toe does not slide. Only its returning foot lifts. The turtle crosses at a constant size and depth; its shell is rigid and the flippers articulate separately. It stays visible at both ends, with the original staggered path morph carrying it into the next animal. Food, perches and coral interactions use the same numeric geometry as their associated animal pose.

## Living vector habitats

`src/habitat.rs` constructs every rock, leaf, branch, coral and water facet as `Path { points: [...] }`. The rendered scene contains no background bitmap and parses no SVG path strings. The generated illustrations in `art/reference/` are composition references only; they are not embedded in the video.

Leaves bend from a fixed attachment point, with separate phases and a four-pose cycle sampled through the original `(0.7, 0.3, 0, 1)` easing. Their facets share the same vertex deformation, so the leaves stay connected. Reeds and coral branches move from their roots; water facets share a continuous displacement field. Light shafts shift and change intensity. Rain, dust and suspended particles have separate timing. Bark facets, leaf ribs, lianas, flowers, branching coral tips and shoreline foam add local detail. Foreground and background geometry surround the animal in separate layers.

| Species | Illustration and motion |
|---|---|
| Helmeted hornbill | Dawn rainforest canopy; lianas, orchids and turning leaves |
| Vaquita | Open Upper Gulf shallows; surface light, sand and drifting particles |
| Golden lion tamarin | Warm Atlantic Forest; bromeliads, vines and a branching perch |
| Golden poison frog | Wet Colombian forest floor; a supporting leaf, rain and a stream |
| Forest owlet | Dry Indian woodland at sunset; gnarled perch and falling leaves |
| Kemp’s ridley turtle | Gulf seagrass meadow; independently bending blades |
| Scimitar oryx | Sahel grassland; acacias, seed grasses and dust |
| Fiji crested iguana | Island dry forest; palm-like fronds, hibiscus and an ocean opening |
| Knysna seahorse | Green estuary; tall eelgrass and rising translucent polygon bubbles |
| Brazilian three-banded armadillo | Caatinga scrub; cactus, thorny trees and dry grass |
| Pygmy three-toed sloth | Island mangroves; varied tapering prop roots anchored in a muddy bank, overhanging branch and water |
| Kākāpō | Twilight forest floor; tree ferns, moss path and a berry sapling |
| Long-beaked echidna | Montane New Guinea forest; mist, tree ferns and leaf litter |
| African penguin | Granite coast; surf behind the dry rock silhouette and succulents, without scattered gem-like pebbles |
| Greek red damsel | Mediterranean stream; reeds, flowers and reflected light |
| Sun bear | Bornean rainforest; buttress trunk, epiphytes and a planted forest floor |
| Rainbow parrotfish | Caribbean reef; branching corals, sea fans and moving light |
| Wild Bactrian camel | Rocky Gobi landscape; layered mountains, scrub and dust |

The names use embedded **Instrument Serif Italic**, with **DM Sans** for population figures and dates. `src/composition.rs` defines a separate composition for every species. Titles occupy quiet sky, water or forest openings, balancing the animal across thirds, golden-section heights and the diagonals of branches and terrain. Six scenes reflect the habitat, animal and interaction props together, placing the subject on the left third without breaking contact with its perch, food or ground. The original morph interpolates between these composed poses, including the ending.

Titles hold a stable position throughout each action. Long names use deliberate line breaks and measured font widths. Most names fade in after the assembly; the sloth's name waits for its longer original morph. Dark inks sit in clear sky, water or mist, while the kākāpō uses cream against its twilight clearing. Foliage, surface facets and horizon lines leave room around each caption. Only documented counts are shown, preserving their dates and units. A species without a usable count shows its name and habitat locator, with no population line. Captions contain no em dashes or dot separators.

Three independent visual audits covered all eighteen full-size scenes and their movement strips. Revisions include a forest opening for the tamarin, clear central water below the turtle, sky placement for the iguana and sloth, a lower reed bank around the damsel, and an open canopy above the bear. Smaller corrections remove leaf and mountain tangencies, keep the penguin's side boulder grounded, and connect the sloth's branch to its trunk. The review records and rendered comparisons are in `output/composition-review/revised/`.

## Opening and ending

The opening and ending share an asymmetric composition: a large ivory **18** built from thirty triangle paths on the left, with **Species in Pieces** in Instrument Serif Italic on the right. A small coral **One shared world** heading and a short DM Sans paragraph explain the film: “Thirty pieces. Eighteen species. A glimpse of their lives and the worlds they depend on.” Fine charcoal seams reveal the individual pieces without adding different colors to the number.

`src/title_number.rs` contains the actual thirty numeric `Path {}` triangles, including the two open counters of the eight. The number is geometry, not a text glyph, mask or replacement image. The opening starts with the reference's scattered triangle silhouettes, confined to the left half so they never cross the copy, and uses the original per-piece delays and cubic easing to assemble them. They hold as the number before morphing directly into the hornbill. At 3:08, the camel's final pose returns to exactly the same eighteen. Piece identities remain continuous through both handoffs.

Both bookends last eight seconds. The paragraph is visible from the first opening frame, and the ending copy arrives as the final pieces settle. At 3:13.3, the eighteen disperses into the exact scattered paths used at frame zero. The final and first GPU frames are pixel-identical, so playback loops without a visual cut. The animal scenes retain their ten-second timing and the sloth's slow assembly. The score includes the longer opening and retains the animal phrases on their original scene clock. The current GPU frames and movement strips are in `output/armadillo-loop-review/`.

`src/armadillo_ball.rs` defines a closed shell using the same thirty numeric `Path {}` facets. The head and paws tuck first, followed by the armour. Rotation drives horizontal travel around successive shell contact points, keeping the roll grounded. The next species morph starts from the actual rolled pose.

## Flat habitat maps

Each scene has a regional atlas drawing, independently composed against the animal. The drawings are 580–700 pixels wide, with a different geographic crop and aspect ratio for each setting. Fine coastlines, faint country boundaries and a light land tint replace the repeated filled world silhouettes. A short leader connects a steady coral marker to the location name. The map and label fade in gently without a repeating pulse. Scene lengths, species order and the original morph clock stay unchanged.

`MAP_COMPOSITIONS` in `src/map.rs` chooses the screen position, size and ink for each habitat. The frog's map balances its leaf across the frame, the owlet's occupies the woodland opening, and the penguin's sits over open water. The turtle's map and two-line name occupy separate lower areas beneath the complete flipper sweep. Selected ferns, reeds, flowers and acacias were moved away from map labels. Soft edges blend the drawings into the scene; local color veils render behind the animal and foreground plants.

`src/map.rs` draws numeric `Path { contours, closed }` values. `src/regional_map_data.rs` contains eighteen precomputed drawings derived from the stored Natural Earth coastlines and country geometry. Each view uses a local equirectangular projection, with longitude scaled by the cosine of its central latitude. The drawing's aspect ratio changes the geographic window without stretching the geography. Date-line wrapping keeps Fiji's Pacific view continuous. Skia Metal draws the paths directly; there are no bitmap maps, SVG path strings or GIS calculations during a frame.

`python3 examples/species-in-pieces/scripts/prepare_regional_maps.py` regenerates the regional geometry and provenance manifest. Run `cargo fmt -p species-in-pieces` afterward. The earlier `prepare_map.py` and `map_data.rs` preserve the whole-world study.

The pins identify representative habitats shown in the illustrations, not complete species ranges or precise animal sightings. Approximate coordinates remain in `art/source/habitats.json`; `art/source/regional-map-manifest.json` records both map sources, their checksums and each regional view. The sloth's Panama marker points to Escudo de Veraguas.

The default treatment is `atlas`; `--map-style coastline|atlas|facets` permits the alternate path treatments. The current full-scene and movement reviews are in `output/regional-map-review/`. The three treatments were compared in the frog, turtle and armadillo scenes before refining all eighteen placements. Earlier map-review folders are historical studies.

The contact revision shares explicit leaf and branch surfaces through `src/supports.rs`. The frog's feet land on its leaf edge, the tamarin's haunch rests on the branch, the iguana's forelegs bend onto the perch, and the armadillo's three visible soles sit into the soil. The thirty animal paths still morph between poses. Repeated gem-like ground pebbles and detached frog dew shards were removed; large habitat rocks remain.

```sh
cargo run --release -p species-in-pieces -- preview 10s --backend metal
cargo run --release -p species-in-pieces -- --map-style atlas preview 10s --backend metal
cargo run --release -p species-in-pieces -- --map-style facets preview 10s --backend metal
```

`python3 examples/species-in-pieces/scripts/prepare_title.py` regenerates the original title shards and ink texture. The old globe experiment, palette gallery and MP4 exports remain historical artifacts; use the native preview to review this revision.

To inspect the moving SVG background on its own:

```sh
cargo run --release -p species-in-pieces -- --habitat-only preview 38s --backend metal
cargo run --release -p species-in-pieces -- --habitat-only frame 42s,43s --svg -o output/backgrounds
```

These SVG exports are individual sampled frames; use the native preview or the video to see their animation.

## Numeric `Path {}` morphing

`src/geometry.rs` defines the editable geometry:

```rust
pub struct Path {
    pub points: [(f32, f32); 3],
}
```

`src/source_data.rs` contains actual `Path { points: [...] }` values. The importer resolves the original CSS cascade, including transition shorthand resets, then writes numeric geometry, colours, timings and state tables. It does not store SVG path strings.

`Performance` precomputes the rigs, shared edges and imported morph easing runtimes. Each transition samples fframes’ `AnimationRuntime` with the source `Easing::CubicBezier`; `Animatable for Path` interpolates corresponding vertices between the previous action’s last pose and the next entrance. `Choreography` also preserves the original CSS state machine for reference. No CSS or SVG text is parsed while rendering.

At the SVG boundary, `From<Path> for SvgAttributeValue` builds typed `PathSegment::MoveTo`, `LineTo`, `LineTo`, and `ClosePath` commands. The actual draw call is `<path d={path} fill={piece.color} />`. The fixed 100:70 aspect ratio matches the source’s illustration box.

To regenerate the imported data (development-only Python dependencies: `tinycss2`, `cssselect2`):

```sh
python examples/species-in-pieces/scripts/import_original.py
cargo fmt -p species-in-pieces
python examples/species-in-pieces/generate_score.py
```

## Sequence and population evidence

Sources checked on 15 October 2026. These are dated estimates or observations, not live counters. Mature individuals, breeding pairs, survey sightings and total animals are distinct units. Population captions are omitted when there is no usable documented count; the research notes below retain the source context.

| Time | Species | Caption basis |
| --- | --- | --- |
| 0:08 | Helmeted hornbill | Global total unquantified, [IUCN / BirdLife 2020](https://doi.org/10.2305/IUCN.UK.2020-3.RLTS.T22682464A184587039.en). |
| 0:18 | Vaquita | 7–10 observed, not a complete census, [2025 survey report](https://iucn-csg.org/wp-content/uploads/2025/11/Report-vaquita-acoustic-and-visual-survey-2025-EN-Nov-11-clean-FINAL.pdf). |
| 0:28 | Golden lion tamarin | About 4,800 wild animals, [AMLD / SGLT 2023 census](https://www.savetheliontamarin.org/faqs/). |
| 0:38 | Golden poison frog | No verified global count; [Citizen Conservation species guidance](https://citizen-conservation.org/wp-content/uploads/2024/10/CC-Breeding-Guidelines_Phyllobates-terribilis_07-2024.pdf). |
| 0:48 | Forest owlet | 250–999 mature individuals, [IUCN / BirdLife 2018 assessment](https://doi.org/10.2305/IUCN.UK.2018-2.RLTS.T22689335A132251554.en). |
| 0:58 | Kemp’s ridley turtle | About 22,000 mature individuals, rounded from the [2019 IUCN assessment](https://www.iucnredlist.org/species/11533/155057916), discussed in [this research paper](https://pmc.ncbi.nlm.nih.gov/articles/PMC11415756/). |
| 1:08 | Scimitar oryx | More than 650 wild animals in Chad, [Sahara Conservation, mid 2026](https://saharaconservation.org/species-recovery/the-scimitar-horned-oryx-reintroduction/). |
| 1:18 | Fiji crested iguana | Approximately 13,000, [International Iguana Foundation](https://www.iguanafoundation.org/what-we-support/fiji-iguanas/fiji-crested-iguana/); no census year specified on the page. |
| 1:28 | Knysna seahorse | Current total unknown. SANParks’ new assessment was still underway in the [6 March 2026 parliamentary response](https://www.wcpp.gov.za/?q=node/20594). |
| 1:38 | Brazilian three-banded armadillo | No verified global total; [IUCN specialist group](https://xenarthrans.org/species/armadillos-2/brazilian-three-banded-armadillo/). |
| 1:48 | Pygmy three-toed sloth | Total uncertain; the [Sloth Conservation Foundation](https://www.slothconservation.org/blog/the-pygmy-sloths-of-panama) describes a dated 79-animal survey, not a current complete census. |
| 1:58 | Kākāpō | 325 alive, [New Zealand Department of Conservation](https://www.doc.govt.nz/our-work/kakapo-recovery/). |
| 2:08 | Long-beaked echidna | The original names the genus *Zaglossus*, not one species. No combined total is asserted. See the [eastern species account](https://animaldiversity.org/accounts/Zaglossus_bartoni/) and [2025 Attenborough’s echidna research](https://ora.ox.ac.uk/objects/uuid:d8e9f841-35e6-47a5-95fa-c3517c756dd4/files/rpn89d9110). |
| 2:18 | African penguin | Fewer than 10,000 breeding pairs, [BirdLife, March 2025](https://www.birdlife.org/news/2025/03/23/landmark-court-victory-secures-critical-protection-for-african-penguins/). |
| 2:28 | Greek red damsel | No verified global total. [IUCN 2024 European review](https://nc.iucnredlist.org/redlist/content/attachment_files/ERL_Pulse_Dragonflies_and_Damselflies_2024.pdf) and [Dragonfly Fund survey](https://dragonflyfund.org/wp-content/uploads/2024/06/IDF_Report_116_Stille_Stille_Corfu_2018.pdf) document its restricted populations. |
| 2:38 | Sun bear | No reliable population estimate, [International Association for Bear Research and Management](https://www.bearbiology.org/the-eight-bear-species/helarctos-malayanus-sun-bear/). |
| 2:48 | Rainbow parrotfish | No verified global count; [research review](https://pmc.ncbi.nlm.nih.gov/articles/PMC3394754/) and [Puerto Rico conservation strategy](https://www.drna.pr.gov/wp-content/uploads/2015/04/Puerto-Ricos-Comprehensive-Wildlife-Conservation-Strategy.pdf). |
| 2:58 | Wild Bactrian camel | About 600 in China and 450 in Mongolia, [Wild Camel Protection Foundation](https://www.wildcamels.com/camelus-ferus/). The caption explicitly refers to *Camelus ferus*, correcting the original site’s use of domestic *C. bactrianus* for its endangered wild-camel story. |

## Verification

```sh
cargo run --release -p species-in-pieces -- timeline
cargo run --release -p species-in-pieces -- inspect --all-frames
cargo run --release -p species-in-pieces -- strip 18s..20.5s -n 12
cargo run --release -p species-in-pieces -- strip 60s..64s -n 12
cargo run --release -p species-in-pieces -- audio analyze --waveform waveform.png
cargo clippy -p species-in-pieces --all-targets -- -D warnings -A linker-messages
```

The example uses the fframes workspace packages and the repository's standard dependencies. No editor bridge is present. The `linker-messages` exception covers deprecated macOS framework linker warnings.

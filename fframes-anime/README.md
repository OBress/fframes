# fframes-anime

Character animation for [fframes](https://github.com/dmtrKovalenko/fframes), built the way Japanese
animation studios build motion and 3DCG characters: motion capture retargeted onto anime
proportions, secondary motion from spring bones and cloth, drawings exposed on twos and threes,
and characters modelled, skinned, cel shaded and inked with vector line art the way an anime 3DCG
department does it (Pencil+ style), composited by fframes.

`examples/anime-op` is the proving ground: a 90 second opening in the style of MASHLE's second
opening ("Bling-Bang-Bang-Born"), every character animated by this crate.

## What is in the crate

| module | what |
| --- | --- |
| `bvh` | Biovision Hierarchy parser and forward kinematics (the format of most mocap libraries) |
| `rig` | humanoid skeleton with editable proportions, poses as world rotations, two-bone IK, `Retarget` (calibrates any BVH rig against the target skeleton once, then copies the motion with the target's bone lengths) |
| `spring` | VRM style spring bones (hair, ties, scarves, tails) and position based cloth (robes, capes), colliders, baked over a time range so frames can render in parallel and in any order |
| `koma` | exposure timing: `hold(t, fps, koma)` for animation on ones, twos and threes, `Exposure` for timesheet columns, and `Sheet`: keys on their frames, in-betweens from timing charts (ツメ / タメ), overshoots and moving holds, the way a key animator times a cut |
| `acting` | expressions over time: expression keys changed through an in-between, natural blinks (and one on every change of mind), eye darts, lip sync from vowels (`a i u e o`) |
| `camera` | perspective camera with roll, lens shift, shadow flattening (`shadow(light)`) and `scaled_about` for compositing giants and miniatures |
| `shape` | path building for flat shapes: capsules, convex hulls, ribbons, Catmull-Rom curves; closed shapes are wound consistently so overlapping strands union cleanly |
| `toon` | character design: a `Design` (hair locks, glasses, beard, hood, robe, tie, scarf, build, muscles), the per-frame `Look` (eyes with gaze, lids, squint and wink; angled and tilted brows; mouths from a line to vowels, teeth, smirks and the ω cat mouth; blush, sweat drops, gloom lines, anger marks, sparkles, tears; hand shapes) and the physics `Rig` (spring chains for every hair lock, tie and scarf, cloth for the robe) |
| `sdf` | signed distance modelling: rounded primitives (round cones, ellipsoids, boxes, tori, half spaces) combined with smooth unions, cuts and clips, each in a material group and bound to a bone |
| `mesh` | surface nets meshing with vertices projected onto the true surface, skin weights from the primitives' bones, the exact curves where material groups meet, linear blend skinning (plus spring chain links as extra bones, the way VRM skins hair), tubes, ribbons and cloth sheets built per frame |
| `figure` | the character builder: body, head (face, hair locks, beard, ears) and five hand poses per side from a `Design`, the posed surfaces of a frame, and the shader that paints faces, jacket seams and buttons per pixel in the rest pose (crisp at any size) |
| `prop` | props (小物) modelled like the characters (rounded primitives in material groups, painted details such as cracks or rings), meshed once and placed per frame in the same depth buffer and line art as the cast, so a fist closes around a bar and a cream puff sits in a palm |
| `raster` | a cel shading rasterizer: depth buffer with four samples per pixel, shaded once per pixel and triangle (deferred, like MSAA), cel shadows with a clean edge, output as a premultiplied image |
| `ink` | vector line art (Pencil+ / Blender Line Art style): contours where `n · v` changes sign, material borders, hems, glasses; every point depth tested against the raster, chained runs become tapered ribbons with pen pressure and a slight wobble that changes with every drawing (line boil on twos), outer contours thicker, fine strand lines dropped when a drawing is small |

Everything is deterministic: the same inputs bake the same physics, so render workers agree.

```rust
use fframes_anime::{bvh::Bvh, rig::{Proportions, Retarget, Skeleton}, spring, koma};

let skeleton = Skeleton::new(Proportions::anime_adult());
let bvh = Bvh::parse(&std::fs::read_to_string("walk.bvh")?, 0.01)?;
let retarget = Retarget::new(&bvh, &skeleton, 0).unwrap();
let (rig, layout) = design.physics();
// bake hair, tie and robe once, on the same held drawings the audience sees
let baked = spring::bake(&rig, 0.0, seconds, 24.0, 10, 0.8, |t| {
    let posed = skeleton.solve(&retarget.pose_at(&bvh, koma::hold(t, 24.0, 2)));
    design.body_state(&posed, wind)
});
// once: the meshes (about a second per character)
let figure = Figure::new(&design, &skeleton, Detail(1.0));
// in render_frame
let th = koma::hold(t, 24.0, 2);
let posed = skeleton.solve(&retarget.pose_at(&bvh, th));
let surfaces = figure.surfaces(&posed, &baked.at(th), &layout, &look, 0);
let proj = camera.projector();
let (x, y, w, h) = raster::bounds(&surfaces, &proj, 1920.0, 1080.0, 16.0).unwrap();
let raster = Raster::new(x, y, w, h, &surfaces, &proj);
let cast = [Cast { figure: &figure, look, palette: &palette }];
let pixels = raster.resolve(&surfaces, &proj, &|f| figure::shade(&cast, f)); // an <image>
let lines = ink::draw(&surfaces, &raster, &proj, &InkStyle::default(), &|_| true); // <path>s
```

`Figure::hand(side, shape)` gives the mesh of a hand as drawn, `grip(side)` the hollow of a
closed fist and `palm(side)` the middle of the palm (both in the hand's frame), so props can be
held by what the audience sees: a physics engine can use the hand mesh as a kinematic collider and
a bar can run through the fist.

Hair locks are modelled into the head and skinned to their spring chains with the swing of every
link softly limited, so fast moves bend the hair without tearing the mesh. Cloth takes a wind and
a `billow` (air flowing out from the body) so a robe can open like wings in a leap, and a `shape`
force that pulls it back towards its designed drape (anime CG keeps cloth on model), and rests on
a `floor` when there is one (a robe spreading on the ground under a push-up). Chains like ties
need their bending critically damped (joint damping about 2·√(stiffness / link length)): an
underdamped tie rings at several hertz, and sampled on twos that reads as the tie thrashing about.
Proportions, heads and hair scale with the skeleton (`toon::head_scale`), so a giant is its own
skeleton, not a fattened human.

A 1080p frame with six characters rasterizes and inks in tens of milliseconds on one core; the
example renders its 90 seconds in about a minute on an M4 Max with the Skia backend.

## Research: what Japanese studios use, and where fframes fits

The question this crate started from: can the animation be produced with the open source tools
real studios use, instead of generating video? The short answer is that the open source parts of
a Japanese pipeline are formats, data and algorithms, not a renderer you can point at a song, so
we implemented the animation in fframes on top of those.

### The TV anime pipeline

1. **絵コンテ storyboard** and **レイアウト layout**: paper or Clip Studio Paint.
2. **原画 key animation** and **動画 in-betweens**: still largely paper, digitally Clip Studio
   Paint EX, TVPaint, Toon Boom Harmony, and OpenToonz (the open-sourced Toonz that Studio Ghibli
   customised, released by Dwango in 2016).
3. **タイムシート timesheets**: how many frames each drawing is held (on ones, twos, threes),
   traditionally paper; digitally XDTS, the exchange timesheet format Clip Studio Paint writes and
   OpenToonz imports (`toonz/sources/toonz/xdtsio.cpp`).
4. **仕上げ ink and paint**: RETAS PaintMan historically, now Clip Studio Paint and OpenToonz.
5. **背景 backgrounds**: painted separately.
6. **撮影 compositing ("photography")**: Adobe After Effects almost everywhere. Camera moves,
   multiplane parallax, glows (透過光), gradient overlays (パラ), diffusion, shakes, focus lines.
7. **3DCG**: crowds, vehicles and more and more characters, cel shaded in 3ds Max, Maya or
   Blender, outlined with line renderers like Pencil+, then rendered on twos so it sits with the
   drawn animation.

Avatars and game characters use the VRM standard (VRM Consortium): MToon shading and SpringBone
secondary motion, implemented in open source by UniVRM, pixiv's three-vrm (MIT) and the VRM Add-on
for Blender (MIT). Motion data comes from studios' own mocap; Bandai Namco Research published
two sets of it (BVH, CC BY-NC 4.0).

### What was usable, and how this crate uses it

| piece | used as |
| --- | --- |
| Bandai Namco Research Motion Dataset (BVH, 175 clips: walks in 15 styles, runs, dashes, bows, waves, punches, kicks, slashes, dances) | every walk, wave, punch and kick in the opening; `bvh` + `rig::Retarget` move it onto each character's proportions |
| VRM SpringBone (the model behind hair and accessories on VRM avatars) | `spring`: chains that keep their momentum, return to their rest direction, fall, collide; plus joint damping so equal links do not resonate down the chain |
| Position based dynamics (the standard game cloth solver) | `spring::Cloth`: robes with tension-only shear and bend links so they fold like fabric |
| Rapier (dimforge, open source) | 2D rigid body piles and floods in the example (cream puffs, stickers, keycaps, 110 geese pushed by the hero) |
| Timesheet practice (koma) | `koma`: drawings held on twos, physics baked on the same held times, camera on ones |
| Cel shaded 3DCG with line renderers (Pencil+ in 3ds Max/Maya, Line Art in Blender) | `sdf` + `mesh` + `figure` + `raster` + `ink`: modelled, skinned characters, flat cel shading, vector contours and material lines, face details painted like texture decals |
| 撮影 conventions | the example's focus lines, speed lines, impact frames, smears, letterbox, scaled giant compositing; on the GPU (SkSL run by fframes' Skia backend, `examples/anime-op/src/gpu.rs`): diffusion glow and パラ over every frame, motion blur, rim light, chromatic split on impacts, focus lines, impact bursts, fighting auras, cel-shaded smoke, light rays, foil glints |
| Key animation practice (原画, 中割り, 表情集) | `koma::Sheet` times the dance from keys and timing charts; `acting::Acting` keys expressions, blinks and lip sync for every character in every shot |

Nothing was generated by a video model: every frame is drawn by fframes from these sources.

### What fframes needs to be a tool for a studio

The opening shows fframes can do the 撮影 and procedural parts. To fit an actual production:

1. **XDTS import**: read a timesheet from Clip Studio Paint or OpenToonz and play cel sequences
   (PNG/TGA/PSD layers) by their exposure, so the hand-drawn animation drives fframes instead of
   being re-timed by hand.
2. **Cel layer compositing presets**: the 撮影 vocabulary as components: パラ gradient overlays,
   透過光 glows, diffusion, T.U./T.B. (track up/back) and pans over oversized backgrounds,
   multiplane parallax, camera shake curves, focus and speed lines (both in the example already).
3. **VRM import**: load VRoid/VRM characters with their spring bone settings and render them
   through `raster` and `ink` (the renderer already takes any skinned triangle mesh and skins hair
   to spring chains the way VRM does), on twos, next to the drawn cels.
4. **Line control**: per-part line weights and tapering exist (`ink`), next are artist-painted
   line weight maps, crease lines from normals, and exporting the line art as editable vector
   layers for touch-ups in Clip Studio Paint.
5. **Mocap and keyframe layering**: blend mocap with hand-keyed poses per body part (the example
   does it by hand for the dance), plus MMD (VMD) motion import for the fan ecosystem.
6. **Deterministic baking for farms**: physics baked once per shot, shared by render workers
   (already how this crate works).

## Credits

- Bandai Namco Research Motion Dataset, CC BY-NC 4.0, https://github.com/BandaiNamcoResearchInc/Bandai-Namco-Research-Motiondataset
- The VRM SpringBone model, https://github.com/vrm-c/vrm-specification

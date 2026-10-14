# Anime opening

A 90 second anime opening in the style of MASHLE's second opening ("Bling-Bang-Bang-Born", Creepy
Nuts), cut on the reference's 93 cuts and starring the author of
[fff](https://github.com/dmtrKovalenko/fff), [fframes](https://github.com/dmtrKovalenko/fframes),
[odiff](https://github.com/dmtrKovalenko/odiff) and [zlob](https://github.com/dmtrKovalenko/zlob)
(@neogoose_btw) with @chshersh, @Shreyassanthu77, @dillon_mulroy, @theo and @teej_dv.

Every character is animated by [`fframes-anime`](../../fframes-anime): 3D skeletons driven by the
Bandai Namco Research motion capture dataset and hand-keyed moves (the BBBB dance, the barbell,
the fight), modelled and skinned 3D toon characters with spring-bone hair locks, ties and scarves
and cloth robes, cel shaded and inked with vector line art, on twos. Every character acts:
expression keys, blinks, eye darts and lip sync (the chorus sings the hook on the vocal's
half-beat grid). The dance is timed like key animation, keys snapping onto the beat with
overshoots and moving holds. The rivals strike JoJo poses (ジョジョ立ち) keyed on the same timesheets,
with ゴゴゴ, impact frames and name cards. The villain, SLOP, is a giant of AI-generated goo. 撮影
effects run as SkSL shaders on the GPU: diffusion glow, motion blur, rim light, impact bursts,
auras, smoke. Crowds and piles (cream puffs, stickers, keycaps, a flood of geese) are Rapier rigid
bodies, and held props are 3D: the cream puff and the barbell are modelled and inked like the
cast, the bar runs through the fists, and the puff falls into the hands, rests in a palm and rides
it with Rapier 3D, the hands colliding with their own drawn meshes. No frame comes from a video
model.

It is an experiment: the song and the reference are not in the repository and the mocap is
non-commercial. `tools/fetch.sh` downloads all three into `dynamic/`.

```bash
tools/fetch.sh                          # song, reference video, mocap
R="cargo run --release -p anime-op --"
$R preview                              # watch it
$R timeline                             # the 62 shots on the song
$R strip Fight -n 24                    # contact sheet of a shot
$R render                               # anime-op.mp4
bash tools/compare.sh anime-op.mp4 0 30 cmp.png   # reference over render, one pair per second
ANIME_OP_LAB=heads $R frame 1s          # the cast's heads from three angles
ANIME_OP_LAB=head-theo ANIME_OP_CHAINS=1 $R frame 1s   # one head, six angles, hair chains drawn
ANIME_OP_LAB=hands $R frame 0.5s        # the five hand shapes
ANIME_OP_LAB=sheet-demon $R frame 0.5s  # model sheet: front, 3/4, side, back with head heights
ANIME_OP_LAB=faces-theo $R frame 1.1s   # expression sheet: twelve faces, blinks, singing
ANIME_OP_LAB=turn-neogoose $R frame 0.5s   # one face turning from the front to the profile
ANIME_OP_LAB=jojo-latency $R frame 0.5s    # a rival in every JoJo pose, front and 3/4
ANIME_OP_LAB=props $R frame 0s          # the cream puff from six angles
tools/vs.sh cmp.png 27.8 74.1           # reference next to freshly rendered frames
```

| file | what |
| --- | --- |
| `src/cast.rs` | the cast: proportions, hair (spring chains), glasses, beards, robes, the rivals and the demon |
| `src/motion.rs` | mocap clips (looping walks keep travelling, `in_place` for dancing) |
| `src/procedural.rs` | hand-keyed poses with IK, the beat grid (157 BPM) and the Bling-Bang-Bang-Born dance |
| `src/actor.rs` | a cast member in a shot: motion, expression, exposure, baked physics, smears |
| `src/jojo.rs` | JoJo poses (lean back, point, crossed arms, hand on the chest) and their timesheet |
| `src/props.rs` | the cream puff (cracked choux cap, sugar, piped cream) and the barbell as 3D props |
| `src/physics3d.rs` | Rapier 3D: props dropped into hands or carried in a palm, hands colliding with their drawn meshes |
| `src/expr.rs` | the expression sheet as code (happy, laugh, angry, shocked, smug, strain...) and lip helpers (chew, talk, ha) |
| `src/gpu.rs`, `src/shaders/` | GPU effect layers in SkSL: focus lines, impact bursts, auras, toon smoke, rays, glints; rim light and chromatic split filters |
| `src/render.rs` | draws actors and props: their meshes share one depth buffer, the cel shaded pixels become an `<image>`, the line art `<path>`s on top; figures are meshed once per character |
| `src/rigid.rs` | Rapier rigid bodies baked per frame |
| `src/shots/` | the shots, by section (`title`, `banner`, `intro`, `friends`, `middle`, `middle2`, `villains`, `dance`, `finale`) |
| `tools/analyze.py` | beat and onset analysis of the song |

Credits: music Creepy Nuts "Bling-Bang-Bang-Born"; motion capture Bandai Namco Research Motion
Dataset (CC BY-NC 4.0); fonts UnifrakturCook, Pirata One, Dela Gothic One, Zen Kaku Gothic New (OFL).

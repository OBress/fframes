# THE WHY

A 98 second film for engineers who lost the work they were trained for.

The machines learned the *how*. They build anything, without error, without rest. What they do
not have is a reason. The film follows one engineer from a factory that no longer needs him to a
line he draws in the dust, and shows what happens when someone gives the machines a *why*.

> Hat man sein warum? des Lebens, so verträgt man sich fast mit jedem wie?
> *Friedrich Nietzsche, Twilight of the Idols, 1889*
>
> If you have your why, you can bear almost any how. *(our translation)*

1920x1080 for review, 3840x2160 for the master. 60 fps. Everything is rendered by fframes-3d on
the GPU; titles and the robot HUD are SVG in the same frame. Real places come from open data:
Manhattan from the NYC 3D Building Model (`fetch-nyc.sh`), the Golden Gate from USGS terrain,
DataSF buildings and OpenStreetMap (`fetch-sf.sh`).

## Look

- Light tells the story. Cold and blue where the machines work alone, one warm color for
  human intent: the glowing line, the village, the sunrise. Warm light grows from shot 6 on.
- Haze everywhere. Light shafts in the factory, ground haze in the desert, dust at sunrise.
- Lenses: 24 mm for the factory scale, 50 mm for the engineer, 85 to 135 mm for the line.
  Depth of field in every shot. Motion blur on fast moves.
- The engineer is never seen in detail. He is a silhouette, a shape against light. Every
  engineer can be him.
- Every transition is unique. No cut is a plain cut.

## Shots

| # | time | shot | transition into the next shot |
|---|---|---|---|
| 1 | 0.0 to 7.0 | **The cursor.** A macro of the subpixels of a blinking cursor. A question types itself; the machine answers with code, faster than anyone can read. The camera pulls back: the screen is one of thousands, all writing. | The screens go dark; the camera rushes into the last cursor, which becomes a welding arc. |
| 2 | 7.0 to 19.0 | **The factory of answers.** An endless hall. Thousands of robot arms in perfect rhythm, sparks, conveyors of glowing parts, light shafts through high windows. Slow dolly at floor level, 24 mm. | A robot turns its camera eye toward the catwalk; we cut into its view. |
| 3 | 19.0 to 29.0 | **The engineer.** Robot POV with a HUD: `HUMAN · ROLE: NONE`. He stands on a catwalk, an old laptop closed on the rail. The arms below never pause. | The robot places a chip; the camera dives into the chip. |
| 4 | 29.0 to 42.0 | **The perfect city.** The traces of the chip become the streets of Manhattan (the NYC 3D Building Model). Past the Empire State Building, through Times Square, where the screens play made-of-motion. Traffic lights change for nobody. No people. | The camera tilts up from the city lights into the stars. |
| 5 | 42.0 to 52.0 | **The walk.** The same stars; the camera tilts down to a desert plateau. The engineer walks away from the factory glow on the horizon. Wind. | He kneels; the camera orbits low around him, time slows. |
| 6 | 52.0 to 58.5 | **The line.** Close on the dust. He draws one line with his finger. It begins to glow, warm. The music stops; one note. | The light of the line runs across the desert into the factory. |
| 7 | 58.5 to 67.0 | **The wave.** Inside, the arms stop. One after another, thousands of them turn toward the glass wall and the light. | The light of the line becomes the first cable of a bridge. |
| 8 | 67.0 to 82.0 | **The why.** Machines build what people need: the Golden Gate Bridge in its real strait at night, and San Francisco and Sausalito light up (67 to 72); a greenhouse dome that turns green in the desert at first light (72 to 77); a rocket that leaves at dawn (77 to 82). Each part starts from the line. | The rocket flare fills the frame and becomes the sun. |
| 9 | 82.0 to 92.0 | **Sunrise.** The engineer stands on the plateau. The Golden Gate over the canyon, the greenhouse, the rocket trail in the light. Long shadows, dust in the air, birds. The quote. | Fade to black on the last word. |
| 10 | 92.0 to 98.0 | **Title.** "The how is automated." Beat. "The why is yours." A warm line draws under "why". Small: the title and made with fframes. | |

## Narration

A calm, warm voice, close to the microphone, unhurried. Five lines and the quote.

| at | line |
|---|---|
| 10.0 | We taught the machines how. |
| 21.5 | Now they do it better than we ever did. |
| 32.0 | They can build anything. |
| 37.0 | They just don't know what is worth building. |
| 61.0 | That part was never theirs to take. |
| 84.0 | If you have your why... you can bear almost any how. |

## Score

One piece, composed to the cut.

| time | section |
|---|---|
| 0 to 7 | a low drone and a ticking clock, sparse |
| 7 to 29 | mechanical pulse, metallic percussion, cold synths, rising scale |
| 29 to 42 | hollow piano, wide reverb, the pulse drops away |
| 42 to 52 | almost nothing: wind, one sustained string |
| 52 to 58.5 | silence, then one warm note |
| 58.5 to 67 | strings swell under a heartbeat |
| 67 to 82 | full build: strings, brass, drums, hope |
| 82 to 92 | the peak resolves into a sustained warm chord under the quote |
| 92 to 98 | a single piano line, end |

## Sound

Factory ambience, servo whine and welding crackle (2 to 29); city hum and distant cranes (29 to
42); desert wind and footsteps in dust (42 to 58); the hum of the glowing line (54); a wave of
servos turning (59); bridge cables tensing, a door of glass panels closing, a rocket launch (67
to 82); morning birds (85).

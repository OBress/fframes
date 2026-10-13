//! Shot 1: the cursor. A person types a question. The machine answers with code, faster and
//! faster, until nobody can read it. The code flies away, and the cursor grows into the light
//! of the welding arc that opens shot 2.

use fframes::{Svgr, svgr};
use fframes_3d::math::{hash, smoothstep};
use fframes_3d::{Vec2, vec2};

use crate::WIDTH;
use crate::look::{BAR, CODE, PICTURE_HEIGHT};

/// The question that the person types.
const QUESTION: &str = "// what should we build?";
/// The lines of the answers. The machine repeats them in an order without a pattern.
const ANSWERS: [&str; 40] = [
    "fn build(plan: &Plan) -> Result<Part, Error> {",
    "    let seams = plan.seams().filter(|s| s.open());",
    "    for seam in seams {",
    "        weld(seam, Torch::Laser { power: 4.2 })?;",
    "    }",
    "    inspect(&part, Tolerance::Microns(2))?;",
    "    Ok(part)",
    "}",
    "let route = optimize(graph, Strategy::Shortest);",
    "deploy(model, Region::All).await?;",
    "assert!(yield_rate > 0.9998);",
    "// 14 212 units per hour",
    "impl Factory for Line07 {",
    "    fn next(&mut self) -> Option<Unit> {",
    "        self.queue.pop_front().map(assemble)",
    "    }",
    "}",
    "let city = generate(grid, Density::Max);",
    "for block in city.blocks() { block.raise(); }",
    "schedule.cranes(|c| c.lift(beam).place(slot));",
    "// all tests passed (98 431)",
    "let answer = solve(question)?;",
    "traffic.sync(lights, Cycle::Seconds(42));",
    "cache.warm(&mut index, Policy::Eager);",
    "let chip = layout(netlist).route()?.verify()?;",
    "while demand.rises() { supply.scale(2.0); }",
    "commit(\"rewrite the world\")",
    "match request {",
    "    Ask::How(task) => automate(task),",
    "    Ask::Why(_) => unimplemented!(),",
    "}",
    "let parts = stock.reorder(Threshold::Zero);",
    "train(model, data, Epochs(1_000_000));",
    "fn decide(&self) -> Decision { self.cheapest() }",
    "pipeline.run().await.expect(\"no failure\");",
    "// generated in 0.003 s",
    "let output = input.map(transform).collect();",
    "vision.track(Human::any()).classify()?;",
    "throughput *= 1.07;",
    "#[derive(Optimal, Tireless, Exact)]",
];

const COLOR: &str = "#aebdd2";
const COMMENT: &str = "#62707f";
const CURSOR: &str = "#e8f0fb";

/// The number of answer lines written by time `t`: the speed doubles every 0.45 s.
fn written(t: f32) -> f32 {
    4.0 * (((t - 3.4).max(0.0) / 0.45).exp2() - 1.0)
}

const SIZE: f32 = 46.0;
const LINE: f32 = 60.0;

/// One terminal of the wall: the answers written by time `t`, as a terminal shows them. New
/// lines go below the last line; when the screen is full, the lines scroll up. `seed` picks
/// the lines. Returns the text elements and the position of the end of the last line.
fn terminal(
    seed: u32,
    count: f32,
    x: f32,
    top: f32,
    height: f32,
    first_y: f32,
) -> (Vec<Svgr<'static>>, Vec2) {
    let rows = ((top + height - 60.0 - first_y) / LINE).floor();
    let scroll = (count - rows).max(0.0) * LINE;
    let mut lines = Vec::new();
    let mut end = vec2(x, first_y - scroll);
    for i in 0..count.ceil() as u32 {
        let y = first_y + LINE * (i + 1) as f32 - scroll;
        let pick = hash(i * 7 + 1 + seed * 7919) * ANSWERS.len() as f32;
        let text = ANSWERS[pick as usize % ANSWERS.len()];
        end = vec2(x + text.len() as f32 * SIZE * 0.6, y);
        if y < top + 50.0 || y > top + height {
            continue;
        }
        let fill = if text.starts_with("//") {
            COMMENT
        } else {
            COLOR
        };
        lines.push(svgr!(
            <text x={x} y={y} font-family={CODE} font-size={SIZE} fill={fill}>{text}</text>
        ));
    }
    (lines, end)
}

/// The SVG of shot 1 at `t` seconds.
pub fn cursor(t: f32) -> Svgr<'static> {
    let top = BAR as f32;
    let (width, height) = (WIDTH as f32, PICTURE_HEIGHT as f32);
    let center = vec2(width * 0.5, top + height * 0.5);
    let x0 = 520.0;
    let question_y = top + height * 0.42;

    // the question, typed by a person after the cursor blinks
    let typed = (((t - 1.4) * 13.0).max(0.0) as usize).min(QUESTION.len());
    let question = &QUESTION[..typed];

    // The answers come faster and faster. The view zooms out to a wall of terminals that all
    // write at once. Then the wall fades and leaves the cursor.
    let count = written(t.min(5.9));
    let zoom = smoothstep(4.3, 5.7, t);
    let scale = 1.0 - 0.78 * zoom;
    let wall_fade = 1.0 - smoothstep(5.75, 6.15, t);
    let main_scroll = (count - ((top + height - 60.0 - question_y) / LINE).floor()).max(0.0) * LINE;
    let (main, end) = terminal(0, count, x0, top, height, question_y);
    let mut wall = Vec::new();
    for row in -2i32..=2 {
        for column in -3i32..=3 {
            if row == 0 && column == 0 {
                continue;
            }
            let seed = ((row + 2) * 7 + column + 3) as u32 + 1;
            // each terminal started at another moment
            let start = hash(seed * 13) * 2.0;
            let (lines, _) = terminal(
                seed,
                written(t.min(5.9) + start),
                x0,
                top,
                height,
                top + 120.0,
            );
            let dx = column as f32 * (width + 120.0);
            let dy = row as f32 * (height + 120.0);
            wall.push(svgr!(<g transform={format!("translate({dx} {dy})")}>{lines}</g>));
        }
    }
    let view = format!(
        "translate({} {}) scale({scale}) translate({} {})",
        center.x, center.y, -center.x, -center.y
    );
    // the flood blurs vertically when it is too fast to read
    let blur = (count / 60.0).clamp(0.0, 1.0).powi(2) * 10.0;

    // the cursor: at the end of the question, then at the end of the newest line, then it
    // glides to the center and grows into light
    let writer = if count < 1.0 {
        vec2(x0 + typed as f32 * SIZE * 0.6, question_y)
    } else {
        end
    };
    let on_screen = center + (writer + vec2(SIZE * 0.3, -SIZE * 0.35) - center) * scale;
    let glide = smoothstep(5.8, 6.4, t);
    let cursor = on_screen.lerp(center, glide);
    let grow = smoothstep(6.3, 6.95, t);
    let size = (
        SIZE * 0.6 * scale.max(0.5 * (1.0 - glide) + glide) + 260.0 * grow * grow,
        SIZE * 0.95 * scale.max(0.5 * (1.0 - glide) + glide) + 160.0 * grow * grow,
    );
    let blinking = t < 1.4 || (3.0..3.4).contains(&t);
    let on = !blinking || ((t * 2.0) as u32).is_multiple_of(2);
    let glow = 0.25 + 0.75 * grow;
    let flash = smoothstep(6.75, 7.0, t);
    let (rx, ry) = (cursor.x - size.0 * 0.5, cursor.y - size.1 * 0.5);
    let question_y = question_y - main_scroll;

    svgr!(
        <g>
            <defs>
                <clipPath id="picture">
                    <rect x="0" y={BAR} width={WIDTH} height={PICTURE_HEIGHT} />
                </clipPath>
                <filter id="flood-blur" x="-10%" y="-50%" width="120%" height="200%">
                    <feGaussianBlur stdDeviation={format!("0 {blur:.2}")} />
                </filter>
                <filter id="cursor-glow" x="-200%" y="-200%" width="500%" height="500%">
                    <feGaussianBlur stdDeviation={18.0 + 60.0 * grow} />
                </filter>
            </defs>
            <rect x="0" y={BAR} width={WIDTH} height={PICTURE_HEIGHT} fill="#000" />
            <g clip-path="url(#picture)">
                <g transform={view} filter="url(#flood-blur)" opacity={wall_fade}>
                    <text x={x0} y={question_y} font-family={CODE} font-size={SIZE} fill={COMMENT}>{question}</text>
                    {main}
                    <g opacity={zoom}>{wall}</g>
                </g>
                <g opacity={if on { 1.0 } else { 0.0 }}>
                    <rect x={rx} y={ry} width={size.0} height={size.1} fill="#9fc8ff" opacity={glow} filter="url(#cursor-glow)" />
                    <rect x={rx} y={ry} width={size.0} height={size.1} fill={CURSOR} />
                </g>
            </g>
            <rect x="0" y={BAR} width={WIDTH} height={PICTURE_HEIGHT} fill="#f4f8ff" opacity={flash} />
        </g>
    )
}

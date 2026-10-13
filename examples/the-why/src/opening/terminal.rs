//! The terminal of shot 1: the layout of the text on a screen, the lines of text as flat
//! meshes, the pixel grid of the screen and the page of code on the screens far away.

use std::sync::Arc;

use fframes_3d::math::hash;
use fframes_3d::*;

/// The monospaced font of the terminal.
const FONT: &[u8] = include_bytes!("../../media/JetBrainsMono-Regular.ttf");

/// The question that the person types.
pub const QUESTION: &str = "// what should we build?";
/// The lines of the answers. The machine writes the first eight in order, then the others
/// in an order without a pattern.
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

/// The em size of the font in meters. The screens are large: 1.16 m wide.
pub const EM: f32 = 0.025;
/// The advance of one character in meters. All characters of the font are 0.6 em wide.
pub const ADVANCE: f32 = 0.6 * EM;
/// The distance between two lines in meters.
pub const LINE: f32 = 1.45 * EM;
/// The characters of a line of the screen.
pub const COLUMNS: usize = 72;
/// The lines of the screen.
pub const ROWS: usize = 16;
/// The space between the text and the edge of the screen in meters.
const MARGIN: f32 = 1.6 * EM;
/// The size of the screen in meters.
pub const SCREEN: Vec2 = vec2(
    COLUMNS as f32 * ADVANCE + 2.0 * MARGIN,
    ROWS as f32 * LINE + 2.0 * MARGIN,
);
/// The height of the baseline above the bottom of its line.
const BASELINE: f32 = 0.36 * EM;
/// The size of one pixel of the screen in meters: 24 pixels for each em.
const PIXEL: f32 = EM / 24.0;
/// The mean of the pixel grid texture in each color channel (linear). The colors of the
/// text divide by it, so the text has the same light near and far.
pub const GRID_MEAN: f32 = (1.0 / 3.0) * (5.0 / 6.0) * (16.0 / 18.0);
/// The lines of the page of the screens far away. The page repeats.
pub const PAGE_LINES: usize = 64;

/// The left end of the baseline of `row`, on the screen. Row 0 is the top row. A row can
/// have a fraction when the text scrolls.
pub fn baseline(row: f32) -> Vec2 {
    vec2(
        -SCREEN.x * 0.5 + MARGIN,
        SCREEN.y * 0.5 - MARGIN - (row + 1.0) * LINE + BASELINE,
    )
}

/// The center of the text area on the screen, and its size.
pub fn text_area() -> (Vec2, Vec2) {
    let size = vec2(COLUMNS as f32 * ADVANCE, ROWS as f32 * LINE);
    (vec2(0.0, SCREEN.y * 0.5 - MARGIN - size.y * 0.5), size)
}

/// The answer on line `i` of the page.
pub fn page_line(i: usize) -> usize {
    let i = i % PAGE_LINES;
    if i < 8 {
        i
    } else {
        (hash(i as u32 * 7 + 1) * ANSWERS.len() as f32) as usize % ANSWERS.len()
    }
}

/// The text of answer `i`.
pub fn answer(i: usize) -> &'static str {
    ANSWERS[i]
}

/// The meshes and the textures of the terminal, made once.
pub struct Terminal {
    /// The question up to each of its characters: `question[k]` has `k` characters.
    pub question: Vec<Arc<Geometry>>,
    /// The answers, in the order of [`ANSWERS`].
    pub answers: Vec<Arc<Geometry>>,
    /// The pixel grid of the screen. UVs count the pixels.
    pub grid: Arc<Texture>,
    /// The page of the screens far away: [`PAGE_LINES`] lines, white on black.
    pub page: Arc<Texture>,
}

impl Terminal {
    pub fn new() -> Self {
        let question = (0..=QUESTION.len())
            .map(|k| Arc::new(flat_text(&QUESTION[..k])))
            .collect();
        let answers: Vec<Arc<Geometry>> = ANSWERS
            .iter()
            .map(|line| Arc::new(flat_text(line)))
            .collect();
        let page = Arc::new(page(&answers));
        Self {
            question,
            answers,
            grid: Arc::new(pixel_grid()),
            page,
        }
    }
}

/// `text` as a flat mesh in the XY plane that faces +Z, from the left end of its baseline.
/// The UVs count the pixels of the screen, for the pixel grid.
fn flat_text(text: &str) -> Geometry {
    let solid = Geometry::text(FONT, text, EM, EM * 0.01, 0.0)
        .expect("the font has all characters of the terminal");
    // Only the front faces are visible on the screen.
    let indices = solid
        .indices
        .as_chunks::<3>()
        .0
        .iter()
        .filter(|tri| tri.iter().all(|&i| solid.normals[i as usize].z > 0.5))
        .flatten()
        .copied()
        .collect();
    let uvs = solid
        .positions
        .iter()
        .map(|p| vec2(p.x, -p.y) / PIXEL)
        .collect();
    Geometry::new(solid.positions, solid.normals, uvs, indices)
}

/// A rectangle of `size` meters in the XY plane that faces +Z, with UVs that count the
/// pixels of the screen.
pub fn pixel_plane(size: Vec2) -> Geometry {
    let mut plane = Geometry::plane(size.x, size.y);
    plane.uvs = plane
        .positions
        .iter()
        .map(|p| vec2(p.x, -p.y) / PIXEL)
        .collect();
    plane
}

/// One pixel of the screen, 18 x 18 texels: a red, a green and a blue stripe with dark
/// gaps between them. Its mean is [`GRID_MEAN`].
fn pixel_grid() -> Texture {
    Texture::from_fn_data(18, 18, true, |u, v| {
        let (x, y) = ((u * 18.0) as usize, (v * 18.0) as usize);
        let mut texel = [0, 0, 0, 255];
        if x % 6 != 0 && y != 0 && y != 17 {
            texel[x / 6] = 255;
        }
        texel
    })
}

/// The page of the screens far away: [`PAGE_LINES`] lines of answers as the terminal
/// shows them, white on black. Comments are darker. The renderer draws it once.
fn page(answers: &[Arc<Geometry>]) -> Texture {
    let size = vec2(COLUMNS as f32 * ADVANCE, PAGE_LINES as f32 * LINE);
    let mut scene = Scene::new().background(Background::Color(Rgb::BLACK));
    for i in 0..PAGE_LINES {
        let line = page_line(i);
        let light = if ANSWERS[line].starts_with("//") {
            0.45
        } else {
            1.0
        };
        scene.add(
            Node::mesh(answers[line].clone(), Material::unlit(Rgb::WHITE * light)).at(
                0.0,
                -(i as f32 + 1.0) * LINE + BASELINE,
                0.0,
            ),
        );
    }
    let center = vec3(size.x * 0.5, -size.y * 0.5, 0.0);
    let camera = Camera::orthographic(size.y)
        .position(center + Vec3::Z)
        .look_at(center);
    let width = 2048;
    let height = (width as f32 * size.y / size.x).round() as u32;
    let pixels = Renderer::new(width, height)
        .tone_mapping(ToneMapping::Linear)
        .render_pixels(&scene, &camera);
    Texture::from_rgba8(
        pixels.width,
        pixels.height,
        &pixels.to_straight_rgba(),
        true,
    )
}

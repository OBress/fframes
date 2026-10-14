//! GPU effect layers for the action (撮影 effects): SkSL shaders run by the Skia backend while
//! it draws the frame (focus lines, impact bursts, fighting auras, toon smoke, light rays) and
//! SVG filter passes (chromatic split). The tiny-skia CPU backend skips shader layers.

use std::sync::OnceLock;

use fframes::{Color, Frame, Shader, ShaderUniforms, Svgr};
use fframes_anime::glam::Vec2;

const COMMON: &str = include_str!("shaders/common.sksl");
pub const SPEEDLINES: &str = include_str!("shaders/speedlines.sksl");
pub const BURST: &str = include_str!("shaders/burst.sksl");
pub const AURA: &str = include_str!("shaders/aura.sksl");
pub const SMOKE: &str = include_str!("shaders/smoke.sksl");
pub const RAYS: &str = include_str!("shaders/rays.sksl");
pub const SHINE: &str = include_str!("shaders/shine.sksl");

/// A shader with the shared noise functions in front of it.
pub fn compile(src: &str) -> Shader {
    Shader::sksl(format!("{COMMON}\n{src}"))
}

fn cached(cell: &'static OnceLock<Shader>, src: &str) -> &'static Shader {
    cell.get_or_init(|| compile(src))
}

static SPEEDLINES_SHADER: OnceLock<Shader> = OnceLock::new();
static BURST_SHADER: OnceLock<Shader> = OnceLock::new();
static AURA_SHADER: OnceLock<Shader> = OnceLock::new();
static SMOKE_SHADER: OnceLock<Shader> = OnceLock::new();
static RAYS_SHADER: OnceLock<Shader> = OnceLock::new();
static SHINE_SHADER: OnceLock<Shader> = OnceLock::new();

fn layer<'a>(
    image: &fframes::media::ImageData<'_>,
    (x, y, w, h): (f32, f32, f32, f32),
    blend: &'static str,
) -> Svgr<'a> {
    let href = image.href();
    fframes::svgr!(
        <g style={blend}>
            <image href={href} x={x} y={y} width={w} height={h} />
        </g>
    )
}

/// The drawing number on twos (of 24 fps): effects are redrawn with the characters, not every
/// frame, at any output rate.
fn drawing(frame: &Frame) -> f32 {
    (frame.seconds() * 12.0 + 1e-3).floor()
}

/// Focus lines (集中線) rushing at `center`, clear inside `clear` pixels.
pub fn focus_lines<'a>(
    frame: &Frame,
    center: Vec2,
    clear: f32,
    color: Color,
    density: f32,
) -> Svgr<'a> {
    let image = cached(&SPEEDLINES_SHADER, SPEEDLINES).draw(
        frame,
        ShaderUniforms::new()
            .float2("uCenter", center.x, center.y)
            .float("uClear", clear)
            .color("uColor", color)
            .float("uDensity", density)
            .float("uSeed", drawing(frame)),
    );
    layer(&image, (0.0, 0.0, 1920.0, 1080.0), "mix-blend-mode:normal")
}

/// An impact at `center`: a spiky flash collapsing and a shock ring racing out as `t` goes
/// 0..1.
pub fn burst<'a>(
    frame: &Frame,
    center: Vec2,
    radius: f32,
    t: f32,
    flash: Color,
    ring: Color,
    seed: f32,
) -> Svgr<'a> {
    if !(0.0..1.0).contains(&t) {
        return Svgr::empty();
    }
    let half = radius * 2.2;
    let image = cached(&BURST_SHADER, BURST).draw(
        frame,
        ShaderUniforms::new()
            .float2("uCenter", half, half)
            .float("uRadius", radius)
            .float("uT", t)
            .color("uFlash", flash)
            .color("uRing", ring)
            .float("uSeed", seed),
    );
    layer(
        &image,
        (center.x - half, center.y - half, half * 2.0, half * 2.0),
        "mix-blend-mode:normal",
    )
}

/// A fighting aura: cel-shaded flames licking up out of a body standing in the box
/// `center ± size / 2`.
pub fn aura<'a>(
    frame: &Frame,
    center: Vec2,
    size: Vec2,
    inner: Color,
    outer: Color,
    power: f32,
    seed: f32,
) -> Svgr<'a> {
    aura_towards(frame, center, size, inner, outer, power, seed, 0.0)
}

/// [`aura`] with its flames streaming `angle` degrees away from straight up (90 = to the
/// right, behind a body flying left).
#[allow(clippy::too_many_arguments)]
pub fn aura_towards<'a>(
    frame: &Frame,
    center: Vec2,
    size: Vec2,
    inner: Color,
    outer: Color,
    power: f32,
    seed: f32,
    angle: f32,
) -> Svgr<'a> {
    let image = cached(&AURA_SHADER, AURA).draw(
        frame,
        ShaderUniforms::new()
            .color("uInner", inner)
            .color("uOuter", outer)
            .float("uPower", power)
            .float("uSeed", seed),
    );
    let placed = layer(
        &image,
        (
            center.x - size.x * 0.5,
            center.y - size.y * 0.5,
            size.x,
            size.y,
        ),
        "mix-blend-mode:normal",
    );
    if angle == 0.0 {
        return placed;
    }
    let turn = format!("rotate({angle:.1} {:.1} {:.1})", center.x, center.y);
    fframes::svgr!(<g transform={turn}>{placed}</g>)
}

/// Colours of a toon smoke cloud.
#[derive(Debug, Clone, Copy)]
pub struct SmokeColors {
    pub light: Color,
    pub mid: Color,
    pub dark: Color,
    pub line: Color,
}

/// A toon explosion cloud of `radius` pixels around `center`, billowing to full size as `grow`
/// goes to 1.
pub fn smoke<'a>(
    frame: &Frame,
    center: Vec2,
    radius: f32,
    grow: f32,
    colors: SmokeColors,
    seed: f32,
) -> Svgr<'a> {
    let image = cached(&SMOKE_SHADER, SMOKE).draw(
        frame,
        ShaderUniforms::new()
            .float("uGrow", grow)
            .color("uLight", colors.light)
            .color("uMid", colors.mid)
            .color("uDark", colors.dark)
            .color("uLine", colors.line)
            .float("uSeed", seed),
    );
    layer(
        &image,
        (
            center.x - radius * 1.6,
            center.y - radius,
            radius * 3.2,
            radius * 2.0,
        ),
        "mix-blend-mode:normal",
    )
}

/// Light rays fanning out of `center` over the whole frame, added onto what is under them.
pub fn rays<'a>(frame: &Frame, center: Vec2, color: Color, count: f32) -> Svgr<'a> {
    let image = cached(&RAYS_SHADER, RAYS).draw(
        frame,
        ShaderUniforms::new()
            .float2("uCenter", center.x, center.y)
            .color("uColor", color)
            .float("uCount", count),
    );
    layer(&image, (0.0, 0.0, 1920.0, 1080.0), "mix-blend-mode:screen")
}

/// A glint sweeping across the box `(x, y, w, h)` as `pos` goes 0..1, screened onto it.
pub fn shine<'a>(frame: &Frame, rect: (f32, f32, f32, f32), pos: f32, color: Color) -> Svgr<'a> {
    if !(0.0..=1.0).contains(&pos) {
        return Svgr::empty();
    }
    let image = cached(&SHINE_SHADER, SHINE).draw(
        frame,
        ShaderUniforms::new()
            .float("uPos", pos)
            .color("uColor", color),
    );
    layer(&image, rect, "mix-blend-mode:screen")
}

static SPLITS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Chromatic aberration for impacts: the red and blue channels pulled `amount` pixels apart
/// along `dir`.
pub fn rgb_split<'a>(content: Svgr<'a>, amount: f32, dir: Vec2) -> Svgr<'a> {
    if amount < 0.5 {
        return content;
    }
    let id = format!(
        "rgbsplit{}",
        SPLITS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let url = format!("url(#{id})");
    let d = dir.normalize_or(Vec2::X) * amount;
    let (rx, ry, bx, by) = (d.x, d.y, -d.x, -d.y);
    fframes::svgr!(
        <g>
            <defs>
                <filter id={id} x="0" y="0" width="1920" height="1080" filterUnits="userSpaceOnUse" color-interpolation-filters="sRGB">
                    <feColorMatrix in="SourceGraphic" type="matrix" values="1 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 1 0" result="r" />
                    <feOffset in="r" dx={rx} dy={ry} result="ro" />
                    <feColorMatrix in="SourceGraphic" type="matrix" values="0 0 0 0 0  0 1 0 0 0  0 0 0 0 0  0 0 0 1 0" result="g" />
                    <feColorMatrix in="SourceGraphic" type="matrix" values="0 0 0 0 0  0 0 0 0 0  0 0 1 0 0  0 0 0 1 0" result="b" />
                    <feOffset in="b" dx={bx} dy={by} result="bo" />
                    <feBlend in="ro" in2="g" mode="screen" result="rg" />
                    <feBlend in="rg" in2="bo" mode="screen" />
                </filter>
            </defs>
            <g filter={url}>{content}</g>
        </g>
    )
}

static RIMS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Rim light (リムライト): a thin band of `color` along the edges of `content` that face the
/// light coming from `toward` (screen direction, e.g. up-left), the way compositors light a
/// cel against a bright background.
pub fn rim_light<'a>(content: Svgr<'a>, color: &'static str, toward: Vec2, width: f32) -> Svgr<'a> {
    let id = format!(
        "rim{}",
        RIMS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let url = format!("url(#{id})");
    let d = -toward.normalize_or(Vec2::new(-0.6, -0.8)) * width;
    let (dx, dy) = (d.x, d.y);
    fframes::svgr!(
        <g>
            <defs>
                <filter id={id} x="0" y="0" width="1920" height="1080" filterUnits="userSpaceOnUse" color-interpolation-filters="sRGB">
                    <feOffset in="SourceAlpha" dx={dx} dy={dy} result="shifted" />
                    <feComposite in="SourceAlpha" in2="shifted" operator="out" result="edge" />
                    <feGaussianBlur in="edge" stdDeviation="0.8" result="soft" />
                    <feFlood flood-color={color} result="tint" />
                    <feComposite in="tint" in2="soft" operator="in" result="rim" />
                    <feMerge>
                        <feMergeNode in="SourceGraphic" />
                        <feMergeNode in="rim" />
                    </feMerge>
                </filter>
            </defs>
            <g filter={url}>{content}</g>
        </g>
    )
}

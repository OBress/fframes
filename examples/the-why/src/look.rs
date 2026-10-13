//! The look of the film: the 2.39:1 picture between black bars, lenses, film grain and the
//! vignette. Every shot uses these, so the cuts match.

use fframes::media::ImageData;
use fframes::{Frame, Svgr, svgr};
use fframes_3d::math::{hash, smoothstep};
use fframes_3d::{Camera, Pixels, Renderer, Scene};

use crate::{FPS, HEIGHT, WIDTH};

/// The height of the picture between the bars, in frame pixels: 3840 / 2.39, made even.
pub const PICTURE_HEIGHT: usize = 1608;
/// The height of each black bar.
pub const BAR: usize = (HEIGHT - PICTURE_HEIGHT) / 2;
/// The aspect ratio of the picture.
pub const ASPECT: f32 = WIDTH as f32 / PICTURE_HEIGHT as f32;

pub const SERIF: &str = "Instrument Serif";
pub const SANS: &str = "Inter 24pt";
pub const CODE: &str = "JetBrains Mono";
pub const HUD: &str = "IBM Plex Mono";

/// A camera with the field of view of a lens of `mm` millimeters on a full frame sensor
/// (36 mm wide), cropped to the picture.
pub fn lens(mm: f32) -> Camera {
    let half_width = 18.0 / mm;
    Camera::perspective((half_width / ASPECT).atan().to_degrees() * 2.0)
}

/// The time that the shutter is open in seconds: half a frame, a 180 degree shutter.
pub const SHUTTER: f32 = 0.5 / FPS as f32;

/// The 3D picture at `t` seconds. `moment(t)` poses the scene and the camera. With `samples`
/// above 1, the picture is the average of that many moments in the shutter time: motion blur.
pub fn picture(
    renderer: &Renderer,
    samples: usize,
    t: f32,
    moment: impl Fn(f32) -> (Scene, Camera),
) -> ImageData<'static> {
    if samples > 1 {
        renderer.render_motion_blur(samples, |offset| moment(t + offset * SHUTTER))
    } else {
        let (scene, camera) = moment(t);
        renderer.render(&scene, &camera)
    }
}

/// [`picture`] as pixels, for tests.
pub fn picture_pixels(
    renderer: &Renderer,
    samples: usize,
    t: f32,
    moment: impl Fn(f32) -> (Scene, Camera),
) -> Pixels {
    if samples > 1 {
        renderer.render_motion_blur_pixels(samples, |offset| moment(t + offset * SHUTTER))
    } else {
        let (scene, camera) = moment(t);
        renderer.render_pixels(&scene, &camera)
    }
}

/// The size of the 3D images for the output `scale` of the command line.
pub fn picture_size(scale: f64) -> (u32, u32) {
    let size = |pixels: usize| ((pixels as f64 * scale).round() as u32).max(2) & !1;
    (size(WIDTH), size(PICTURE_HEIGHT))
}

/// A white flash that fades out over the first `length` seconds of a shot. It carries the
/// light of the cut in shot 1 into the welding arc of shot 2.
pub fn flash_in(t: f32, length: f32) -> Svgr<'static> {
    let opacity = 1.0 - smoothstep(0.0, length, t);
    if opacity <= 0.0 {
        return Svgr::empty();
    }
    svgr!(<rect x="0" y={BAR} width={WIDTH} height={PICTURE_HEIGHT} fill="#fff" opacity={opacity} />)
}

/// The iris of a camera that opens over the first `length` seconds of a shot.
pub fn iris(t: f32, length: f32) -> Svgr<'static> {
    let open = smoothstep(0.0, length, t);
    if open >= 1.0 {
        return Svgr::empty();
    }
    let (cx, cy) = (WIDTH as f32 * 0.5, (BAR + PICTURE_HEIGHT / 2) as f32);
    let r = 2200.0 * open * open;
    let d = format!(
        "M0 {top} H{w} V{bottom} H0 Z M{x} {cy} a{r} {r} 0 1 0 {d} 0 a{r} {r} 0 1 0 {nd} 0 Z",
        top = BAR,
        w = WIDTH,
        bottom = BAR + PICTURE_HEIGHT,
        x = cx - r,
        d = 2.0 * r,
        nd = -2.0 * r,
    );
    svgr!(<path d={d} fill="#000" fill-rule="evenodd" />)
}

/// Film grain, drawn over every frame.
pub struct Overlay {
    grain: Vec<fframes::media::ImageData<'static>>,
}

impl std::fmt::Debug for Overlay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Overlay").finish_non_exhaustive()
    }
}

/// The grain images. Each frame picks one at random, so the grain does not repeat.
const GRAIN_TILES: u32 = 12;
/// The size of a grain image. It stretches over the picture, so a grain is 4 frame pixels.
const GRAIN_SIZE: (u32, u32) = (960, 402);

impl Overlay {
    pub fn new() -> Self {
        let grain = (0..GRAIN_TILES)
            .map(|tile| {
                let (w, h) = GRAIN_SIZE;
                let rgba = (0..w * h)
                    .flat_map(|i| {
                        // the sum of three uniform values is close to a normal distribution
                        let seed = (tile * w * h + i) * 3;
                        let n = (hash(seed) + hash(seed + 1) + hash(seed + 2)) / 3.0;
                        let v = (n * 255.0).round() as u8;
                        [v, v, v, 255]
                    })
                    .collect();
                Pixels {
                    width: w,
                    height: h,
                    rgba,
                }
                .into_image()
            })
            .collect();
        Self { grain }
    }

    /// The grain for `frame`. `grain` is the strength, about 0.1.
    pub fn render(&self, frame: &Frame, grain: f32) -> Svgr<'_> {
        let tile = &self.grain
            [(hash(frame.index as u32 * 7 + 3) * GRAIN_TILES as f32) as usize % self.grain.len()];
        svgr!(
            <image href={tile.href()} x="0" y={BAR} width={WIDTH} height={PICTURE_HEIGHT}
                opacity={grain} style="mix-blend-mode:overlay" preserveAspectRatio="none" />
        )
    }
}

/// The vignette of the lens over the picture. `strength` goes from 0 (none) to 1.
pub fn vignette(strength: f32) -> Svgr<'static> {
    if strength <= 0.0 {
        return Svgr::empty();
    }
    svgr!(
        <g opacity={strength.min(1.0)}>
            <defs>
                <radialGradient id="vignette" cx="50%" cy="50%" r="75%">
                    <stop offset="0.55" stop-color="#000" stop-opacity="0" />
                    <stop offset="1" stop-color="#000" stop-opacity="0.55" />
                </radialGradient>
            </defs>
            <rect x="0" y={BAR} width={WIDTH} height={PICTURE_HEIGHT} fill="url(#vignette)" />
        </g>
    )
}

impl Default for Overlay {
    fn default() -> Self {
        Self::new()
    }
}

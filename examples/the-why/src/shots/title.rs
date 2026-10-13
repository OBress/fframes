//! The words of the film: the quote over the sunrise (shot 9) and the title (shot 10).

use fframes::{Svgr, svgr};
use fframes_3d::math::smoothstep;

use crate::WIDTH;
use crate::look::{BAR, PICTURE_HEIGHT, SANS, SERIF};

/// The one warm color of the film: human intent.
const WARM: &str = "#ffb35c";
const WHITE: &str = "#f4f1ec";

/// A fade in from `start` over `length` seconds.
fn fade_in(t: f32, start: f32, length: f32) -> f32 {
    smoothstep(start, start + length, t)
}

/// The quote over the sunrise at `t` seconds into shot 9, in time with the voice, and the
/// fade to black on the last word.
pub fn quote(t: f32) -> Svgr<'static> {
    let center = WIDTH as f32 * 0.5;
    let y = (BAR + PICTURE_HEIGHT) as f32 - 330.0;
    let out = 1.0 - smoothstep(8.6, 9.3, t);
    let first = fade_in(t, 2.0, 0.9) * out;
    let second = fade_in(t, 4.4, 0.9) * out;
    let name = fade_in(t, 6.6, 1.0) * out;
    let black = smoothstep(9.0, 10.0, t);
    svgr!(
        <g>
            <text x={center} y={y} font-family={SERIF} font-style="italic" font-size="104" fill={WHITE}
                text-anchor="middle" opacity={first}>
                "If you have your "<tspan fill={WARM}>"why"</tspan>","
            </text>
            <text x={center} y={y + 124.0} font-family={SERIF} font-style="italic" font-size="104" fill={WHITE}
                text-anchor="middle" opacity={second}>
                "you can bear almost any how."
            </text>
            <text x={center} y={y + 220.0} font-family={SANS} font-size="34" letter-spacing="8" fill={WHITE}
                text-anchor="middle" opacity={name * 0.7}>
                "FRIEDRICH NIETZSCHE"
            </text>
            <rect x="0" y={BAR} width={WIDTH} height={PICTURE_HEIGHT} fill="#000" opacity={black} />
        </g>
    )
}

/// The title at `t` seconds into shot 10.
pub fn title(t: f32) -> Svgr<'static> {
    let center = WIDTH as f32 * 0.5;
    let middle = (BAR + PICTURE_HEIGHT / 2) as f32;
    // the first line comes, then steps back for the second
    let how = fade_in(t, 0.5, 1.0) * (1.0 - 0.6 * fade_in(t, 2.6, 0.8));
    let why = fade_in(t, 2.9, 1.2);
    let signature = fade_in(t, 4.6, 0.8);
    let rise = 30.0 * (1.0 - fade_in(t, 2.9, 1.6));
    svgr!(
        <g>
            <rect x="0" y={BAR} width={WIDTH} height={PICTURE_HEIGHT} fill="#000" />
            <text x={center} y={middle - 70.0} font-family={SANS} font-size="112" fill={WHITE}
                text-anchor="middle" opacity={how}>
                "The "<tspan font-family={SERIF} font-style="italic">"how"</tspan>" is automated."
            </text>
            <text x={center} y={middle + 110.0 + rise} font-family={SANS} font-size="112" fill={WHITE}
                text-anchor="middle" opacity={why}>
                "The "<tspan font-family={SERIF} font-style="italic" fill={WARM}>"why"</tspan>" is yours."
            </text>
            <text x={center} y={(BAR + PICTURE_HEIGHT) as f32 - 120.0} font-family={SANS} font-size="30"
                letter-spacing="6" fill="#8a8f98" text-anchor="middle" opacity={signature}>
                "MADE WITH FFRAMES"
            </text>
        </g>
    )
}

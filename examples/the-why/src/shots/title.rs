//! The words of the film: the quote over the sunrise (shot 9) and the title (shot 10).

use fframes::{Svgr, svgr};
use fframes_3d::math::smoothstep;

use crate::WIDTH;
use crate::look::{BAR, PICTURE_HEIGHT, SERIF};

/// The one warm color of the film: human intent.
const WARM: &str = "#ffb35c";
const WHITE: &str = "#f4f1ec";

/// A fade in from `start` over `length` seconds.
fn fade_in(t: f32, start: f32, length: f32) -> f32 {
    smoothstep(start, start + length, t)
}

/// The quote over the sunrise at `t` seconds into shot 9, in time with the voice, and the
/// fade to black on the last word. It is left of the engineer, on the sand. A soft dark
/// glow behind it keeps it readable.
pub fn quote(t: f32) -> Svgr<'static> {
    let x = 330.0;
    let y = (BAR + PICTURE_HEIGHT) as f32 - 300.0;
    let out = 1.0 - smoothstep(8.6, 9.3, t);
    let first = fade_in(t, 2.0, 0.9) * out;
    let second = fade_in(t, 4.4, 0.9) * out;
    let name = fade_in(t, 6.6, 1.0) * out;
    let black = smoothstep(9.0, 10.0, t);
    svgr!(
        <g>
            <defs>
                <radialGradient id="quote-scrim" cx="50%" cy="50%" r="50%">
                    <stop offset="0" stop-color="#000" stop-opacity="0.45" />
                    <stop offset="1" stop-color="#000" stop-opacity="0" />
                </radialGradient>
            </defs>
            <ellipse cx={x + 650.0} cy={y + 40.0} rx="1100" ry="300" fill="url(#quote-scrim)" opacity={first} />
            <text x={x} y={y} font-family={SERIF} font-style="italic" font-size="104" fill={WHITE}
                opacity={first}>
                "If you have your "<tspan fill={WARM}>"why"</tspan>","
            </text>
            <text x={x} y={y + 124.0} font-family={SERIF} font-style="italic" font-size="104" fill={WHITE}
                opacity={second}>
                "you can bear almost any how."
            </text>
            <text x={x + 4.0} y={y + 210.0} font-family={SERIF} font-size="38" letter-spacing="8" fill={WHITE}
                opacity={name * 0.75}>
                "FRIEDRICH NIETZSCHE"
            </text>
            <rect x="0" y={BAR} width={WIDTH} height={PICTURE_HEIGHT} fill="#000" opacity={black} />
        </g>
    )
}

/// The width of `text` in pixels at `size` in the font file `font`, without kerning.
fn text_width(font: &[u8], text: &str, size: f32) -> f32 {
    let Ok(face) = fframes::ttf_parser::Face::parse(font, 0) else {
        return 0.0;
    };
    let advance: u16 = text
        .chars()
        .filter_map(|ch| face.glyph_hor_advance(face.glyph_index(ch)?))
        .sum();
    f32::from(advance) * size / f32::from(face.units_per_em())
}

const ROMAN_FONT: &[u8] = include_bytes!("../../media/InstrumentSerif-Regular.ttf");
const ITALIC_FONT: &[u8] = include_bytes!("../../media/InstrumentSerif-Italic.ttf");
const TITLE_SIZE: f32 = 152.0;
/// The italic word is a little larger, so its x-height matches the roman words.
const ITALIC: f32 = 1.06;

/// One line of the title: `before`, the word `word` in the italic serif, and `after`,
/// centered on the frame. Returns the line and the left and right ends of the word.
fn title_line(
    before: &'static str,
    word: &'static str,
    after: &'static str,
    y: f32,
    word_fill: &'static str,
    opacity: f32,
) -> (Svgr<'static>, f32, f32) {
    let widths = [
        text_width(ROMAN_FONT, before, TITLE_SIZE),
        text_width(ITALIC_FONT, word, TITLE_SIZE * ITALIC),
        text_width(ROMAN_FONT, after, TITLE_SIZE),
    ];
    let left = (WIDTH as f32 - widths.iter().sum::<f32>()) * 0.5;
    let (word_left, word_right) = (left + widths[0], left + widths[0] + widths[1]);
    let line = svgr!(
        <g opacity={opacity}>
            <text x={left} y={y} font-family={SERIF} font-size={TITLE_SIZE} fill={WHITE}>{before}</text>
            <text x={word_left} y={y} font-family={SERIF} font-style="italic" font-size={TITLE_SIZE * ITALIC}
                fill={word_fill}>{word}</text>
            <text x={word_right} y={y} font-family={SERIF} font-size={TITLE_SIZE} fill={WHITE}>{after}</text>
        </g>
    );
    (line, word_left, word_right)
}

/// The title at `t` seconds into shot 10. The second line ends with a warm line drawn under
/// "why", like the line in the dust of shot 6. The name of the film and the credit follow.
pub fn title(t: f32) -> Svgr<'static> {
    let middle = (BAR + PICTURE_HEIGHT / 2) as f32;
    // the first line comes, then steps back for the second
    let how = fade_in(t, 0.3, 0.9) * (1.0 - 0.65 * fade_in(t, 2.1, 0.8));
    let why = fade_in(t, 2.3, 1.0);
    let drift = |start: f32| 18.0 * (1.0 - fade_in(t, start, 1.6));
    let (first, _, _) = title_line(
        "The ",
        "how",
        " is automated.",
        middle - 90.0 + drift(0.3),
        WHITE,
        how,
    );
    let y = middle + 110.0 + drift(2.3);
    let (second, word_left, word_right) = title_line("The ", "why", " is yours.", y, WARM, why);

    // the warm line, drawn from left to right under the word
    let drawn = fade_in(t, 3.1, 0.9);
    let line_y = y + 52.0;
    let length = ((word_right - word_left + 40.0) * drawn).max(0.01);
    let start = word_left - 20.0;
    let end = start + length;
    // the drawing tip glows brighter while it moves
    let tip = (1.0 - fade_in(t, 3.9, 0.6)) * drawn;
    let credit = fade_in(t, 3.9, 0.7);
    let out = 1.0 - smoothstep(5.6, 6.0, t);
    svgr!(
        <g>
            <defs>
                <radialGradient id="why-glow" cx="50%" cy="50%" r="50%">
                    <stop offset="0" stop-color={WARM} stop-opacity="0.22" />
                    <stop offset="1" stop-color={WARM} stop-opacity="0" />
                </radialGradient>
                <radialGradient id="tip-glow" cx="50%" cy="50%" r="50%">
                    <stop offset="0" stop-color="#fff2d8" stop-opacity="0.9" />
                    <stop offset="0.3" stop-color={WARM} stop-opacity="0.45" />
                    <stop offset="1" stop-color={WARM} stop-opacity="0" />
                </radialGradient>
            </defs>
            <rect x="0" y={BAR} width={WIDTH} height={PICTURE_HEIGHT} fill="#000" />
            <g opacity={out}>
                <ellipse cx={f32::midpoint(word_left, word_right)} cy={y - 40.0} rx="420" ry="190"
                    fill="url(#why-glow)" opacity={why} />
                {first}
                {second}
                <rect x={start} y={line_y - 9.0} width={length} height="18" rx="9" fill={WARM} opacity={0.18 * drawn} />
                <rect x={start} y={line_y - 2.5} width={length} height="5" rx="2.5" fill={WARM} />
                <rect x={start} y={line_y - 1.0} width={length} height="2" rx="1" fill="#fff2d8" opacity="0.8" />
                <ellipse cx={end} cy={line_y} rx="70" ry="34" fill="url(#tip-glow)" opacity={tip} />
                <text x={WIDTH as f32 * 0.5} y={(BAR + PICTURE_HEIGHT) as f32 - 150.0} font-family={SERIF}
                    font-style="italic" font-size="64" fill={WHITE} text-anchor="middle" opacity={credit * 0.85}>
                    "The Why"
                </text>
                <text x={WIDTH as f32 * 0.5} y={(BAR + PICTURE_HEIGHT) as f32 - 90.0} font-family={SERIF} font-size="30"
                    letter-spacing="7" fill="#8a8f98" text-anchor="middle" opacity={credit}>
                    "MADE WITH FFRAMES"
                </text>
            </g>
        </g>
    )
}

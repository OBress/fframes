//! Draws retargeted mocap clips as stick figures into an SVG contact sheet.
use fframes_anime::bvh::Bvh;
use fframes_anime::glam::Vec3;
use fframes_anime::rig::{BONES, Bone, Proportions, Retarget, Skeleton};
use std::fmt::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = &args[1];
    let clips: Vec<&str> = args[2].split(',').collect();
    let out = &args[3];
    let sk = Skeleton::new(Proportions::anime_adult());
    let cols = 8;
    let cell = 220.0;
    let mut svg = String::new();
    let rows = clips.len();
    write!(svg, "<svg xmlns='http://www.w3.org/2000/svg' width='{}' height='{}'><rect width='100%' height='100%' fill='#f5e100'/>", cols as f32 * cell, rows as f32 * cell).unwrap();
    for (row, clip) in clips.iter().enumerate() {
        let src = std::fs::read_to_string(format!("{dir}/{clip}.bvh")).unwrap();
        let bvh = Bvh::parse(&src, 0.01).unwrap();
        let rt = Retarget::new(&bvh, &sk, 0).unwrap();
        for col in 0..cols {
            let t = bvh.duration() * col as f32 / (cols - 1) as f32;
            let pose = rt.pose_at(&bvh, t);
            let posed = sk.solve(&pose);
            let ox = col as f32 * cell + cell * 0.5;
            let oy = row as f32 * cell + cell * 0.92;
            // front view: x right = -X (character's left on screen right means we look at its front from +Z)
            let proj = |p: Vec3| {
                (
                    ox - (p.x - posed.pos(Bone::Hips).x) * 100.0 + p.z * 0.0,
                    oy - p.y * 100.0,
                )
            };
            for b in BONES {
                if let Some(parent) = b.parent() {
                    let (x1, y1) = proj(posed.pos(parent));
                    let (x2, y2) = proj(posed.pos(b));
                    let color = match b.side() {
                        Some(fframes_anime::rig::Side::Left) => "#c00",
                        Some(_) => "#00c",
                        None => "#000",
                    };
                    write!(svg, "<line x1='{x1}' y1='{y1}' x2='{x2}' y2='{y2}' stroke='{color}' stroke-width='3' stroke-linecap='round'/>").unwrap();
                }
            }
            for side in [
                fframes_anime::rig::Side::Left,
                fframes_anime::rig::Side::Right,
            ] {
                let (x1, y1) = proj(posed.pos(Bone::arm(side)[3]));
                let (x2, y2) = proj(posed.hand_tip(side));
                write!(
                    svg,
                    "<line x1='{x1}' y1='{y1}' x2='{x2}' y2='{y2}' stroke='#080' stroke-width='3'/>"
                )
                .unwrap();
            }
            let (hx, hy) = proj(posed.pos(Bone::Head));
            let (tx, ty) = proj(posed.head_top());
            write!(
                svg,
                "<line x1='{hx}' y1='{hy}' x2='{tx}' y2='{ty}' stroke='#000' stroke-width='9'/>"
            )
            .unwrap();
            // nose: head forward
            let nose = posed.attach(Bone::Head, Vec3::new(0.0, 0.1, 0.12));
            let (nx, ny) = proj(nose);
            write!(svg, "<circle cx='{nx}' cy='{ny}' r='3' fill='#f0f'/>").unwrap();
            write!(
                svg,
                "<text x='{}' y='{}' font-size='11'>{clip} {t:.1}s</text>",
                col as f32 * cell + 4.0,
                row as f32 * cell + 12.0
            )
            .unwrap();
        }
    }
    svg.push_str("</svg>");
    std::fs::write(out, svg).unwrap();
}

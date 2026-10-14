//! Prints the root path and heading of mocap clips (to spot clips that turn around or stop).
use std::fmt::Write;

use fframes_anime::bvh::Bvh;
use fframes_anime::glam::Vec3;
use fframes_anime::rig::{Bone, Proportions, Retarget, Skeleton};

fn main() {
    let dir = std::env::args().nth(1).expect("mocap dir");
    let names: Vec<String> = std::env::args().skip(2).collect();
    let sk = Skeleton::new(Proportions::anime_adult());
    for name in names {
        let src = std::fs::read_to_string(format!("{dir}/dataset-1_{name}_001.bvh")).expect("clip");
        let bvh = Bvh::parse(&src, 0.01).expect("bvh");
        let rt = Retarget::new(&bvh, &sk, 0).expect("retarget");
        let d = bvh.duration();
        let mut line = format!("{name:22} {d:5.1}s |");
        let mut t = 0.0;
        while t <= d {
            let pose = rt.pose_at(&bvh, t);
            let f = pose.get(Bone::Hips) * Vec3::Z;
            let heading = f.x.atan2(f.z).to_degrees();
            let _ = write!(
                line,
                " {t:.0}s:({:+.1},{:+.1}) {heading:+4.0}°",
                pose.root.x, pose.root.z
            );
            t += 1.0;
        }
        println!("{line}");
    }
}

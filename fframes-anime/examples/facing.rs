use fframes_anime::bvh::Bvh;
use fframes_anime::glam::Vec3;
use fframes_anime::rig::{Bone, Proportions, Retarget, Skeleton};
fn main() {
    let dir = std::env::args().nth(1).unwrap();
    let sk = Skeleton::new(Proportions::anime_adult());
    for name in [
        "walk_proud",
        "walk_normal",
        "walk_happy",
        "walk_chimpira",
        "walk_musical",
        "walk_active",
        "dance-long_normal",
        "byebye_happy",
    ] {
        let src = std::fs::read_to_string(format!("{dir}/dataset-1_{name}_001.bvh")).unwrap();
        let bvh = Bvh::parse(&src, 0.01).unwrap();
        let rt = Retarget::new(&bvh, &sk, 0).unwrap();
        for t in [0.0, 1.0, 2.0] {
            let pose = rt.pose_at(&bvh, t);
            let f = |b: Bone| pose.get(b) * Vec3::Z;
            let u = |b: Bone| pose.get(b) * Vec3::Y;
            println!(
                "{name:18} t{t} head fwd {:.2?} up {:.2?} | chest fwd {:.2?} | hips fwd {:.2?} | handL {:.2?}",
                f(Bone::Head),
                u(Bone::Head),
                f(Bone::Chest),
                f(Bone::Hips),
                pose.get(Bone::HandL) * Vec3::X
            );
        }
    }
}

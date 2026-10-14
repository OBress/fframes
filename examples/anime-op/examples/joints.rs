use anime_op::cast::{Who, member};
use fframes_anime::rig::{BONES, Bone};
fn main() {
    let sk = member(Who::Neogoose).skeleton();
    let p = sk.solve(&sk.rest_pose());
    for b in BONES {
        let v = p.pos(b);
        println!("{b:?}: {:.3} {:.3} {:.3}", v.x, v.y, v.z);
    }
    let _ = Bone::Chest;
}

//! The cast's hair and ties settle where they should in a still pose.

use anime_op::cast::{Who, member};
use fframes_anime::glam::Vec3;
use fframes_anime::spring;

fn settled(who: Who) -> (spring::Snapshot, fframes_anime::toon::RigLayout) {
    let m = member(who);
    let sk = m.skeleton();
    let (rig, layout) = m.design.physics(&sk.proportions);
    let posed = sk.solve(&sk.rest_pose());
    let baked = spring::bake(&rig, 0.0, 2.0, 24.0, 10, 0.8, |_| {
        m.design.body_state(&posed, Vec3::ZERO)
    });
    (baked.at(2.0), layout)
}

#[test]
fn tie_hangs_down_the_shirt() {
    let (snap, layout) = settled(Who::Chshersh);
    let chain = &snap.chains[layout.tie.unwrap()];
    let drop = chain[0].y - chain[chain.len() - 1].y;
    assert!(drop > 0.3, "the tie only drops {drop} m");
    let lean = (chain[chain.len() - 1].z - chain[0].z).abs();
    assert!(lean < 0.05, "the tie sticks out {lean} m");
}

#[test]
fn quiff_stands_up() {
    let (snap, layout) = settled(Who::Neogoose);
    for i in layout.hair.clone().take(5) {
        let chain = &snap.chains[i];
        let rise = chain[chain.len() - 1].y - chain[0].y;
        assert!(rise > 0.04, "quiff lock {i} rises only {rise} m");
    }
}

/// A cream puff dropped from 40 cm lands on an open, palm-up hand and stays on it upright: the
/// hand's collider is the drawn hand mesh.
#[test]
fn cream_puff_lands_in_the_palm() {
    use anime_op::procedural::{Arms, Body, build};
    use fframes_anime::glam::Quat;
    use fframes_anime::rig::{Bone, Side};
    use fframes_anime::toon::HandShape;

    let sk = member(Who::Neogoose).skeleton();
    let fig = anime_op::render::figure(Who::Neogoose);
    let pose = build(
        &sk,
        Body {
            root: Vec3::new(0.0, sk.proportions.hip_height, 0.0),
            ..Default::default()
        },
        Arms {
            right: Some(Vec3::new(-0.05, 0.05, 0.38)),
            right_palm: Some(Vec3::new(0.0, 1.0, -0.15)),
            right_fingers: Some(Vec3::new(-0.2, 0.15, 1.0)),
            ..Default::default()
        },
        None,
    );
    let posed = sk.solve(&pose);
    let palm = posed.pos(Bone::HandR) + posed.rot(Bone::HandR) * fig.palm(Side::Right);
    let toss = anime_op::physics3d::Toss {
        hull: anime_op::props::puff_hull(),
        scale: 1.0,
        pos: palm + Vec3::new(0.0, 0.4, 0.0),
        rot: Quat::from_rotation_z(0.2),
        vel: Vec3::ZERO,
        spin: Vec3::ZERO,
        restitution: 0.2,
        friction: 1.1,
        release: 0.0,
    };
    let baked = anime_op::physics3d::bake(
        &toss,
        &fig,
        &[(Side::Right, HandShape::Relaxed)],
        &|_| posed,
        &[],
        0.0,
        1.5,
    );
    assert!(baked.landed.is_some(), "it never touched the hand");
    let (p, r) = baked.at(1.5);
    let off = (p - palm) * Vec3::new(1.0, 0.0, 1.0);
    assert!(off.length() < 0.07, "it came to rest {off} from the palm");
    assert!(
        p.y > palm.y - 0.02 && p.y < palm.y + 0.05,
        "height {} vs palm {}",
        p.y,
        palm.y
    );
    assert!((r * Vec3::Y).y > 0.7, "it fell over: up = {}", r * Vec3::Y);
}

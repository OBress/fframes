//! Motion sources: Bandai Namco Research motion capture (retargeted to each cast member) and
//! hand-keyed moves built on the same skeleton.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use fframes_anime::bvh::Bvh;
use fframes_anime::glam::{Quat, Vec3};
use fframes_anime::rig::{Pose, Retarget, Skeleton};

pub type MotionFn = Arc<dyn Fn(f32) -> Pose + Send + Sync>;

/// The clips of the Bandai Namco Research Motion Dataset 1 used in the video.
pub const CLIPS: &[&str] = &[
    "walk_normal",
    "walk_proud",
    "walk_chimpira",
    "walk_happy",
    "walk_active",
    "walk_masculinity",
    "walk_musical",
    "walk_not-confident",
    "run_active",
    "run_normal",
    "dash_active",
    "dash_normal",
    "byebye_happy",
    "byebye_active",
    "bye_proud",
    "bow_proud",
    "guide_normal",
    "call_normal",
    "respond_normal",
    "kick_normal",
    "punch_normal",
    "slash_normal",
    "dance-long_normal",
    "dance-short_normal",
];

#[derive(Default)]
pub struct Library {
    clips: HashMap<&'static str, Arc<Bvh>>,
}

impl Library {
    /// Reads `dataset-1_{name}_001.bvh` for every clip in [`CLIPS`] from `dir`. Missing files are
    /// skipped (the video then shows the hand-keyed fallback).
    pub fn load(dir: &Path) -> Self {
        let mut clips = HashMap::new();
        for name in CLIPS {
            let path = dir.join(format!("dataset-1_{name}_001.bvh"));
            match std::fs::read_to_string(&path) {
                Ok(src) => match Bvh::parse(&src, 0.01) {
                    Ok(bvh) => {
                        clips.insert(*name, Arc::new(bvh));
                    }
                    Err(e) => eprintln!("{}: {e}", path.display()),
                },
                Err(e) => eprintln!("{}: {e}", path.display()),
            }
        }
        Library { clips }
    }

    pub fn get(&self, name: &str) -> Option<Arc<Bvh>> {
        self.clips.get(name).cloned()
    }
}

/// Where and how a clip plays.
#[derive(Debug, Clone, Copy)]
pub struct Play {
    /// Seconds into the clip at shot time 0.
    pub from: f32,
    pub speed: f32,
    pub looped: bool,
    /// Turn around Y in degrees (0 = towards the camera at +Z).
    pub yaw: f32,
    pub at: Vec3,
}

impl Default for Play {
    fn default() -> Self {
        Play {
            from: 0.0,
            speed: 1.0,
            looped: true,
            yaw: 0.0,
            at: Vec3::ZERO,
        }
    }
}

/// A mocap clip as a motion. Looping clips keep walking forward: every loop continues from where
/// the previous one ended.
pub fn clip(lib: &Library, name: &str, skeleton: &Skeleton, play: Play) -> MotionFn {
    let Some(bvh) = lib.get(name) else {
        let rest = skeleton.rest_pose();
        return Arc::new(move |_| rest);
    };
    let retarget = Retarget::new(&bvh, skeleton, 0).expect("Bandai Namco joint names");
    let duration = bvh.duration();
    let start_pose = retarget.pose_at(&bvh, 0.0);
    let end_pose = retarget.pose_at(&bvh, duration);
    let travel = Vec3::new(
        end_pose.root.x - start_pose.root.x,
        0.0,
        end_pose.root.z - start_pose.root.z,
    );
    let turn = Quat::from_rotation_y(play.yaw.to_radians());
    Arc::new(move |t| {
        let ct = play.from + t * play.speed;
        let (time, loops) = if play.looped && duration > 0.0 {
            let loops = (ct / duration).floor();
            (ct - loops * duration, loops)
        } else {
            (ct.clamp(0.0, duration), 0.0)
        };
        let mut pose = retarget.pose_at(&bvh, time);
        pose.root += travel * loops;
        pose.transformed(turn, play.at)
    })
}

/// A clip that stays in place (the travel is removed), for dancing and turntables.
pub fn in_place(lib: &Library, name: &str, skeleton: &Skeleton, play: Play) -> MotionFn {
    let Some(bvh) = lib.get(name) else {
        let rest = skeleton.rest_pose();
        return Arc::new(move |_| rest);
    };
    let retarget = Retarget::new(&bvh, skeleton, 0).expect("Bandai Namco joint names");
    let duration = bvh.duration();
    let turn = Quat::from_rotation_y(play.yaw.to_radians());
    Arc::new(move |t| {
        let ct = play.from + t * play.speed;
        let time = if play.looped && duration > 0.0 {
            ct.rem_euclid(duration)
        } else {
            ct.clamp(0.0, duration)
        };
        let mut pose = retarget.pose_at(&bvh, time);
        pose.root.x = 0.0;
        pose.root.z = 0.0;
        pose.transformed(turn, play.at)
    })
}

//! Biovision Hierarchy (BVH) motion capture files: the hierarchy, the per-frame channels and
//! forward kinematics into world space.

use std::fmt;

use glam::{EulerRot, Quat, Vec3};

#[derive(Debug)]
pub enum BvhError {
    UnexpectedEnd,
    Unexpected {
        expected: &'static str,
        found: String,
    },
    BadNumber(String),
    ChannelCount {
        expected: usize,
        found: usize,
    },
}

impl fmt::Display for BvhError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BvhError::UnexpectedEnd => write!(f, "unexpected end of the BVH file"),
            BvhError::Unexpected { expected, found } => {
                write!(f, "expected {expected}, found {found:?}")
            }
            BvhError::BadNumber(token) => write!(f, "not a number: {token:?}"),
            BvhError::ChannelCount { expected, found } => {
                write!(
                    f,
                    "{expected} channel values expected per frame, found {found}"
                )
            }
        }
    }
}

impl std::error::Error for BvhError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    PosX,
    PosY,
    PosZ,
    RotX,
    RotY,
    RotZ,
}

#[derive(Debug, Clone)]
pub struct BvhJoint {
    pub name: String,
    pub parent: Option<usize>,
    pub offset: Vec3,
    pub channels: Vec<Channel>,
    /// Index of the first channel of this joint in a frame row.
    pub first_channel: usize,
}

/// A parsed BVH clip. Positions are converted with `unit_scale` (centimetres to metres by default).
#[derive(Debug, Clone)]
pub struct Bvh {
    pub joints: Vec<BvhJoint>,
    pub frame_count: usize,
    pub frame_time: f32,
    channels_per_frame: usize,
    values: Vec<f32>,
    unit_scale: f32,
}

/// World transform of one joint.
#[derive(Debug, Clone, Copy)]
pub struct JointWorld {
    pub position: Vec3,
    pub rotation: Quat,
}

struct Tokens<'a> {
    inner: std::str::SplitWhitespace<'a>,
}

impl<'a> Tokens<'a> {
    fn next(&mut self) -> Result<&'a str, BvhError> {
        self.inner.next().ok_or(BvhError::UnexpectedEnd)
    }

    fn expect(&mut self, word: &'static str) -> Result<(), BvhError> {
        let token = self.next()?;
        if token.eq_ignore_ascii_case(word) {
            Ok(())
        } else {
            Err(BvhError::Unexpected {
                expected: word,
                found: token.to_owned(),
            })
        }
    }

    fn number(&mut self) -> Result<f32, BvhError> {
        let token = self.next()?;
        token
            .parse::<f32>()
            .map_err(|_| BvhError::BadNumber(token.to_owned()))
    }
}

impl Bvh {
    /// Parses a BVH file. Offsets and position channels are multiplied by `unit_scale`
    /// (`0.01` turns the usual centimetres into metres).
    pub fn parse(source: &str, unit_scale: f32) -> Result<Bvh, BvhError> {
        let mut tokens = Tokens {
            inner: source.split_whitespace(),
        };
        tokens.expect("HIERARCHY")?;

        let mut joints: Vec<BvhJoint> = Vec::new();
        let mut stack: Vec<Option<usize>> = Vec::new();
        let mut channel_cursor = 0;

        tokens.expect("ROOT")?;
        let mut pending_name = Some(tokens.next()?.to_owned());

        loop {
            let token = tokens.next()?;
            match token {
                "{" => {
                    // `pending_name == None` marks an End Site block.
                    let index = pending_name.take().map(|name| {
                        joints.push(BvhJoint {
                            name,
                            parent: stack.iter().rev().find_map(|j| *j),
                            offset: Vec3::ZERO,
                            channels: Vec::new(),
                            first_channel: channel_cursor,
                        });
                        joints.len() - 1
                    });
                    stack.push(index);
                }
                "}" => {
                    stack.pop();
                    if stack.is_empty() {
                        break;
                    }
                }
                "OFFSET" => {
                    let offset = Vec3::new(tokens.number()?, tokens.number()?, tokens.number()?)
                        * unit_scale;
                    if let Some(Some(index)) = stack.last() {
                        joints[*index].offset = offset;
                    }
                }
                "CHANNELS" => {
                    let count = tokens.number()? as usize;
                    let index = stack
                        .last()
                        .copied()
                        .flatten()
                        .ok_or(BvhError::Unexpected {
                            expected: "a joint before CHANNELS",
                            found: "End Site".to_owned(),
                        })?;
                    joints[index].first_channel = channel_cursor;
                    for _ in 0..count {
                        let name = tokens.next()?;
                        let channel = match name.to_ascii_lowercase().as_str() {
                            "xposition" => Channel::PosX,
                            "yposition" => Channel::PosY,
                            "zposition" => Channel::PosZ,
                            "xrotation" => Channel::RotX,
                            "yrotation" => Channel::RotY,
                            "zrotation" => Channel::RotZ,
                            _ => {
                                return Err(BvhError::Unexpected {
                                    expected: "a channel name",
                                    found: name.to_owned(),
                                });
                            }
                        };
                        joints[index].channels.push(channel);
                    }
                    channel_cursor += count;
                }
                "JOINT" => pending_name = Some(tokens.next()?.to_owned()),
                "End" => {
                    tokens.expect("Site")?;
                    pending_name = None;
                }
                other => {
                    return Err(BvhError::Unexpected {
                        expected: "a hierarchy keyword",
                        found: other.to_owned(),
                    });
                }
            }
        }

        tokens.expect("MOTION")?;
        tokens.expect("Frames:")?;
        let frame_count = tokens.number()? as usize;
        tokens.expect("Frame")?;
        tokens.expect("Time:")?;
        let frame_time = tokens.number()?;

        let mut values = Vec::with_capacity(frame_count * channel_cursor);
        while let Ok(token) = tokens.next() {
            values.push(
                token
                    .parse::<f32>()
                    .map_err(|_| BvhError::BadNumber(token.to_owned()))?,
            );
        }
        if values.len() != frame_count * channel_cursor {
            return Err(BvhError::ChannelCount {
                expected: frame_count * channel_cursor,
                found: values.len(),
            });
        }

        Ok(Bvh {
            joints,
            frame_count,
            frame_time,
            channels_per_frame: channel_cursor,
            values,
            unit_scale,
        })
    }

    pub fn joint(&self, name: &str) -> Option<usize> {
        self.joints.iter().position(|j| j.name == name)
    }

    pub fn duration(&self) -> f32 {
        self.frame_count.saturating_sub(1) as f32 * self.frame_time
    }

    /// Local translation and rotation of a joint at a frame. Rotation channels are applied in the
    /// order they are listed (`Zrotation Xrotation Yrotation` is `Rz * Rx * Ry`).
    fn local(&self, joint: &BvhJoint, frame: usize) -> (Vec3, Quat) {
        let row = &self.values[frame * self.channels_per_frame..];
        let mut translation = joint.offset;
        let mut has_position = false;
        let mut rotation = Quat::IDENTITY;
        for (i, channel) in joint.channels.iter().enumerate() {
            let v = row[joint.first_channel + i];
            match channel {
                Channel::PosX | Channel::PosY | Channel::PosZ => {
                    if !has_position {
                        translation = Vec3::ZERO;
                        has_position = true;
                    }
                    let v = v * self.unit_scale;
                    match channel {
                        Channel::PosX => translation.x = v,
                        Channel::PosY => translation.y = v,
                        _ => translation.z = v,
                    }
                }
                Channel::RotX => rotation *= Quat::from_rotation_x(v.to_radians()),
                Channel::RotY => rotation *= Quat::from_rotation_y(v.to_radians()),
                Channel::RotZ => rotation *= Quat::from_rotation_z(v.to_radians()),
            }
        }
        (translation, rotation)
    }

    /// Forward kinematics of one frame.
    pub fn world_at_frame(&self, frame: usize) -> Vec<JointWorld> {
        let frame = frame.min(self.frame_count.saturating_sub(1));
        let mut out: Vec<JointWorld> = Vec::with_capacity(self.joints.len());
        for joint in &self.joints {
            let (t, r) = self.local(joint, frame);
            let world = match joint.parent {
                Some(p) => {
                    let parent = out[p];
                    JointWorld {
                        position: parent.position + parent.rotation * t,
                        rotation: (parent.rotation * r).normalize(),
                    }
                }
                None => JointWorld {
                    position: t,
                    rotation: r,
                },
            };
            out.push(world);
        }
        out
    }

    /// Forward kinematics at a time in seconds, interpolated between the two nearest frames.
    pub fn world_at(&self, seconds: f32) -> Vec<JointWorld> {
        let f = (seconds / self.frame_time).max(0.0);
        let a = f.floor() as usize;
        let t = f - a as f32;
        let wa = self.world_at_frame(a);
        if t <= 1e-4 || a + 1 >= self.frame_count {
            return wa;
        }
        let wb = self.world_at_frame(a + 1);
        wa.iter()
            .zip(&wb)
            .map(|(a, b)| JointWorld {
                position: a.position.lerp(b.position, t),
                rotation: a.rotation.slerp(b.rotation, t),
            })
            .collect()
    }

    /// Euler angles (degrees, ZXY) of a joint at a frame, handy when inspecting a clip.
    pub fn local_euler_degrees(&self, joint: usize, frame: usize) -> Vec3 {
        let (_, r) = self.local(&self.joints[joint], frame);
        let (z, x, y) = r.to_euler(EulerRot::ZXY);
        Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "HIERARCHY
ROOT Hips
{
  OFFSET 0 100 0
  CHANNELS 6 Xposition Yposition Zposition Zrotation Xrotation Yrotation
  JOINT Spine
  {
    OFFSET 0 10 0
    CHANNELS 3 Zrotation Xrotation Yrotation
    End Site
    {
      OFFSET 0 10 0
    }
  }
}
MOTION
Frames: 2
Frame Time: 0.5
0 100 0 0 0 0 0 0 0
10 100 0 90 0 0 0 0 0
";

    #[test]
    fn parses_and_runs_forward_kinematics() {
        let bvh = Bvh::parse(SAMPLE, 0.01).unwrap();
        assert_eq!(bvh.joints.len(), 2);
        assert_eq!(bvh.frame_count, 2);
        let f0 = bvh.world_at_frame(0);
        assert!((f0[1].position - Vec3::new(0.0, 1.1, 0.0)).length() < 1e-5);
        // hips rolled 90 degrees around Z: the spine now points to -X
        let f1 = bvh.world_at_frame(1);
        assert!((f1[1].position - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-5);
        assert!((f1[0].position - Vec3::new(0.1, 1.0, 0.0)).length() < 1e-5);
        let mid = bvh.world_at(0.25);
        assert!((mid[0].position.x - 0.05).abs() < 1e-5);
    }
}

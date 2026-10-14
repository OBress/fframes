//! Character animation for fframes in the way Japanese studios build it: motion capture
//! retargeted onto anime proportions ([`rig`]), secondary motion from VRM style spring bones and
//! cloth ([`spring`]), exposure on ones, twos and threes ([`koma`]), and toon characters built
//! like an anime 3DCG department builds them: models of rounded primitives ([`sdf`]) meshed and
//! skinned to the skeleton ([`mesh`], [`figure`]), cel shaded ([`raster`]) and inked with vector
//! line art ([`ink`]).

pub mod acting;
pub mod bvh;
pub mod camera;
pub mod figure;
pub mod ink;
pub mod koma;
pub mod mesh;
pub mod prop;
pub mod raster;
pub mod rig;
pub mod sdf;
pub mod shape;
pub mod spring;
pub mod toon;

pub use glam;

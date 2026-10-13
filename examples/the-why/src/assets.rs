//! The CC0 skies and materials from Poly Haven in `dynamic_media` (see `ASSETS.md`). They
//! load once, before the first frame.

use std::path::Path;
use std::sync::Arc;

use fframes_3d::{EnvironmentMap, Material, Rgb, Texture};

/// A set of physically based textures. `orm` holds the ambient occlusion in red, the roughness
/// in green and the metalness in blue, like glTF.
pub struct Pbr {
    pub color: Arc<Texture>,
    pub normal: Arc<Texture>,
    pub orm: Arc<Texture>,
}

impl Pbr {
    /// Loads `<name>_diff.jpg`, `_nor.jpg`, `_rough.jpg`, `_ao.jpg` and, when it exists,
    /// `_metal.jpg`. One repeat of the textures covers `meters` meters of UV space.
    fn load(dir: &Path, name: &str, meters: f32) -> Result<Self, String> {
        let read = |suffix: &str| {
            let path = dir.join(format!("{name}_{suffix}.jpg"));
            std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))
        };
        let gray = |suffix: &str| -> Result<Option<image::GrayImage>, String> {
            let path = dir.join(format!("{name}_{suffix}.jpg"));
            if !path.exists() {
                return Ok(None);
            }
            let bytes = read(suffix)?;
            let image = image::load_from_memory(&bytes).map_err(|e| format!("{name}: {e}"))?;
            Ok(Some(image.to_luma8()))
        };
        let repeat = |t: Texture| Arc::new(t.repeat(1.0 / meters, 1.0 / meters));

        let color = Texture::decode(&read("diff")?).map_err(|e| format!("{name}: {e}"))?;
        let normal = Texture::decode_linear(&read("nor")?).map_err(|e| format!("{name}: {e}"))?;
        let rough = gray("rough")?.ok_or_else(|| format!("{name}: no roughness map"))?;
        let ao = gray("ao")?;
        let metal = gray("metal")?;
        let (w, h) = rough.dimensions();
        let at = |image: &Option<image::GrayImage>, x: u32, y: u32, default: u8| {
            image.as_ref().map_or(default, |i| {
                // the maps of a set can have different sizes
                let (iw, ih) = i.dimensions();
                i.get_pixel(x * iw / w, y * ih / h).0[0]
            })
        };
        let orm: Vec<u8> = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .flat_map(|(x, y)| {
                [
                    at(&ao, x, y, 255),
                    rough.get_pixel(x, y).0[0],
                    at(&metal, x, y, 0),
                    255,
                ]
            })
            .collect();
        Ok(Self {
            color: repeat(color),
            normal: repeat(normal),
            orm: repeat(Texture::from_rgba8(w, h, &orm, false)),
        })
    }

    /// A material with all maps of the set. The roughness and the metalness of the maps
    /// multiply `roughness` and `metalness`.
    pub fn material(&self, tint: impl Into<Rgb>, roughness: f32, metalness: f32) -> Material {
        let mut material = Material::standard(tint)
            .roughness(roughness)
            .map(self.color.clone())
            .normal_map(self.normal.clone())
            .ao_map(self.orm.clone());
        material.metalness = metalness;
        material.metal_rough_map = Some(self.orm.clone());
        material
    }
}

/// Everything in `dynamic_media`.
pub struct Assets {
    pub concrete: Pbr,
    pub steel: Pbr,
    pub painted_metal: Pbr,
    pub corrugated: Pbr,
    pub asphalt: Pbr,
    pub sand: Pbr,
    pub dirt: Pbr,
    pub rock: Pbr,
    pub factory_sky: Arc<EnvironmentMap>,
    pub dusk_sky: Arc<EnvironmentMap>,
    pub sunrise_sky: Arc<EnvironmentMap>,
}

impl Assets {
    /// Loads all files from `dir`, the sets in parallel.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let sky = |name: &str| -> Result<Arc<EnvironmentMap>, String> {
            let path = dir.join(name);
            let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            EnvironmentMap::decode(&bytes)
                .map(Arc::new)
                .map_err(|e| format!("{name}: {e}"))
        };
        std::thread::scope(|s| {
            let set =
                |name: &'static str, meters: f32| s.spawn(move || Pbr::load(dir, name, meters));
            let concrete = set("concrete", 3.0);
            let steel = set("steel", 1.5);
            let painted_metal = set("painted_metal", 1.0);
            let corrugated = set("corrugated_metal", 2.0);
            let asphalt = set("asphalt", 6.0);
            let sand = set("sand", 2.5);
            let dirt = set("dirt", 1.5);
            let rock = set("canyon_rock", 6.0);
            let factory_sky = s.spawn(|| sky("factory_interior.hdr"));
            let dusk_sky = s.spawn(|| sky("sky_dusk.hdr"));
            let sunrise_sky = s.spawn(|| sky("sky_sunrise.hdr"));
            let join = |h: std::thread::ScopedJoinHandle<'_, Result<Pbr, String>>| {
                h.join().map_err(|_| "a loader panicked".to_owned())?
            };
            let join_sky = |h: std::thread::ScopedJoinHandle<
                '_,
                Result<Arc<EnvironmentMap>, String>,
            >| { h.join().map_err(|_| "a loader panicked".to_owned())? };
            Ok(Self {
                concrete: join(concrete)?,
                steel: join(steel)?,
                painted_metal: join(painted_metal)?,
                corrugated: join(corrugated)?,
                asphalt: join(asphalt)?,
                sand: join(sand)?,
                dirt: join(dirt)?,
                rock: join(rock)?,
                factory_sky: join_sky(factory_sky)?,
                dusk_sky: join_sky(dusk_sky)?,
                sunrise_sky: join_sky(sunrise_sky)?,
            })
        })
    }
}

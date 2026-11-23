//! Machine-local presentation preferences. These never enter a network packet.
use serde_json::{json, Value};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tier {
    Low,
    #[default]
    Medium,
    High,
}

impl Tier {
    #[cfg(test)]
    const ALL: [Self; 3] = [Self::Low, Self::Medium, Self::High];

    fn load(value: &Value) -> Self {
        match value.as_str() {
            Some("low") => Self::Low,
            Some("high") => Self::High,
            _ => Self::Medium,
        }
    }

    fn key(self) -> &'static str {
        ["low", "medium", "high"][self as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Graphics {
    pub shadows: Tier,
    pub models: Tier,
    pub foliage: Tier,
    pub materials: Tier,
    pub saturation: u16,
}

impl Graphics {
    pub fn preset(tier: Tier) -> Self {
        Self {
            shadows: tier,
            models: tier,
            foliage: tier,
            materials: tier,
            saturation: 100,
        }
    }

    pub fn matching_preset(self) -> Option<Tier> {
        let tier = self.shadows;
        (self.models == tier && self.foliage == tier && self.materials == tier).then_some(tier)
    }

    pub fn shadow_size(self) -> u32 {
        // A valid sampler is still required by GL when the shadow branch is off.
        [1, 1024, 2048][self.shadows as usize]
    }

    pub fn model_distance(self) -> f32 {
        [56., 93., 150.][self.models as usize]
    }

    pub fn foliage_distance(self) -> f32 {
        [42., 88., 140.][self.foliage as usize]
    }

    pub fn shader(self) -> [f32; 4] {
        [
            self.shadows as usize as f32,
            1. / self.shadow_size() as f32,
            self.saturation as f32 / 100.,
            [0., 0.55, 1.][self.materials as usize],
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub graphics: Graphics,
    pub fullscreen: bool,
    pub show_fps: bool,
    pub sensitivity: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            graphics: Graphics::preset(Tier::Medium),
            fullscreen: false,
            show_fps: true,
            sensitivity: 100,
        }
    }
}

fn bounded(value: &Value, min: i64, max: i64) -> u16 {
    value.as_i64().unwrap_or(100).clamp(min, max) as u16
}

impl Settings {
    fn from_value(v: &Value) -> Self {
        let g = &v["graphics"];
        Self {
            graphics: Graphics {
                shadows: Tier::load(&g["shadows"]),
                models: Tier::load(&g["models"]),
                foliage: Tier::load(&g["foliage"]),
                materials: Tier::load(&g["materials"]),
                saturation: bounded(&g["saturation"], 70, 140),
            },
            fullscreen: v["fullscreen"].as_bool().unwrap_or(false),
            show_fps: v["show_fps"].as_bool().unwrap_or(true),
            sensitivity: bounded(&v["sensitivity"], 30, 200),
        }
    }

    fn path() -> io::Result<PathBuf> {
        #[cfg(windows)]
        let base = env::var_os("APPDATA").map(PathBuf::from);
        #[cfg(not(windows))]
        let base = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")));
        base.map(|p| p.join("frontline-surge").join("settings.json"))
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Каталог профиля недоступен"))
    }

    pub fn load() -> Self {
        Self::path()
            .ok()
            .and_then(|p| fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .map(|v: Value| Self::from_value(&v))
            .unwrap_or_default()
    }

    pub fn save(self) -> io::Result<()> {
        self.save_at(&Self::path()?)
    }

    fn save_at(self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path.parent().expect("profile path has parent"))?;
        let g = self.graphics;
        let content = json!({
            "version": 1, "fullscreen": self.fullscreen, "show_fps": self.show_fps,
            "sensitivity": self.sensitivity,
            "graphics": {
                "shadows": g.shadows.key(), "models": g.models.key(),
                "foliage": g.foliage.key(), "materials": g.materials.key(), "saturation": g.saturation,
            },
        });
        let temp = path.with_extension(format!("{}.tmp", std::process::id()));
        let result = (|| {
            fs::write(&temp, serde_json::to_vec_pretty(&content)?)?;
            fs::rename(&temp, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_round_trip_and_replace_the_previous_file() {
        let dir = env::temp_dir().join(format!("frontline-prefs-test-{}", std::process::id()));
        let path = dir.join("settings.json");
        let mut s = Settings::default();
        s.save_at(&path).unwrap();
        s.graphics = Graphics::preset(Tier::High);
        s.graphics.foliage = Tier::Low;
        s.graphics.saturation = 115;
        s.fullscreen = true;
        s.show_fps = false;
        s.sensitivity = 45;
        s.save_at(&path).unwrap();
        let value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(Settings::from_value(&value), s);
        assert_eq!(s.graphics.matching_preset(), None);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sparse_and_invalid_settings_have_safe_defaults() {
        assert_eq!(Settings::from_value(&json!({})), Settings::default());
        let s = Settings::from_value(&json!({
            "sensitivity": 900, "fullscreen": "on", "graphics": {"saturation": -1000, "models": "unknown"},
        }));
        assert_eq!(s.sensitivity, 200);
        assert_eq!(s.graphics.saturation, 70);
        assert_eq!(s.graphics.models, Tier::Medium);
        assert!(!s.fullscreen);
    }

    #[test]
    fn presets_select_distinct_renderer_costs() {
        let presets = Tier::ALL.map(Graphics::preset);
        assert_eq!(presets[0].shader()[0], 0.);
        for pair in presets.windows(2) {
            assert!(pair[0].shadow_size() < pair[1].shadow_size());
            assert!(pair[0].model_distance() < pair[1].model_distance());
            assert!(pair[0].foliage_distance() < pair[1].foliage_distance());
            assert!(pair[0].shader()[3] < pair[1].shader()[3]);
        }
        assert_eq!(presets[2].shader()[1], 1. / presets[2].shadow_size() as f32);
        assert_eq!(presets.map(Graphics::matching_preset), Tier::ALL.map(Some));
    }
}

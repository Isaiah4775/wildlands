use macroquad::prelude::Vec2;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::PathBuf;

/// Save/load helpers. Macroquad's Vec2 has no serde support, so positions are
/// stored as plain (x, y) pairs.
pub mod vec2_serde {
    use super::*;

    pub fn serialize<S>(value: &Vec2, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        (value.x, value.y).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec2, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (x, y) = <(f32, f32)>::deserialize(deserializer)?;
        Ok(Vec2::new(x, y))
    }
}

pub fn save_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("Wildlands")
        .join("save.json")
}

pub fn save_exists() -> bool {
    save_path().is_file()
}

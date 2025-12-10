use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Represents a single lighting condition with its illuminance value and description
#[derive(Debug, Clone, Serialize, Deserialize, Asset, TypePath)]
pub struct LightingCondition {
    pub lux: f32,
    pub description: String,
}

/// Container for all lighting conditions loaded from JSON
#[derive(Debug, Clone, Serialize, Deserialize, Asset, TypePath)]
pub struct LightingConditions {
    pub lighting_conditions: Vec<LightingCondition>,
}

impl LightingConditions {
    /// Find the closest lighting condition to a given lux value
    pub fn find_closest(&self, target_lux: f32) -> Option<&LightingCondition> {
        self.lighting_conditions.iter().min_by(|a, b| {
            (a.lux - target_lux)
                .abs()
                .partial_cmp(&(b.lux - target_lux).abs())
                .unwrap()
        })
    }

    /// Get all lighting conditions sorted by lux value
    pub fn sorted_by_lux(&self) -> Vec<&LightingCondition> {
        let mut conditions: Vec<&LightingCondition> = self.lighting_conditions.iter().collect();
        conditions.sort_by(|a, b| a.lux.partial_cmp(&b.lux).unwrap());
        conditions
    }

    /// Get lighting conditions within a lux range
    pub fn in_range(&self, min_lux: f32, max_lux: f32) -> Vec<&LightingCondition> {
        self.lighting_conditions
            .iter()
            .filter(|condition| condition.lux >= min_lux && condition.lux <= max_lux)
            .collect()
    }
}

/// Common illuminance values as constants for quick access
pub mod illuminance {
    use bevy_light::light_consts::lux;

    pub const STARLIGHT: f32 = lux::MOONLESS_NIGHT;
    pub const NIGHT_AIRGLOW: f32 = 0.002;
    pub const FULL_MOON_MIN: f32 = lux::FULL_MOON_NIGHT;
    pub const FULL_MOON_MAX: f32 = 0.3;
    pub const CIVIL_TWILIGHT: f32 = lux::CIVIL_TWILIGHT;
    pub const PUBLIC_AREAS_MIN: f32 = 20.0;
    pub const OFFICE_CORRIDOR: f32 = 30.0;
    pub const PUBLIC_AREAS_MAX: f32 = lux::LIVING_ROOM;
    pub const LIVING_ROOM: f32 = lux::LIVING_ROOM;
    pub const HALLWAY_LIGHTING: f32 = lux::HALLWAY;
    pub const DARK_OVERCAST_DAY: f32 = lux::DARK_OVERCAST_DAY;
    pub const TRAIN_STATION: f32 = 150.0;
    pub const OFFICE_LIGHTING_MIN: f32 = lux::OFFICE;
    pub const SUNRISE_SUNSET: f32 = lux::CLEAR_SUNRISE;
    pub const OFFICE_LIGHTING_MAX: f32 = 500.0;
    pub const TV_STUDIO: f32 = lux::OVERCAST_DAY;
    pub const DAYLIGHT_INDIRECT_MIN: f32 = lux::AMBIENT_DAYLIGHT;
    pub const DAYLIGHT_INDIRECT_MAX: f32 = lux::FULL_DAYLIGHT;
    pub const DIRECT_SUNLIGHT_MIN: f32 = 32000.0;
    pub const DIRECT_SUNLIGHT_MAX: f32 = lux::DIRECT_SUNLIGHT;
}

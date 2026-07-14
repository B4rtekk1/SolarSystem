use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Orbit {
    pub center: [f32; 3],
    pub semi_major_axis: f32,
    pub semi_minor_axis: f32,
    pub angular_speed: f32,
    pub phase: f32,
    pub inclination: f32,
}

impl Orbit {
    pub fn circular(radius: f32, angular_speed: f32) -> Self {
        Self::elliptical(radius, radius, angular_speed)
    }

    pub fn elliptical(semi_major_axis: f32, semi_minor_axis: f32, angular_speed: f32) -> Self {
        Self {
            center: [0.0, 0.0, 0.0],
            semi_major_axis,
            semi_minor_axis,
            angular_speed,
            phase: 0.0,
            inclination: 0.0,
        }
    }
}

use crate::color::Color;

#[derive(Debug, Clone, Copy)]
pub struct Sun {
    pub speed: f32,
    pub mass: f32,
    pub radius: f32,
    pub orbit: Option<Orbit>,
    pub color: Color,
    pub brightness: f32,
    pub rotation_speed: f32,
    pub surface_temperature: f32,
}

impl Default for Sun {
    fn default() -> Self {
        Self {
            speed: 0.0,
            mass: 1.989e30,
            radius: 1.0,
            orbit: None,
            color: Color::rgb(1.0, 0.72, 0.08),
            brightness: 1.0,
            rotation_speed: 0.15,
            surface_temperature: 5778.0,
        }
    }
}

impl Sun {
    pub fn with_orbit(mut self, orbit: Orbit) -> Self {
        self.orbit = Some(orbit);
        self
    }

    pub fn without_orbit(mut self) -> Self {
        self.orbit = None;
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Orbit {
    pub center: [f32; 3],
    pub radius: f32,
    pub angular_speed: f32,
    pub phase: f32,
}

impl Orbit {
    pub fn new(center: [f32; 3], radius: f32, angular_speed: f32) -> Self {
        Self {
            center,
            radius,
            angular_speed,
            phase: 0.0,
        }
    }
}

use crate::color::Color;
use crate::orbit::Orbit;

#[derive(Debug, Clone)]
pub struct Moon {
    pub name: String,
    pub speed: f32,
    pub mass: f32,
    pub radius: f32,
    pub orbit: Orbit,
    pub shader: MoonShader,
    pub rotation_speed: f32,
    pub surface_temperature: f32,
}

impl Moon {
    pub fn new(name: impl Into<String>, mass: f32, radius: f32, orbit: Orbit) -> Self {
        Self {
            name: name.into(),
            speed: orbit.angular_speed,
            mass,
            radius,
            orbit,
            shader: MoonShader::default(),
            rotation_speed: 0.0,
            surface_temperature: 250.0,
        }
    }

    pub fn with_shader(mut self, shader: MoonShader) -> Self {
        self.shader = shader;
        self
    }

    pub fn with_rotation_speed(mut self, rotation_speed: f32) -> Self {
        self.rotation_speed = rotation_speed;
        self
    }

    pub fn earth_moon() -> Self {
        Self::new("Earth's Moon", 7.342e22, 0.045, Orbit::circular(0.36, 28.0))
    }
}

#[derive(Debug, Clone)]
pub struct MoonShader {
    pub shader_path: String,
    pub base_color: Color,
    pub roughness: f32,
    pub metallic: f32,
}

impl Default for MoonShader {
    fn default() -> Self {
        Self {
            shader_path: "planet.wgsl".to_string(),
            base_color: Color::rgb(0.58, 0.58, 0.54),
            roughness: 0.8,
            metallic: 0.0,
        }
    }
}

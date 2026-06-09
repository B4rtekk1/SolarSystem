use crate::orbit::Orbit;
use crate::color::Color;
#[derive(Debug, Clone)]
pub struct Moon {
    pub name: String,
    pub speed: f32,
    pub mass: f32,
    pub radius: f32,
    pub orbit: Option<Orbit>,
    pub shader: MoonShader,
    pub rotation_speed: f32,
    pub surface_temperature: f32,
}

impl Moon {
    pub fn new(name: impl Into<String>, mass: f32, radius: f32, orbit: Option<Orbit>) -> Self {
        Self {
            name: name.into(),
            speed: orbit.unwrap().angular_speed,
            mass,
            radius,
            orbit,
            shader: MoonShader::default(),
            rotation_speed: 0.0,
            surface_temperature: 250.0,
        }
    }

    pub fn earth_moon() -> Self {
        Self::new(
            "Earth's Moon",
            7.342e22,
            1.737e6,
            Some(Orbit::circular(3.844e8, 2.6617)
        ))
    }
}

#[derive(Debug, Clone)]
pub struct MoonShader {
    pub shader_path: String,
    pub base_color: Color,
    pub roughness: f32,
    pub metallic: f32,
}

impl MoonShader {
    pub fn default() -> Self {
        Self {
            shader_path: "shaders/moon_shader.wgsl".to_string(),
            base_color: Color::rgb(0.5, 0.5, 0.5),
            roughness: 0.8,
            metallic: 0.0,
        }
    }
}

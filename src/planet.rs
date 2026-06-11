use crate::color::Color;
use crate::moon::Moon;
use crate::orbit::Orbit;

#[derive(Debug, Clone)]
pub struct Planet {
    pub name: String,
    pub speed: f32,
    pub mass: f32,
    pub radius: f32,
    pub orbit: Orbit,
    pub shader: PlanetShader,
    pub rotation_speed: f32,
    pub atmosphere: Option<Atmosphere>,
    pub temperature: f32,
    pub moons: Vec<Moon>,
}

impl Planet {
    pub fn new(name: impl Into<String>, mass: f32, radius: f32, orbit: Orbit) -> Self {
        Self {
            name: name.into(),
            speed: orbit.angular_speed,
            mass,
            radius,
            orbit,
            shader: PlanetShader::default(),
            rotation_speed: 0.0,
            atmosphere: None,
            temperature: 288.0,
            moons: Vec::new(),
        }
    }

    pub fn with_shader(mut self, shader: PlanetShader) -> Self {
        self.shader = shader;
        self
    }

    pub fn with_atmosphere(mut self, atmosphere: Atmosphere) -> Self {
        self.atmosphere = Some(atmosphere);
        self
    }

    pub fn with_moons(mut self, moons: Vec<Moon>) -> Self {
        self.moons = moons;
        self
    }

    pub fn without_atmosphere(mut self) -> Self {
        self.atmosphere = None;
        self
    }
}

#[derive(Debug, Clone)]
pub struct PlanetShader {
    pub shader_path: String,
    pub base_color: Color,
    pub roughness: f32,
    pub metallic: f32,
}

impl Default for PlanetShader {
    fn default() -> Self {
        Self {
            shader_path: "planet.wgsl".to_string(),
            base_color: Color::rgb(0.25, 0.45, 1.0),
            roughness: 0.85,
            metallic: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Atmosphere {
    pub color: Color,
    pub density: f32,
    pub radius_multiplier: f32,
}

impl Atmosphere {
    pub const fn new(color: Color, density: f32, radius_multiplier: f32) -> Self {
        Self {
            color,
            density,
            radius_multiplier,
        }
    }
}
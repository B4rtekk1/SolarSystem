#[derive(Debug, Clone, Copy)]
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

    pub fn position_at_angle(&self, angle: f32) -> [f32; 3] {
        let (sin_angle, cos_angle) = angle.sin_cos();
        let (sin_inclination, cos_inclination) = self.inclination.sin_cos();
        let x = self.semi_major_axis * cos_angle;
        let z = self.semi_minor_axis * sin_angle;

        [
            self.center[0] + x,
            self.center[1] - z * sin_inclination,
            self.center[2] + z * cos_inclination,
        ]
    }
}

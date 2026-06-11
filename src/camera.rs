use crate::{
    CameraUniform, DEFAULT_CAMERA_DISTANCE, MAX_CAMERA_DISTANCE, MIN_CAMERA_DISTANCE, ORBIT_SPEED,
    PAN_SPEED, ZOOM_SPEED,
};
use glam::{Mat4, Vec3};
use std::f32::consts::TAU;

pub struct Camera {
    yaw: f32,
    pitch: f32,
    distance: f32,
    target: Vec3,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: 0.45,
            pitch: 0.25,
            distance: DEFAULT_CAMERA_DISTANCE,
            target: Vec3::ZERO,
        }
    }
}

impl Camera {
    pub fn distance(&self) -> f32 {
        self.distance
    }

    pub fn orbit(&mut self, delta_x: f64, delta_y: f64) {
        self.yaw = (self.yaw + delta_x as f32 * ORBIT_SPEED).rem_euclid(TAU);
        self.pitch = (self.pitch + delta_y as f32 * ORBIT_SPEED).rem_euclid(TAU);
    }

    pub fn zoom(&mut self, scroll_delta: f32) {
        let zoom_factor = (1.0 - scroll_delta * ZOOM_SPEED).max(0.2);
        self.distance =
            (self.distance * zoom_factor).clamp(MIN_CAMERA_DISTANCE, MAX_CAMERA_DISTANCE);
    }

    pub fn pan(&mut self, delta_x: f64, delta_y: f64, viewport_height: u32) {
        let height = viewport_height.max(1) as f32;
        let units_per_pixel = 2.0 * self.distance * (45.0_f32.to_radians() * 0.5).tan() / height;
        let (yaw_sin, yaw_cos) = self.yaw.sin_cos();
        let (pitch_sin, pitch_cos) = self.pitch.sin_cos();
        let right = Vec3::new(yaw_cos, 0.0, -yaw_sin);
        let up = Vec3::new(-pitch_sin * yaw_sin, pitch_cos, -pitch_sin * yaw_cos);

        self.target +=
            (-right * delta_x as f32 + up * delta_y as f32) * units_per_pixel * PAN_SPEED;
    }

    pub fn view_projection(&self, width: u32, height: u32) -> CameraUniform {
        let aspect = width.max(1) as f32 / height.max(1) as f32;
        let (yaw_sin, yaw_cos) = self.yaw.sin_cos();
        let (pitch_sin, pitch_cos) = self.pitch.sin_cos();

        let eye = self.target
            + Vec3::new(
                self.distance * pitch_cos * yaw_sin,
                self.distance * pitch_sin,
                self.distance * pitch_cos * yaw_cos,
            );
        let up = Vec3::new(-pitch_sin * yaw_sin, pitch_cos, -pitch_sin * yaw_cos);

        let view = Mat4::look_at_rh(eye, self.target, up);
        let projection = Mat4::perspective_rh(45.0_f32.to_radians(), aspect, 0.1, 100.0);
        let view_projection = (projection * view).to_cols_array();
        let mut uniform = [0.0; 20];
        uniform[..16].copy_from_slice(&view_projection);
        uniform[16] = width.max(1) as f32;
        uniform[17] = height.max(1) as f32;
        uniform
    }
}

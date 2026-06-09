use crate::{CameraUniform, MAX_CAMERA_DISTANCE, MIN_CAMERA_DISTANCE, ORBIT_SPEED, ZOOM_SPEED};
use glam::{Mat4, Vec3};
use std::f32::consts::TAU;

pub struct Camera {
    yaw: f32,
    pitch: f32,
    distance: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: 0.45,
            pitch: 0.25,
            distance: 8.5,
        }
    }
}

impl Camera {
    pub fn orbit(&mut self, delta_x: f64, delta_y: f64) {
        self.yaw = (self.yaw + delta_x as f32 * ORBIT_SPEED).rem_euclid(TAU);
        self.pitch = (self.pitch + delta_y as f32 * ORBIT_SPEED).rem_euclid(TAU);
    }

    pub fn zoom(&mut self, scroll_delta: f32) {
        let zoom_factor = (1.0 - scroll_delta * ZOOM_SPEED).max(0.2);
        self.distance =
            (self.distance * zoom_factor).clamp(MIN_CAMERA_DISTANCE, MAX_CAMERA_DISTANCE);
    }

    pub fn view_projection(&self, width: u32, height: u32) -> CameraUniform {
        let aspect = width.max(1) as f32 / height.max(1) as f32;
        let (yaw_sin, yaw_cos) = self.yaw.sin_cos();
        let (pitch_sin, pitch_cos) = self.pitch.sin_cos();

        let eye = Vec3::new(
            self.distance * pitch_cos * yaw_sin,
            self.distance * pitch_sin,
            self.distance * pitch_cos * yaw_cos,
        );
        let up = Vec3::new(-pitch_sin * yaw_sin, pitch_cos, -pitch_sin * yaw_cos);

        let view = Mat4::look_at_rh(eye, Vec3::ZERO, up);
        let projection = Mat4::perspective_rh(45.0_f32.to_radians(), aspect, 0.1, 100.0);
        (projection * view).to_cols_array()
    }
}

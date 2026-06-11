use std::f32::consts::{PI, TAU};
use crate::pipeline::Vertex;

pub fn create_sphere(latitudes: u32, longitudes: u32, radius: f32) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::with_capacity(((latitudes + 1) * (longitudes + 1)) as usize);
    let mut indices = Vec::with_capacity((latitudes * longitudes * 6) as usize);
    let radius = radius.max(0.001);

    for lat in 0..=latitudes {
        let theta = lat as f32 / latitudes as f32 * PI;
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();

        for lon in 0..=longitudes {
            let phi = lon as f32 / longitudes as f32 * TAU;
            vertices.push([
                radius * sin_theta * phi.cos(),
                radius * cos_theta,
                radius * sin_theta * phi.sin(),
            ]);
        }
    }

    let stride = longitudes + 1;
    for lat in 0..latitudes {
        for lon in 0..longitudes {
            let top_left = lat * stride + lon;
            let top_right = top_left + 1;
            let bottom_left = top_left + stride;
            let bottom_right = bottom_left + 1;

            indices.extend_from_slice(&[
                top_left,
                bottom_left,
                top_right,
                top_right,
                bottom_left,
                bottom_right,
            ]);
        }
    }

    (vertices, indices)
}
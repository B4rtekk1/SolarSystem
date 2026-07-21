use crate::ecs::CelestialKind;

const STAR_PICK_RADIUS_SCALE: f32 = 1.08;
const BODY_PICK_RADIUS_SCALE: f32 = 1.45;
const MIN_BODY_PICK_RADIUS: f32 = 0.08;

pub fn pick_radius(kind: CelestialKind, render_radius: f32) -> f32 {
    match kind {
        CelestialKind::Star => render_radius * STAR_PICK_RADIUS_SCALE,
        CelestialKind::Planet | CelestialKind::Moon => {
            (render_radius * BODY_PICK_RADIUS_SCALE).max(MIN_BODY_PICK_RADIUS)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_pick_radius_tracks_visible_radius() {
        let render_radius = 0.45;
        let radius = pick_radius(CelestialKind::Star, render_radius);

        assert!(radius > render_radius);
        assert!(radius < render_radius * 1.1);
    }

    #[test]
    fn small_body_pick_radius_keeps_minimum_click_target() {
        assert_eq!(pick_radius(CelestialKind::Moon, 0.01), MIN_BODY_PICK_RADIUS);
    }
}

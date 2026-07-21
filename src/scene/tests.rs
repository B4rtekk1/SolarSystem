use super::*;

#[test]
fn scene_contains_the_eight_planets_and_major_moon_systems() {
    let world = create_world();
    assert_eq!(world.count_kind(CelestialKind::Star), 1);
    assert!(world.count_kind(CelestialKind::Planet) >= 13);
    assert!(world.count_kind(CelestialKind::Moon) >= 55);
}

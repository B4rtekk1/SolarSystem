use crate::{color::Color, orbit::Orbit};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Entity(usize);

impl Entity {
    #[cfg(test)]
    pub(crate) const fn from_index(index: usize) -> Self {
        Self(index)
    }

    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CelestialKind {
    Star,
    Planet,
    Moon,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameComponent {
    pub value: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ParentComponent {
    pub entity: Entity,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BodyComponent {
    pub mass: f32,
    pub radius_km: f32,
    pub render_radius: f32,
    pub orbit: Option<Orbit>,
}

impl BodyComponent {
    pub fn new(mass: f32, radius_km: f32, orbit: Option<Orbit>) -> Self {
        Self {
            mass,
            radius_km,
            render_radius: render_radius_from_km(radius_km),
            orbit,
        }
    }
}

fn render_radius_from_km(radius_km: f32) -> f32 {
    const EARTH_RADIUS_KM: f32 = 6_371.0;
    const MIN_RENDER_RADIUS: f32 = 0.018;
    const EARTH_RENDER_RADIUS: f32 = 0.08;
    const MAX_RENDER_RADIUS: f32 = 0.45;

    let earth_radii = (radius_km / EARTH_RADIUS_KM).max(0.0);
    (MIN_RENDER_RADIUS + earth_radii.sqrt() * EARTH_RENDER_RADIUS).min(MAX_RENDER_RADIUS)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RotationComponent {
    pub speed: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AtmosphereComponent {
    pub color: Color,
    pub density: f32,
    pub radius_multiplier: f32,
}

impl AtmosphereComponent {
    pub const fn new(color: Color, density: f32, radius_multiplier: f32) -> Self {
        Self {
            color,
            density,
            radius_multiplier,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RingComponent {
    pub inner_radius_multiplier: f32,
    pub outer_radius_multiplier: f32,
    pub tilt: f32,
    pub rotation_speed: f32,
    pub color: Color,
    pub particle_count: u32,
}

impl RingComponent {
    pub const fn new(
        inner_radius_multiplier: f32,
        outer_radius_multiplier: f32,
        tilt: f32,
        rotation_speed: f32,
        color: Color,
        particle_count: u32,
    ) -> Self {
        Self {
            inner_radius_multiplier,
            outer_radius_multiplier,
            tilt,
            rotation_speed,
            color,
            particle_count,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct StarMaterial {
    pub base_color: Color,
    pub accent_color: Color,
    pub brightness: f32,
    pub surface_temperature: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SurfaceMaterial {
    pub base_color: Color,
    pub accent_color: Color,
    pub roughness: f32,
    pub metallic: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MaterialComponent {
    Star(StarMaterial),
    Surface(SurfaceMaterial),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RenderComponent {
    pub material: MaterialComponent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectBundle {
    pub name: String,
    pub kind: CelestialKind,
    pub parent: Option<Entity>,
    pub body: BodyComponent,
    pub rotation: RotationComponent,
    pub render: RenderComponent,
    pub atmosphere: Option<AtmosphereComponent>,
    pub ring: Option<RingComponent>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct World {
    names: Vec<NameComponent>,
    kinds: Vec<CelestialKind>,
    parents: Vec<Option<ParentComponent>>,
    bodies: Vec<BodyComponent>,
    rotations: Vec<RotationComponent>,
    renders: Vec<RenderComponent>,
    atmospheres: Vec<Option<AtmosphereComponent>>,
    rings: Vec<Option<RingComponent>>,
}

impl World {
    pub fn spawn(&mut self, bundle: ObjectBundle) -> Entity {
        let entity = Entity(self.names.len());

        self.names.push(NameComponent { value: bundle.name });
        self.kinds.push(bundle.kind);
        self.parents
            .push(bundle.parent.map(|entity| ParentComponent { entity }));
        self.bodies.push(bundle.body);
        self.rotations.push(bundle.rotation);
        self.renders.push(bundle.render);
        self.atmospheres.push(bundle.atmosphere);
        self.rings.push(bundle.ring);

        entity
    }

    pub fn entity_capacity(&self) -> usize {
        self.kinds.len()
    }

    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        (0..self.kinds.len()).map(Entity)
    }

    pub fn entities_of_kind(&self, kind: CelestialKind) -> impl Iterator<Item = Entity> + '_ {
        self.entities()
            .filter(move |entity| self.kind(*entity) == kind)
    }

    pub fn children_of_kind(
        &self,
        parent: Entity,
        kind: CelestialKind,
    ) -> impl Iterator<Item = Entity> + '_ {
        self.entities().filter(move |entity| {
            self.kind(*entity) == kind && self.parent(*entity).is_some_and(|p| p.entity == parent)
        })
    }

    pub fn first_entity_of_kind(&self, kind: CelestialKind) -> Option<Entity> {
        self.entities_of_kind(kind).next()
    }

    pub fn count_kind(&self, kind: CelestialKind) -> usize {
        self.entities_of_kind(kind).count()
    }

    pub fn name(&self, entity: Entity) -> &str {
        &self.names[entity.index()].value
    }

    pub fn kind(&self, entity: Entity) -> CelestialKind {
        self.kinds[entity.index()]
    }

    pub fn parent(&self, entity: Entity) -> Option<ParentComponent> {
        self.parents[entity.index()]
    }

    pub fn body(&self, entity: Entity) -> BodyComponent {
        self.bodies[entity.index()]
    }

    pub fn rotation(&self, entity: Entity) -> RotationComponent {
        self.rotations[entity.index()]
    }

    pub fn render(&self, entity: Entity) -> RenderComponent {
        self.renders[entity.index()]
    }

    pub fn atmosphere(&self, entity: Entity) -> Option<AtmosphereComponent> {
        self.atmospheres[entity.index()]
    }

    pub fn atmosphere_mut(&mut self, entity: Entity) -> Option<&mut AtmosphereComponent> {
        self.atmospheres[entity.index()].as_mut()
    }

    pub fn ring(&self, entity: Entity) -> Option<RingComponent> {
        self.rings[entity.index()]
    }

    pub fn ring_mut(&mut self, entity: Entity) -> &mut Option<RingComponent> {
        &mut self.rings[entity.index()]
    }

    pub fn surface_material(&self, entity: Entity) -> Option<SurfaceMaterial> {
        match self.renders[entity.index()].material {
            MaterialComponent::Surface(material) => Some(material),
            MaterialComponent::Star(_) => None,
        }
    }

    pub fn surface_material_mut(&mut self, entity: Entity) -> Option<&mut SurfaceMaterial> {
        match &mut self.renders[entity.index()].material {
            MaterialComponent::Surface(material) => Some(material),
            MaterialComponent::Star(_) => None,
        }
    }

    pub fn star_material_mut(&mut self, entity: Entity) -> Option<&mut StarMaterial> {
        match &mut self.renders[entity.index()].material {
            MaterialComponent::Star(material) => Some(material),
            MaterialComponent::Surface(_) => None,
        }
    }

    pub fn set_name(&mut self, entity: Entity, name: String) {
        self.names[entity.index()].value = name;
    }
}

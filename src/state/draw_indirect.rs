use crate::ecs::{CelestialKind, Entity, World};

pub type IndexedIndirectArgs = [u32; 5];

#[derive(Clone, Copy)]
pub struct IndirectBatch {
    pub offset: wgpu::BufferAddress,
    pub instance_count: u32,
}

pub fn indexed_indirect_args(
    index_count: u32,
    instance_count: u32,
    first_instance: u32,
) -> IndexedIndirectArgs {
    [index_count, instance_count, 0, 0, first_instance]
}

pub fn entity_indirect_offset(entity: Entity) -> wgpu::BufferAddress {
    (entity.index() * size_of::<IndexedIndirectArgs>()) as wgpu::BufferAddress
}

pub fn append_kind_batches(
    world: &World,
    kind: CelestialKind,
    index_count: u32,
    commands: &mut Vec<IndexedIndirectArgs>,
) -> Vec<IndirectBatch> {
    let mut batches = Vec::new();
    let mut run_start = None;
    let mut run_count = 0_u32;

    for entity in world.entities() {
        if world.kind(entity) == kind {
            if run_start.is_none() {
                run_start = Some(entity.index() as u32);
            }
            run_count += 1;
        } else if let Some(first_instance) = run_start.take() {
            let offset = (commands.len() * size_of::<IndexedIndirectArgs>()) as wgpu::BufferAddress;
            commands.push(indexed_indirect_args(
                index_count,
                run_count,
                first_instance,
            ));
            batches.push(IndirectBatch {
                offset,
                instance_count: run_count,
            });
            run_count = 0;
        }
    }

    if let Some(first_instance) = run_start {
        let offset = (commands.len() * size_of::<IndexedIndirectArgs>()) as wgpu::BufferAddress;
        commands.push(indexed_indirect_args(
            index_count,
            run_count,
            first_instance,
        ));
        batches.push(IndirectBatch {
            offset,
            instance_count: run_count,
        });
    }

    batches
}

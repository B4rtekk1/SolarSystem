use crate::constants::{TEXT_OVERLAY_VERTEX_ATTRIBUTES, VERTEX_ATTRIBUTES};
use crate::render_utils::{
    alpha_blending_fragment_state, alpha_blending_fragment_targets, depth_stencil_state,
    replace_fragment_targets,
};

pub type TextOverlayVertex = [f32; 4];
pub type Vertex = [f32; 3];

pub fn create_sphere_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    sample_count: u32,
    label: &str,
) -> wgpu::RenderPipeline {
    create_sphere_pipeline_with_depth(
        device,
        format,
        layout,
        shader,
        sample_count,
        label,
        true,
        wgpu::CompareFunction::Less,
    )
}

pub fn create_sphere_overlay_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    sample_count: u32,
    label: &str,
) -> wgpu::RenderPipeline {
    create_sphere_pipeline_with_depth(
        device,
        format,
        layout,
        shader,
        sample_count,
        label,
        false,
        wgpu::CompareFunction::Always,
    )
}

pub fn create_sphere_replace_overlay_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    sample_count: u32,
    label: &str,
) -> wgpu::RenderPipeline {
    create_sphere_pipeline_with_options(
        device,
        format,
        layout,
        shader,
        sample_count,
        label,
        false,
        wgpu::CompareFunction::Always,
        &replace_fragment_targets(format),
    )
}

fn create_sphere_pipeline_with_options(
    device: &wgpu::Device,
    _format: wgpu::TextureFormat,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    sample_count: u32,
    label: &str,
    depth_write_enabled: bool,
    depth_compare: wgpu::CompareFunction,
    fragment_targets: &[Option<wgpu::ColorTargetState>],
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: size_of::<Vertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &VERTEX_ATTRIBUTES,
            }],
            compilation_options: Default::default(),
        },
        fragment: Some(alpha_blending_fragment_state(shader, fragment_targets)),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(depth_stencil_state(depth_write_enabled, depth_compare)),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            ..Default::default()
        },
        multiview_mask: None,
        cache: None,
    })
}

fn create_sphere_pipeline_with_depth(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    sample_count: u32,
    label: &str,
    depth_write_enabled: bool,
    depth_compare: wgpu::CompareFunction,
) -> wgpu::RenderPipeline {
    create_sphere_pipeline_with_options(
        device,
        format,
        layout,
        shader,
        sample_count,
        label,
        depth_write_enabled,
        depth_compare,
        &alpha_blending_fragment_targets(format),
    )
}

pub fn create_text_overlay_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    shader: &wgpu::ShaderModule,
    sample_count: u32,
    text_bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Text Overlay Pipeline Layout"),
        bind_group_layouts: &[Some(text_bind_group_layout)],
        immediate_size: 0,
    });

    let fragment_targets = alpha_blending_fragment_targets(format);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Text Overlay Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: size_of::<TextOverlayVertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &TEXT_OVERLAY_VERTEX_ATTRIBUTES,
            }],
            compilation_options: Default::default(),
        },
        fragment: Some(alpha_blending_fragment_state(shader, &fragment_targets)),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: Some(depth_stencil_state(false, wgpu::CompareFunction::Always)),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            ..Default::default()
        },
        multiview_mask: None,
        cache: None,
    })
}

pub fn create_screen_dim_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    shader: &wgpu::ShaderModule,
    sample_count: u32,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Screen Dim Pipeline Layout"),
        bind_group_layouts: &[],
        immediate_size: 0,
    });

    let fragment_targets = alpha_blending_fragment_targets(format);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Screen Dim Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(alpha_blending_fragment_state(shader, &fragment_targets)),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: Some(depth_stencil_state(false, wgpu::CompareFunction::Always)),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            ..Default::default()
        },
        multiview_mask: None,
        cache: None,
    })
}

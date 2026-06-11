use crate::constants::{FPS_FONT_SIZE, FPS_LINE_HEIGHT, FPS_TEXT_TEXTURE_HEIGHT, FPS_TEXT_TEXTURE_WIDTH, GOOGLE_SANS_BYTES, TEXT_OVERLAY_MAX_VERTICES};
use crate::pipeline::TextOverlayVertex;
use cosmic_text::{Attrs, Buffer as TextBuffer, Color as TextColor, Family, FontSystem, Metrics, Shaping, SwashCache};

use crate::constants::FPS_OVERLAY_MARGIN;

pub struct FpsOverlay {
    font_system: FontSystem,
    swash_cache: SwashCache,
    text_buffer: TextBuffer,
    font_family: String,
    text_texture: wgpu::Texture,
    pub text_bind_group: wgpu::BindGroup,
    pub text_vertex_count: u32,
    pub text_vertex_buffer: wgpu::Buffer,
    last_viewport_width: u32,
    last_viewport_height: u32,
    last_text: String,
}

impl FpsOverlay {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        text_bind_group_layout: &wgpu::BindGroupLayout,
        viewport_width: u32,
        viewport_height: u32,
    ) -> Self {
        let mut font_system = FontSystem::new();
        let font_family = {
            let db = font_system.db_mut();
            let existing_face_count = db.len();
            db.load_font_data(GOOGLE_SANS_BYTES.to_vec());
            let family = db
                .faces()
                .skip(existing_face_count)
                .next()
                .and_then(|face| face.families.first())
                .map_or_else(|| "Google Sans".to_string(), |(family, _)| family.clone());
            db.set_sans_serif_family(&family);
            family
        };
        let mut text_buffer = TextBuffer::new(
            &mut font_system,
            Metrics::new(FPS_FONT_SIZE, FPS_LINE_HEIGHT),
        );
        text_buffer.set_size(
            &mut font_system,
            Some(FPS_TEXT_TEXTURE_WIDTH as f32),
            Some(FPS_TEXT_TEXTURE_HEIGHT as f32),
        );

        let text_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("FPS Text Texture"),
            size: wgpu::Extent3d {
                width: FPS_TEXT_TEXTURE_WIDTH,
                height: FPS_TEXT_TEXTURE_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let text_texture_view = text_texture.create_view(&Default::default());
        let text_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("FPS Text Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let text_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("FPS Text Bind Group"),
            layout: text_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&text_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&text_sampler),
                },
            ],
        });
        let text_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("FPS Text Vertex Buffer"),
            size: (TEXT_OVERLAY_MAX_VERTICES * std::mem::size_of::<TextOverlayVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut overlay = Self {
            font_system,
            swash_cache: SwashCache::new(),
            text_buffer,
            font_family,
            text_texture,
            text_bind_group,
            text_vertex_count: 0,
            text_vertex_buffer,
            last_viewport_width: 0,
            last_viewport_height: 0,
            last_text: String::new(),
        };
        overlay.update(queue, 0.0, viewport_width, viewport_height);
        overlay
    }

    pub fn update(
        &mut self,
        queue: &wgpu::Queue,
        fps: f64,
        viewport_width: u32,
        viewport_height: u32,
    ) {
        let text = format!("FPS {:.0}", fps);
        if self.last_text != text {
            let pixels = rasterize_google_sans_text(
                &mut self.font_system,
                &mut self.swash_cache,
                &mut self.text_buffer,
                &self.font_family,
                &text,
            );
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.text_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(FPS_TEXT_TEXTURE_WIDTH * 4),
                    rows_per_image: Some(FPS_TEXT_TEXTURE_HEIGHT),
                },
                wgpu::Extent3d {
                    width: FPS_TEXT_TEXTURE_WIDTH,
                    height: FPS_TEXT_TEXTURE_HEIGHT,
                    depth_or_array_layers: 1,
                },
            );
            self.last_text = text;
        }

        if self.last_viewport_width != viewport_width
            || self.last_viewport_height != viewport_height
        {
            let text_vertices = build_fps_text_vertices(viewport_width, viewport_height);
            self.text_vertex_count = text_vertices.len() as u32;
            queue.write_buffer(
                &self.text_vertex_buffer,
                0,
                bytemuck::cast_slice(&text_vertices),
            );
            self.last_viewport_width = viewport_width;
            self.last_viewport_height = viewport_height;
        }
    }
}

fn rasterize_google_sans_text(
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    text_buffer: &mut TextBuffer,
    font_family: &str,
    text: &str,
) -> Vec<u8> {
    let mut pixels = vec![0; (FPS_TEXT_TEXTURE_WIDTH * FPS_TEXT_TEXTURE_HEIGHT * 4) as usize];
    let attrs = Attrs::new().family(Family::Name(font_family));
    text_buffer.set_text(font_system, text, &attrs, Shaping::Advanced, None);
    text_buffer.shape_until_scroll(font_system, true);
    text_buffer.draw(
        font_system,
        swash_cache,
        TextColor::rgba(210, 245, 255, 255),
        |x, y, width, height, color| {
            let [red, green, blue, alpha] = color.as_rgba();
            for row in 0..height as i32 {
                for column in 0..width as i32 {
                    let pixel_x = x + column;
                    let pixel_y = y + row;
                    if pixel_x < 0
                        || pixel_y < 0
                        || pixel_x >= FPS_TEXT_TEXTURE_WIDTH as i32
                        || pixel_y >= FPS_TEXT_TEXTURE_HEIGHT as i32
                    {
                        continue;
                    }

                    let index =
                        ((pixel_y as u32 * FPS_TEXT_TEXTURE_WIDTH + pixel_x as u32) * 4) as usize;
                    pixels[index] = red;
                    pixels[index + 1] = green;
                    pixels[index + 2] = blue;
                    pixels[index + 3] = alpha;
                }
            }
        },
    );

    pixels
}

fn build_fps_text_vertices(viewport_width: u32, viewport_height: u32) -> Vec<TextOverlayVertex> {
    let x = (viewport_width as f32 - FPS_TEXT_TEXTURE_WIDTH as f32 - FPS_OVERLAY_MARGIN).max(0.0);
    let y = FPS_OVERLAY_MARGIN.min(
        (viewport_height as f32 - FPS_TEXT_TEXTURE_HEIGHT as f32 - FPS_OVERLAY_MARGIN).max(0.0),
    );
    let mut vertices = Vec::with_capacity(TEXT_OVERLAY_MAX_VERTICES);
    push_textured_screen_rect(
        &mut vertices,
        x,
        y,
        FPS_TEXT_TEXTURE_WIDTH as f32,
        FPS_TEXT_TEXTURE_HEIGHT as f32,
        viewport_width,
        viewport_height,
    );
    vertices
}

fn push_textured_screen_rect(
    vertices: &mut Vec<TextOverlayVertex>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    viewport_width: u32,
    viewport_height: u32,
) {
    let left = screen_x_to_clip(x, viewport_width);
    let right = screen_x_to_clip(x + width, viewport_width);
    let top = screen_y_to_clip(y, viewport_height);
    let bottom = screen_y_to_clip(y + height, viewport_height);

    vertices.extend_from_slice(&[
        text_overlay_vertex(left, top, 0.0, 0.0),
        text_overlay_vertex(left, bottom, 0.0, 1.0),
        text_overlay_vertex(right, bottom, 1.0, 1.0),
        text_overlay_vertex(left, top, 0.0, 0.0),
        text_overlay_vertex(right, bottom, 1.0, 1.0),
        text_overlay_vertex(right, top, 1.0, 0.0),
    ]);
}

fn text_overlay_vertex(x: f32, y: f32, u: f32, v: f32) -> TextOverlayVertex {
    [x, y, u, v]
}

fn screen_x_to_clip(x: f32, viewport_width: u32) -> f32 {
    x / viewport_width.max(1) as f32 * 2.0 - 1.0
}

fn screen_y_to_clip(y: f32, viewport_height: u32) -> f32 {
    1.0 - y / viewport_height.max(1) as f32 * 2.0
}

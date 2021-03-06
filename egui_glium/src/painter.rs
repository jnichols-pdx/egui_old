#![allow(deprecated)] // legacy implement_vertex macro
#![allow(semicolon_in_expressions_from_macros)] // glium::program! macro

use {
    egui::{
        emath::Rect,
        epaint::{Color32, Mesh},
    },
    glium::{
        implement_vertex,
        index::PrimitiveType,
        program,
        texture::{self, srgb_texture2d::SrgbTexture2d},
        uniform,
        uniforms::{MagnifySamplerFilter, SamplerWrapFunction},
    },
    std::{collections::HashMap, rc::Rc},
};

pub struct Painter {
    program: glium::Program,
    egui_texture: Option<SrgbTexture2d>,
    egui_texture_version: Option<u64>,

    /// Index is the same as in [`egui::TextureId::User`].
    user_textures: HashMap<u64, Rc<SrgbTexture2d>>,

    #[cfg(feature = "epi")]
    next_native_tex_id: u64, // TODO: 128-bit texture space?
}

impl Painter {
    pub fn new(facade: &dyn glium::backend::Facade) -> Painter {
        let program = program! {
            facade,
            120 => {
                vertex: include_str!("shader/vertex_120.glsl"),
                fragment: include_str!("shader/fragment_120.glsl"),
            },
            140 => {
                vertex: include_str!("shader/vertex_140.glsl"),
                fragment: include_str!("shader/fragment_140.glsl"),
            },
            100 es => {
                vertex: include_str!("shader/vertex_100es.glsl"),
                fragment: include_str!("shader/fragment_100es.glsl"),
            },
            300 es => {
                vertex: include_str!("shader/vertex_300es.glsl"),
                fragment: include_str!("shader/fragment_300es.glsl"),
            },
        }
        .expect("Failed to compile shader");

        Painter {
            program,
            egui_texture: None,
            egui_texture_version: None,
            user_textures: Default::default(),
            #[cfg(feature = "epi")]
            next_native_tex_id: 1 << 32,
        }
    }

    pub fn upload_egui_texture(
        &mut self,
        facade: &dyn glium::backend::Facade,
        font_image: &egui::FontImage,
    ) {
        if self.egui_texture_version == Some(font_image.version) {
            return; // No change
        }

        let pixels: Vec<Vec<(u8, u8, u8, u8)>> = font_image
            .pixels
            .chunks(font_image.width as usize)
            .map(|row| {
                row.iter()
                    .map(|&a| Color32::from_white_alpha(a).to_tuple())
                    .collect()
            })
            .collect();

        let format = texture::SrgbFormat::U8U8U8U8;
        let mipmaps = texture::MipmapsOption::NoMipmap;
        self.egui_texture =
            Some(SrgbTexture2d::with_format(facade, pixels, format, mipmaps).unwrap());
        self.egui_texture_version = Some(font_image.version);
    }

    /// Main entry-point for painting a frame.
    /// You should call `target.clear_color(..)` before
    /// and `target.finish()` after this.
    pub fn paint_meshes<T: glium::Surface>(
        &mut self,
        display: &glium::Display,
        target: &mut T,
        pixels_per_point: f32,
        cipped_meshes: Vec<egui::ClippedMesh>,
        font_image: &egui::FontImage,
    ) {
        self.upload_egui_texture(display, font_image);

        for egui::ClippedMesh(clip_rect, mesh) in cipped_meshes {
            self.paint_mesh(target, display, pixels_per_point, clip_rect, &mesh);
        }
    }

    #[inline(never)] // Easier profiling
    fn paint_mesh<T: glium::Surface>(
        &mut self,
        target: &mut T,
        display: &glium::Display,
        pixels_per_point: f32,
        clip_rect: Rect,
        mesh: &Mesh,
    ) {
        debug_assert!(mesh.is_valid());

        let vertex_buffer = {
            #[derive(Copy, Clone)]
            struct Vertex {
                a_pos: [f32; 2],
                a_tc: [f32; 2],
                a_srgba: [u8; 4],
            }
            implement_vertex!(Vertex, a_pos, a_tc, a_srgba);

            let vertices: Vec<Vertex> = mesh
                .vertices
                .iter()
                .map(|v| Vertex {
                    a_pos: [v.pos.x, v.pos.y],
                    a_tc: [v.uv.x, v.uv.y],
                    a_srgba: v.color.to_array(),
                })
                .collect();

            // TODO: we should probably reuse the `VertexBuffer` instead of allocating a new one each frame.
            glium::VertexBuffer::new(display, &vertices).unwrap()
        };

        // TODO: we should probably reuse the `IndexBuffer` instead of allocating a new one each frame.
        let index_buffer =
            glium::IndexBuffer::new(display, PrimitiveType::TrianglesList, &mesh.indices).unwrap();

        let (width_in_pixels, height_in_pixels) = display.get_framebuffer_dimensions();
        let width_in_points = width_in_pixels as f32 / pixels_per_point;
        let height_in_points = height_in_pixels as f32 / pixels_per_point;

        if let Some(texture) = self.get_texture(mesh.texture_id) {
            // The texture coordinates for text are so that both nearest and linear should work with the egui font texture.
            // For user textures linear sampling is more likely to be the right choice.
            let filter = MagnifySamplerFilter::Linear;

            let uniforms = uniform! {
                u_screen_size: [width_in_points, height_in_points],
                u_sampler: texture.sampled().magnify_filter(filter).wrap_function(SamplerWrapFunction::Clamp),
            };

            // egui outputs colors with premultiplied alpha:
            let color_blend_func = glium::BlendingFunction::Addition {
                source: glium::LinearBlendingFactor::One,
                destination: glium::LinearBlendingFactor::OneMinusSourceAlpha,
            };

            // Less important, but this is technically the correct alpha blend function
            // when you want to make use of the framebuffer alpha (for screenshots, compositing, etc).
            let alpha_blend_func = glium::BlendingFunction::Addition {
                source: glium::LinearBlendingFactor::OneMinusDestinationAlpha,
                destination: glium::LinearBlendingFactor::One,
            };

            let blend = glium::Blend {
                color: color_blend_func,
                alpha: alpha_blend_func,
                ..Default::default()
            };

            // egui outputs mesh in both winding orders:
            let backface_culling = glium::BackfaceCullingMode::CullingDisabled;

            // Transform clip rect to physical pixels:
            let clip_min_x = pixels_per_point * clip_rect.min.x;
            let clip_min_y = pixels_per_point * clip_rect.min.y;
            let clip_max_x = pixels_per_point * clip_rect.max.x;
            let clip_max_y = pixels_per_point * clip_rect.max.y;

            // Make sure clip rect can fit within a `u32`:
            let clip_min_x = clip_min_x.clamp(0.0, width_in_pixels as f32);
            let clip_min_y = clip_min_y.clamp(0.0, height_in_pixels as f32);
            let clip_max_x = clip_max_x.clamp(clip_min_x, width_in_pixels as f32);
            let clip_max_y = clip_max_y.clamp(clip_min_y, height_in_pixels as f32);

            let clip_min_x = clip_min_x.round() as u32;
            let clip_min_y = clip_min_y.round() as u32;
            let clip_max_x = clip_max_x.round() as u32;
            let clip_max_y = clip_max_y.round() as u32;

            let params = glium::DrawParameters {
                blend,
                backface_culling,
                scissor: Some(glium::Rect {
                    left: clip_min_x,
                    bottom: height_in_pixels - clip_max_y,
                    width: clip_max_x - clip_min_x,
                    height: clip_max_y - clip_min_y,
                }),
                ..Default::default()
            };

            target
                .draw(
                    &vertex_buffer,
                    &index_buffer,
                    &self.program,
                    &uniforms,
                    &params,
                )
                .unwrap();
        }
    }

    // ------------------------------------------------------------------------

    #[cfg(feature = "epi")]
    pub fn set_texture(
        &mut self,
        facade: &dyn glium::backend::Facade,
        tex_id: u64,
        image: &epi::Image,
    ) {
        assert_eq!(
            image.size[0] * image.size[1],
            image.pixels.len(),
            "Mismatch between texture size and texel count"
        );

        let pixels: Vec<Vec<(u8, u8, u8, u8)>> = image
            .pixels
            .chunks(image.size[0] as usize)
            .map(|row| row.iter().map(|srgba| srgba.to_tuple()).collect())
            .collect();

        let format = texture::SrgbFormat::U8U8U8U8;
        let mipmaps = texture::MipmapsOption::NoMipmap;
        let gl_texture = SrgbTexture2d::with_format(facade, pixels, format, mipmaps).unwrap();

        self.user_textures.insert(tex_id, gl_texture.into());
    }

    pub fn free_texture(&mut self, tex_id: u64) {
        self.user_textures.remove(&tex_id);
    }

    fn get_texture(&self, texture_id: egui::TextureId) -> Option<&SrgbTexture2d> {
        match texture_id {
            egui::TextureId::Egui => self.egui_texture.as_ref(),
            egui::TextureId::User(id) => self.user_textures.get(&id).map(|rc| rc.as_ref()),
        }
    }
}

#[cfg(feature = "epi")]
impl epi::NativeTexture for Painter {
    type Texture = Rc<SrgbTexture2d>;

    fn register_native_texture(&mut self, native: Self::Texture) -> egui::TextureId {
        let id = self.next_native_tex_id;
        self.next_native_tex_id += 1;
        self.user_textures.insert(id, native);
        egui::TextureId::User(id as u64)
    }

    fn replace_native_texture(&mut self, id: egui::TextureId, replacing: Self::Texture) {
        if let egui::TextureId::User(id) = id {
            self.user_textures.insert(id, replacing);
        }
    }
}

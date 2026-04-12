#[macro_use]
mod base;
use base::*;

mod shared;
use shared::*;

mod slug;
use slug::{SlugAtlas, SlugString};

use std::str::FromStr;
use wasm_bindgen::prelude::*;

pub struct Output {
    pub index: &'static mut OutputIndex,
    pub messages: Vec<Message>,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub font_data: Vec<u8>,
}

impl Output {
    fn clear(&mut self) {
        self.messages.clear();
        self.vertices.clear();
        self.indices.clear();
        self.font_data.clear();
    }

    fn update_index(&mut self) -> *const OutputIndex {
        self.index.message_count = self.messages.len() as u32;
        self.index.messages_ptr = self.messages.as_ptr();
        self.index.indices_ptr = self.indices.as_ptr();
        self.index.vertices_ptr = self.vertices.as_ptr();
        self.index.font_data_ptr = self.font_data.as_ptr();
        self.index as *const OutputIndex
    }
}

#[wasm_bindgen]
pub struct RustSlugDemoInit {
    min_storage_alignment: u32,
}

#[wasm_bindgen]
impl RustSlugDemoInit {
    pub fn new() -> RustSlugDemoInit {
        RustSlugDemoInit {
            min_storage_alignment: 1,
        }
    }

    pub fn set_min_storage_alignment(&mut self, value: u32) {
        self.min_storage_alignment = value;
    }
}

pub struct StringWithFont {
    pub raw: String,
    pub slug: SlugString,
    pub font_size: f32,
    pub color: Rgba8,
    pub font_index: u32,
    pub update: bool,
}

pub struct RustSlugDemoFont {
    pub name: String,
    pub atlas: SlugAtlas,
    pub last_glyph_count: usize,
}

/// The WASM application state
#[wasm_bindgen]
pub struct RustSlugDemo {
    fonts: Vec<RustSlugDemoFont>,
    text: Vec<StringWithFont>,
    output: Output,
    min_storage_alignment: u32,
}

#[wasm_bindgen]
impl RustSlugDemo {

    pub fn initialize(init: RustSlugDemoInit) -> Option<Self> {
        let output_index = Box::leak(Box::default());
        let output = Output {
            index: output_index,
            messages: Vec::with_capacity(8),
            vertices: Vec::with_capacity(1000),
            indices: Vec::with_capacity(2000),
            font_data: Vec::with_capacity(8192)
        };

        let demo = RustSlugDemo {
            fonts: Vec::with_capacity(4),
            text:  Vec::with_capacity(4),
            output,
            min_storage_alignment: init.min_storage_alignment
        };

        Some(demo)
    }

    fn render_text(&mut self) {
        // Realistically, you could just stream the vertices, however no CPU deserves the torture of rasterizing the bee movie script every frame.
        let update_text = self.text.iter().any(|t| t.update );
        if !update_text {
            return;
        }

        // Tell typescript to invalidate all text instances
        self.output.messages.push(Message { 
            ty: MessageType::InvalidateText,
            data: MessageData { none: () }
        });

        // Generate glyph quads
        let mut index_count = 0;
        for text in self.text.iter_mut() {
            let text_indices_count = text.slug.indices_count() as u32;

            text.slug.write_mesh(
                0.0, 0.0,
                &mut self.output.indices,
                &mut self.output.vertices,
                text.color,
            );

            self.output.messages.push(Message { 
                ty: MessageType::DrawText,
                data: MessageData { draw_text: DrawTextParams { 
                    indices_count: text_indices_count,
                    first_index: index_count,
                    font_id: text.font_index as u32
                } }
            });

            index_count += text_indices_count;
            text.update = false;
        }

        // Update vertex buffer
        let indices_size = (self.output.indices.len() * size_of::<u32>()) as u32;
        let vertices_size = (self.output.vertices.len() * size_of::<Vertex>()) as u32;
        self.output.messages.push(Message { 
            ty: MessageType::UpdateVertexBuffer,
            data: MessageData {
                update_vertex: UpdateVertexParams {
                    indices_size,
                    vertices_size,
                    data_size: indices_size.next_multiple_of(4) + vertices_size
                }
            }
        });
    }

    // Update the font atlas if new glyphs were added by render_text
    fn update_font_atlas(&mut self) {
        for (font_id, font) in self.fonts.iter_mut().enumerate() {
            let processed_glyph_count = font.atlas.processed_glyph_count();
            if processed_glyph_count == font.last_glyph_count {
                continue;
            }

            let data_offset = self.output.font_data.len() as u32;
            let atlas_info = font.atlas.pack_into(&mut self.output.font_data, Some(self.min_storage_alignment));

            self.output.messages.push(Message { 
                ty: MessageType::UpdateFontAtlas,
                data: MessageData {
                    update_font_atlas: UpdateFontAtlasParams {
                        font_id: font_id as u32,
                        data_offset,
                        data_size: atlas_info.total_size,
                        curves_offset: atlas_info.curves_offset,
                        curves_size: atlas_info.curves_size,
                        curves_indices_offset: atlas_info.curves_indices_offset,
                        curves_indices_size: atlas_info.curves_indices_size,
                        glyphs_offset: atlas_info.glyphs_offset,
                        glyphs_size: atlas_info.glyphs_size
                    }
                }
            });

            font.last_glyph_count = processed_glyph_count;
        }
    }

    pub fn update(&mut self) -> *const OutputIndex {
        self.output.clear();
        self.render_text();
        self.update_font_atlas();
        self.output.update_index()
    }

    pub fn add_text(&mut self, raw: String, font_name: String, font_size: f32, color: u32) -> u32 {
        let (font_index, font) = 
        match self.fonts.iter_mut().enumerate().find(|(_, f)| f.name == font_name ) {
            Some((i, font)) => (i as u32, font),
            _ => {
                log!("No font named {font_name:?} in the app");
                return u32::MAX;
            }
        };

        let r = ((color >> 24) & 0xFF) as u8;
        let g = ((color >> 16) & 0xFF) as u8;
        let b = ((color >> 8) & 0xFF) as u8;
        let a = (color & 0xFF) as u8;

        let slug = font.atlas.build_slug_string(&raw, font_size);
        let text = StringWithFont {
            raw,
            slug,
            font_size,
            font_index,
            color: rgba8(r, g, b, a),
            update: true
        };

        self.text.push(text);

        (self.text.len() - 1) as u32
    }

    pub fn update_text_value(&mut self, text_id: u32, value: String) {
        let text = match self.text.get_mut(text_id as usize) {
            Some(text) => text,
            None => {
                log!("No text with ID {text_id} in the app");
                return;
            }
        };

        let font = match self.fonts.get_mut(text.font_index as usize) {
            Some(font) => font,
            _ => { return;  }
        };

        text.raw = value;
        text.slug = font.atlas.build_slug_string(&text.raw, text.font_size);
        text.update = true;
    }

    pub fn update_text_font(&mut self, text_id: u32, font: String) {
        let text = match self.text.get_mut(text_id as usize) {
            Some(text) => text,
            None => {
                log!("No text with ID {text_id} in the app");
                return;
            }
        };

       match self.fonts.iter_mut().enumerate().find(|(_, f)| f.name == font ) {
            Some((i, font)) => { 
                text.font_index = i as u32;
                text.update = true;
                text.slug = font.atlas.build_slug_string(&text.raw, text.font_size);
            },
            _ => {
                log!("No font named {font:?} in the app");
            }
        }
    }

    pub fn add_font(&mut self, name: String, data: Vec<u8>) {
        let font_data = data.leak();
        let atlas = SlugAtlas::from_static_slice(
            font_data,
            0,
            rustybuzz::Language::from_str("en").unwrap(),
            rustybuzz::Direction::LeftToRight,
            rustybuzz::script::LATIN
        );

        self.fonts.push(RustSlugDemoFont {
            name,
            atlas,
            last_glyph_count: 0,
        });
    }
    
    pub fn get_text_dimensions(&self, text_id: u32) -> Box<[f32]> {
        let size = self.text.get(text_id as usize)
            .map(|text| [text.slug.width, text.slug.height])
            .unwrap_or([0.0, 0.0]);

        Box::new(size)
    }

}
//! Data type shared between the rust WASM binary and javascript
//! All offsets/size in this file are in bytes

#[derive(Copy, Clone)]
#[repr(u32)]
pub enum MessageType {
    UpdateVertexBuffer = 0,
    UpdateFontAtlas = 1,
    DrawText = 2,
    InvalidateText = 3,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Vertex {
    pub pos: [f32; 4],
    pub data: [u32; 2],
    pub color: [u8; 4],
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct DrawTextParams {
    /// Number of indices to draw
    pub indices_count: u32,
    /// Offset to the first index in the indices buffer
    pub first_index: u32,
    /// Font ID
    pub font_id: u32,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct UpdateVertexParams {
    /// Total size of the indices and vertex. Aligned to 4 bytes
    pub data_size: u32,
    /// Size of the indices in the vertex data
    pub indices_size: u32,
    /// Size of the vertex
    pub vertices_size: u32,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct UpdateFontAtlasParams {
    /// Unique ID of the font
    pub font_id: u32,
    /// Offset of the vertex data in [shared_data_ptr]
    pub data_offset: u32,
    /// Total size of the font atlas
    pub data_size: u32,
    /// Offset of the curves data in the buffer (from data_offset)
    pub curves_offset: u32,
    /// Total size in bytes of the curves data
    pub curves_size: u32,
    /// Offset of the curves indices data in the buffer (from data_offset)
    pub curves_indices_offset: u32,
    /// Total size in bytes of the curves data
    pub curves_indices_size: u32,
    /// Offset of the glyphs data in the buffer (from data_offset)
    pub glyphs_offset: u32,
    /// Total size in bytes of the glyphs data
    pub glyphs_size: u32
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union MessageData {
    pub none: (),
    pub draw_text: DrawTextParams,
    pub update_vertex: UpdateVertexParams,
    pub update_font_atlas: UpdateFontAtlasParams,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Message {
    pub ty: MessageType,
    pub data: MessageData
}

/// Pointer size in wasm are 32 bit by default so the fields will be correctly aligned
#[repr(C)]
#[derive(Default)]
pub struct OutputIndex {
    pub message_count: u32,
    pub messages_ptr: *const Message,
    pub vertices_ptr: *const Vertex,
    pub indices_ptr: *const u32,
    pub font_data_ptr: *const u8,
}

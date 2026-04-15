use rustybuzz::ttf_parser;
use rustybuzz::{Face, ShapePlan, UnicodeBuffer};
use std::collections::HashMap;
use std::fmt::Debug;
use crate::base::{AABB, AABBi16, Point, Rgba8, line_is_significant, line_delta, point, aabb_i16, aabb};
use crate::shared::Vertex;

// Band count is also hardcoded in the shader
const BAND_COUNT: usize = 8;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct QuadCurve {
    pub p0: Point,
    pub p1: Point,
    pub p2: Point,
}

impl QuadCurve {
    fn line_to_quadratic(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        const LINE_EPSILON: f32 = 0.125;

        let mut mx = (x0 + x1) / 2.0;
        let mut my = (y0 + y1) / 2.0;
        let [dx, dy] = line_delta(x0, y0, x1, y1);

        if dx.abs() > 0.1 && dy.abs() > 0.1 {
            let length = f32::hypot(dx, dy);
            if length > 0.0 {
                let inv_length = LINE_EPSILON / length;
                mx -= dy * inv_length;
                my += dx * inv_length;
            }
        }

        QuadCurve { p0: point(x0, y0), p1: point(mx, my), p2: point(x1, y1) }
    }


    fn bounding_box(&self) -> [f32; 4] {
        let [x0, x1, x2] = [self.p0.x, self.p1.x, self.p2.x];
        let [y0, y1, y2] = [self.p0.y, self.p1.y, self.p2.y];

        let cxmin = x0.min(x1).min(x2);
        let cxmax = x0.max(x1).max(x2);
        let cymin = y0.min(y1).min(y2);
        let cymax = y0.max(y1).max(y2);

        [cxmin, cymin, cxmax, cymax]
    }

    pub fn max_x(&self) -> f32 {
        f32::max(self.p2.x, f32::max(self.p0.x, self.p1.x))
    }

    pub fn max_y(&self) -> f32 {
        f32::max(self.p2.y, f32::max(self.p0.y, self.p1.y))
    }
}

#[derive(Copy, Clone, Default)]
struct QuadCurveRanges {
    pub start: u32,
    pub end: u32,
}

impl QuadCurveRanges {
    pub fn as_range(&self) -> [usize; 2] {
        [self.start as usize, self.end as usize]
    }
}

impl Debug for QuadCurveRanges {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

/// A packed range. Storing the count in the high 8 bits and the base offset in the low 24 bits
/// Used to store the curves offsets for a specific band in [SlugGlyph]
#[derive(Default, Copy, Clone)]
struct PackedRange {
    raw: u32
}

impl PackedRange {
    pub fn new(offset: u32, count: u32) -> Self {
        PackedRange { 
            raw: (count << 24) | offset
        }
    }

    pub fn offset(&self) -> u32 { self.raw & 0xFFFFFF }
    pub fn count(&self) -> u32 { self.raw >> 24 }
    pub fn as_range(&self) -> [usize; 2] {
        let offset = self.offset() as usize;
        let count = self.count() as usize;
        [offset, offset + count]
    }
}

impl Debug for PackedRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [start, stop] = self.as_range();
        write!(f, "{}..{}", start, stop)
    }
}

pub struct FontFace {
    pub face: Face<'static>,
    pub plan: ShapePlan,
}

#[derive(Copy, Clone, Debug)]
pub struct SlugAtlasPackInfo {
    pub total_size: u32,
    pub curves_offset: u32,
    pub curves_size: u32,
    pub curves_indices_offset: u32,
    pub curves_indices_size: u32,
    pub glyphs_offset: u32,
    pub glyphs_size: u32,
}

/// Preprocessed [SlugGlyph] ready to be written into a mesh
#[derive(Copy, Clone, Debug)]
pub struct SlugStringGlyph {
    pub positions: AABB,
    pub em_positions: AABBi16,
    pub glyph_index: u32,
}

/// A processed slug string
#[derive(Default, Debug)]
pub struct SlugString {
    /// Total width of the string in pixel
    pub width: f32,
    /// Total height of the string in pixels
    pub height: f32,
    /// Preprocessed glyph
    pub glyphs: Vec<SlugStringGlyph>
}

impl SlugString {
    pub fn indices_count(&self) -> usize {
        self.glyphs.len() * 6
    }

    pub fn vertex_count(&self) -> usize {
        self.glyphs.len() * 4
    }

    /// Write the mesh of the text string into a vertex buffer
    /// using `vertices.len()` as vertex base for the indices.
    /// The mesh will start at `indices.len()`
    pub fn write_mesh(
        &self,
        x: f32,
        y: f32,
        indices: &mut Vec<u32>,
        vertices: &mut Vec<Vertex>,
        color: Rgba8
    ) {
        fn pack_i16(high: i16, low: i16) -> u32 {
            ((high as u16 as u32) << 16) | (low as u16 as u32)
        }

        let color = color.splat();
        let mut base_vertex = vertices.len() as u32;

        for glyph in self.glyphs.iter() {
            indices.extend_from_slice(&[
                base_vertex + 0, base_vertex + 2, base_vertex + 1, 
                base_vertex + 2, base_vertex + 1, base_vertex + 3
            ]);
            base_vertex += 4;

            let [xmin, ymin, xmax, ymax] = glyph.positions.splat();
            let [xmin_em, ymin_em, xmax_em, ymax_em] = glyph.em_positions.splat();

            vertices.extend_from_slice(&[
                Vertex { pos: [x+xmin, y+ymax, -1.0, -1.0], data: [pack_i16(xmin_em, ymin_em), glyph.glyph_index], color },
                Vertex { pos: [x+xmax, y+ymax,  1.0, -1.0], data: [pack_i16(xmax_em, ymin_em), glyph.glyph_index], color },
                Vertex { pos: [x+xmin, y+ymin, -1.0,  1.0], data: [pack_i16(xmin_em, ymax_em), glyph.glyph_index], color },
                Vertex { pos: [x+xmax, y+ymin,  1.0,  1.0], data: [pack_i16(xmax_em, ymax_em), glyph.glyph_index], color }
            ]);
        }
    }
}

// A single processed slug glyph
#[derive(Default, Copy, Clone, Debug)]
#[repr(C)]
struct SlugGlyph {
    pub bbox: AABBi16,
    pub curves: QuadCurveRanges,
    pub vertical_bands: [PackedRange; BAND_COUNT],
    pub horizontal_bands: [PackedRange; BAND_COUNT],
}

/// Extract the curves from a glyph, store the indices in `glyph` and store the curves in `curves`
struct SlugCurveExtractor<'a> {
    pub glyph: &'a mut SlugGlyph,
    pub curves: &'a mut Vec<QuadCurve>,
    pub start_x: f32,
    pub start_y: f32,
    pub x: f32,
    pub y: f32,
}

impl<'a> ttf_parser::OutlineBuilder for SlugCurveExtractor<'a> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.start_x = x;
        self.start_y = y;
        self.x = x;
        self.y = y;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        if line_is_significant(self.x, self.y, x, y) {
          self.curves.push(QuadCurve::line_to_quadratic(self.x, self.y, x, y));
          self.glyph.curves.end += 1;
        }
         
        self.x = x;
        self.y = y;
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.curves.push(QuadCurve { p0: point(self.x, self.y), p1: point(x1, y1), p2: point(x, y) });
        self.glyph.curves.end += 1;
        self.x = x;
        self.y = y;
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let m01x = (self.x + x1) / 2.0;
        let m01y = (self.y + y1) / 2.0;
        let m12x = (x1 + x2) / 2.0;
        let m12y = (y1 + y2) / 2.0;
        let m23x = (x2 + x) / 2.0;
        let m23y = (y2 + y) / 2.0;
        let m012x = (m01x + m12x) / 2.0;
        let m012y = (m01y + m12y) / 2.0;
        let m123x = (m12x + m23x) / 2.0;
        let m123y = (m12y + m23y) / 2.0;
        let midx = (m012x + m123x) / 2.0;
        let midy = (m012y + m123y) / 2.0;

        self.curves.push(QuadCurve {
            p0: point(self.x, self.y),
            p1: point(m01x, m01y),
            p2: point(midx, midy),
        });

        self.curves.push(QuadCurve {
            p0: point(midx, midy),
            p1: point(m123x, m123y),
            p2: point(x, y)
        });

        self.glyph.curves.end += 2;

        self.x = x;
        self.y = y;
    }

    fn close(&mut self) {
        if line_is_significant(self.x, self.y, self.start_x, self.start_y) {
            self.curves.push(QuadCurve::line_to_quadratic(self.x, self.y, self.start_x, self.start_y));
            self.glyph.curves.end += 1;
        }

        self.x = self.start_x;
        self.y = self.start_y;
    }
}

/// A builder that can be used to add one or more new glyphs to a [SlugAtlas] instance
struct SlugAtlasBuilder<'a> {
    pub data: &'a mut SlugAtlas,
    pub current_glyph: usize,
}

impl<'a> SlugAtlasBuilder<'a> {
    pub fn new(data: &'a mut SlugAtlas) -> Self {
        SlugAtlasBuilder { 
            data,
            current_glyph: usize::MAX,
        }
    }

    fn new_glyph(&mut self, glyph_id: u32) {
        self.current_glyph = self.data.glyphs.len();
        self.data.processed_glyph_map.insert(glyph_id,  self.current_glyph);
        
        let mut new_glyph = SlugGlyph::default();
        new_glyph.curves.start = self.data.curves.len() as u32;
        new_glyph.curves.end = new_glyph.curves.start;
        
        self.data.glyphs.push(new_glyph);
    }

    /// Extract the outline of a glyph into a list of quadratic curves
    fn build_glyph_curves(&mut self, glyph_id: u32) -> Option<()> {
        let glyph = &mut self.data.glyphs[self.current_glyph];
        let curves = &mut self.data.curves;
        let mut curve_extractor = SlugCurveExtractor {
            glyph,
            curves,
            start_x: 0.0, start_y: 0.0,
            x: 0.0, y: 0.0
        };

        let bbox = self.data.font.face.outline_glyph(ttf_parser::GlyphId(glyph_id as u16), &mut curve_extractor)?;
        
        glyph.bbox = aabb_i16(bbox.x_min, bbox.y_min, bbox.x_max, bbox.y_max);

        Some(())
    }

    /// Spatial partitioning optimization. Associate the curves extracted in build_glyph_curves into a list of `BAND_COUNT` horizontal and vertical bands
    fn build_glyph_bands(&mut self) {
        let current_glyph = &mut self.data.glyphs[self.current_glyph];
        let [xmin, ymin, xmax, ymax] = current_glyph.bbox.splat_f32();
        let width = xmax-xmin;
        let height = ymax-ymin;
        if width < 1.0 || height < 1.0 {
            return;
        }

        let band_count = BAND_COUNT as f32;
        let [curve_start, curve_end] = current_glyph.curves.as_range();

        // Compute the total number of curves in each band
        let mut hband_count = [0u16; BAND_COUNT];
        let mut vband_count = [0u16; BAND_COUNT];
        let mut total_curve_indices = 0usize;
        for curve_index in curve_start..curve_end {
            let curve = self.data.curves[curve_index];
            let [cxmin, cymin, cxmax, cymax] = curve.bounding_box();

            // Horizontal bands
            let b0 = ((cymin - ymin) / height * band_count).clamp(0.0, band_count-1.0) as usize;
            let b1 = ((cymax - ymin) / height * band_count).clamp(0.0, band_count-1.0) as usize;
            for band_index in b0..=b1 {
                hband_count[band_index] += 1;
                total_curve_indices += 1;
            }

            // Vertical bands
            let b0 = ((cxmin - xmin) / width * band_count).clamp(0.0, band_count-1.0) as usize;
            let b1 = ((cxmax - xmin) / width * band_count).clamp(0.0, band_count-1.0) as usize;
            for band_index in b0..=b1 {
                vband_count[band_index] += 1;
                total_curve_indices += 1;
            }
        }
        

        // Store indices offsets
        let mut curve_indices_offset = self.data.glyphs_curves_indices.len() as u32;
        for (i, &indices_count) in hband_count.iter().enumerate() {
            current_glyph.horizontal_bands[i] = PackedRange::new(curve_indices_offset, indices_count as u32);
            curve_indices_offset += indices_count as u32;
        }

        for (i, &indices_count) in vband_count.iter().enumerate() {
            current_glyph.vertical_bands[i] = PackedRange::new(curve_indices_offset, indices_count as u32);
            curve_indices_offset += indices_count as u32;
        }

        // Reserve indices space in data
        for _ in 0..total_curve_indices {
            self.data.glyphs_curves_indices.push(0);
        }

        // Tightly pack the curves indices in the slug data
        hband_count = [0u16; BAND_COUNT];
        vband_count = [0u16; BAND_COUNT];
        for curve_index in curve_start..curve_end {
            let curve = self.data.curves[curve_index];
            let [cxmin, cymin, cxmax, cymax] = curve.bounding_box();

            let b0 = ((cymin - ymin) / height * band_count).clamp(0.0, band_count-1.0) as usize;
            let b1 = ((cymax - ymin) / height * band_count).clamp(0.0, band_count-1.0) as usize;
            for band_index in b0..=b1 {
                let glyph_offset = current_glyph.horizontal_bands[band_index].offset() as usize;
                let local_offset = hband_count[band_index] as usize;
                self.data.glyphs_curves_indices[glyph_offset + local_offset] = curve_index as u32;
                hband_count[band_index] += 1;
            }

            let b0 = ((cxmin - xmin) / width * band_count).clamp(0.0, band_count-1.0) as usize;
            let b1 = ((cxmax - xmin) / width * band_count).clamp(0.0, band_count-1.0) as usize;
            for band_index in b0..=b1 {
                let glyph_offset = current_glyph.vertical_bands[band_index].offset() as usize;
                let local_offset = vband_count[band_index] as usize;
                self.data.glyphs_curves_indices[glyph_offset + local_offset] = curve_index as u32;
                vband_count[band_index] += 1;
            }
        }

        // Sort curves indices by x coordinates for horizontal bands and y coordinates for vertical bands
        // sort_unstable_by sorts by ascending order, but the curves must be sorted in descending order, so the Greater/Less is inverted
        for &offsets_range in current_glyph.horizontal_bands.iter() {
            let [start, end] = offsets_range.as_range();
            let curve_offsets = &mut self.data.glyphs_curves_indices[start..end];
            curve_offsets.sort_unstable_by(|&a, &b| {
                let curve1_max_x = self.data.curves[a as usize].max_x();
                let curve2_max_x = self.data.curves[b as usize].max_x();
                if curve1_max_x == curve2_max_x {
                    ::std::cmp::Ordering::Equal
                } else if curve1_max_x < curve2_max_x {
                    ::std::cmp::Ordering::Greater
                } else {
                    ::std::cmp::Ordering::Less
                }
            });
        }
    
        for &offsets_range in current_glyph.vertical_bands.iter() {
            let [start, end] = offsets_range.as_range();
            let curve_offsets = &mut self.data.glyphs_curves_indices[start..end];
            curve_offsets.sort_unstable_by(|&a, &b| {
                let curve1_max_y = self.data.curves[a as usize].max_y();
                let curve2_max_y = self.data.curves[b as usize].max_y();
                if curve1_max_y == curve2_max_y {
                    ::std::cmp::Ordering::Equal
                } else if  curve1_max_y < curve2_max_y {
                    ::std::cmp::Ordering::Greater
                } else {
                    ::std::cmp::Ordering::Less
                }
            });
        }
    }

    // Adds glyph from face into the slug atlas. Panics if the glyph already exist
    fn build_glyph(&mut self, glyph_id: u32) -> Option<(usize, SlugGlyph)> {
        assert!(!self.data.has_glyph(glyph_id), "Glyph is already in the atlas!");
        
        self.new_glyph(glyph_id);
        self.build_glyph_curves(glyph_id)?;
        self.build_glyph_bands();

        self.data.glyphs.last().map(|glyph| (self.current_glyph, *glyph))
    }
}


/// A collection of processed font glyphs for a font face
pub struct SlugAtlas {
    /// The font face associated with this atlas
    font: Box<FontFace>,
    /// Map of glyph id -> glyph index in `self.glyphs`
    processed_glyph_map: HashMap<u32, usize>,
    /// All processed glyph curves
    curves: Vec<QuadCurve>,
    /// Regrouped glyph curve index for the glyph bands
    glyphs_curves_indices: Vec<u32>,
    /// A list of processed glyph
    glyphs: Vec<SlugGlyph>,
    /// Reusable buffer when processing text
    text_buffer: Option<UnicodeBuffer>,
    /// 1 / face.units_per_em()
    scale: f32,
    ascender: f32,
    descender: f32,
}

impl SlugAtlas {

    pub fn from_static_slice(
        font_data: &'static [u8],
        face_index: u32,
        language: rustybuzz::Language,
        direction: rustybuzz::Direction,
        script: rustybuzz::Script,
    ) -> SlugAtlas {
        let face = rustybuzz::Face::from_slice(font_data, face_index).unwrap();
        let plan = rustybuzz::ShapePlan::new(&face, direction, Some(script), Some(&language), &[]);
        let scale = 1.0 / (face.units_per_em() as f32);
        let ascender = face.ascender() as f32;
        let descender = face.descender() as f32;
        // let line_gap = face.line_gap() as f32;

        SlugAtlas { 
            font: Box::new(FontFace { face, plan }),
            processed_glyph_map: HashMap::new(),
            curves: Vec::with_capacity(128),
            glyphs_curves_indices: Vec::with_capacity(512),
            glyphs: Vec::with_capacity(64),
            text_buffer: Some(UnicodeBuffer::new()),
            scale,
            ascender,
            descender,
        }
    }

    /// Copy the font atlas data into linear memory. Returns the offsets of the data in SlugAtlasPackInfo
    /// If `align_data` matches the minimum alignment of a GPU storage buffer, the whole buffer content can be used as-is by the slug shader.
    pub fn pack_into(&self, buffer: &mut Vec<u8>, align_data: Option<u32>) -> SlugAtlasPackInfo {
        let align = align_data.unwrap_or(1);

        let curves_offset = 0;
        let curves_size = (self.curves.len() * size_of::<QuadCurve>()) as u32;

        let curves_indices_offset = curves_size.next_multiple_of(align);
        let curves_indices_padding = curves_indices_offset - curves_size;
        let curves_indices_size = (self.glyphs_curves_indices.len() * size_of::<u32>()) as u32;

        let glyphs_offset = (curves_indices_offset + curves_indices_size).next_multiple_of(align);
        let glyphs_padding = glyphs_offset - (curves_indices_offset + curves_indices_size);
        let glyphs_size = (self.glyphs.len() * size_of::<SlugGlyph>()) as u32;

        let total_size = glyphs_offset + glyphs_size;

        let (_, curve_bytes, _) = unsafe { self.curves.align_to::<u8>() };
        buffer.extend_from_slice(&curve_bytes);

        for _ in 0..curves_indices_padding {
            buffer.push(0);
        }

        let (_, indices_bytes, _) = unsafe { self.glyphs_curves_indices.align_to::<u8>() };
        buffer.extend_from_slice(&indices_bytes);

        for _ in 0..glyphs_padding {
            buffer.push(0);
        }

        let (_, glyphs_bytes, _) = unsafe { self.glyphs.align_to::<u8>() };
        buffer.extend_from_slice(&glyphs_bytes);

        return SlugAtlasPackInfo {
            total_size,
            curves_offset,
            curves_size,
            curves_indices_offset,
            curves_indices_size,
            glyphs_offset,
            glyphs_size
        };
    }

    pub fn processed_glyph_count(&self) -> usize {
        self.processed_glyph_map.len()
    }

    fn has_glyph(&self, glyph_id: u32) -> bool {
        self.processed_glyph_map.contains_key(&glyph_id)
    }

    /// Returns the preprocessed glyph data for `glyph_id`. If glyph is not found in the face, it returns None
    fn glyph_data(&mut self, glyph_id: u32) -> Option<(usize, SlugGlyph)> {
        if let Some(&glyph_index) = self.processed_glyph_map.get(&glyph_id) {
            return self.glyphs.get(glyph_index).map(|glyph| (glyph_index, *glyph) );
        }

        // Process a new glyph and store it in the atlas and return the glyph data
        // If `glyph_id` is not in the font face, it returns None
        SlugAtlasBuilder::new(self).build_glyph(glyph_id)
    }

    /// Process a string of text into a [SlugString]
    /// Add any missing glyphs to the [SlugAtlas]
    pub fn build_slug_string(&mut self, value: &str, size: f32) -> SlugString {
        let mut text_buffer = self.text_buffer.take().unwrap();

        let mut glyphs = Vec::with_capacity(value.len());
        let mut width = 0.0;
        let mut height = 0.0;
        let mut x_advance = 0.0;
        let mut y_offset = 0.0;
        let scale = self.scale * size;
        let ascender = self.ascender * scale;
        let line_height = ascender + (-self.descender * scale);

        for line in value.split('\n') {
            text_buffer.push_str(line);
            let glyph_buffer = rustybuzz::shape_with_plan(&self.font.face, &self.font.plan, text_buffer);
            let iter_glyphs = glyph_buffer.glyph_infos().iter().map(|g| g.glyph_id)
                .zip(glyph_buffer.glyph_positions().iter());

            for (glyph_id, glyph_position) in iter_glyphs {
                let (glyph_index, glyph_data) = match self.glyph_data(glyph_id) {
                    Some(data) => data,
                    None => {
                        x_advance += (glyph_position.x_advance as f32) * scale;
                        continue;
                    }
                };

                let glyph_x_offset = (glyph_position.x_offset as f32 * scale) as f32;
                let glyph_y_offset = (glyph_position.y_offset as f32 * scale) as f32;
                let [xmin, ymin, xmax, ymax] = glyph_data.bbox.scale_splat_f32(scale);

                let top = -(ymin + glyph_y_offset) + ascender + y_offset;
                let right = x_advance + glyph_x_offset + xmax;
                let positions = aabb(
                    x_advance + glyph_x_offset + xmin,
                    -(ymax + glyph_y_offset) + ascender + y_offset,
                    right,
                    top
                );

                glyphs.push(SlugStringGlyph {
                    positions,
                    em_positions: glyph_data.bbox,
                    glyph_index: glyph_index as u32
                });

                width = f32::max(width, right);
                height = f32::max(height, top);

                x_advance += (glyph_position.x_advance as f32) * scale;
            }

            text_buffer = glyph_buffer.clear();
            y_offset += line_height;
            x_advance = 0.0;
        }

        self.text_buffer = Some(text_buffer);

        let string = SlugString {
            width,
            height,
            glyphs,
        };

        string
    }

}

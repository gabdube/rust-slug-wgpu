override SLUG_EVENODD: bool = false;
override SLUG_WEIGHT: bool = true;

struct GlobalParams {
    mvp: mat3x3<f32>, 
};

struct GlyphInfo {
    data: vec4<i32>,      // XY: [xmin, ymin, xmax, ymax] / ZW: curves offset, curves count (unused)
    vband: array<u32, 8>, // Curve indices range for vertical bands
    hband: array<u32, 8>, // Curve indices range for horizontal bands
};

@group(0) @binding(0) var<uniform> params: GlobalParams;

// The font curves. Each curve being 3 control point
@group(1) @binding(0) var<storage, read> curves: array<array<vec2<f32>, 3>>;

// The font curve indices
@group(1) @binding(1) var<storage, read> curve_indices: array<u32>;

// Glyph data
@group(1) @binding(2) var<storage, read> glyph_data: array<GlyphInfo>;

struct VertexInput {
    @location(0) pos: vec4<f32>,
    @location(1) glyph: vec2<u32>,
    @location(2) color: vec4<f32>,
};

struct VertexStruct {
    @builtin(position) positions: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) positions_em: vec2<f32>,
    @location(2) @interpolate(flat) glyph_index: u32,
};

struct FragmentOutput {
    @location(0) color: vec4<f32>,
}

@vertex
fn vertex_main(attrib: VertexInput) -> VertexStruct {
    var vertex: VertexStruct;
    
    // Vertex positions
    let transformed = params.mvp * vec3<f32>(attrib.pos.xy, 1.0);
    vertex.positions = vec4<f32>(transformed.x, transformed.y, 0.0, transformed.z);

    // Glyph texture coordinates
    let data0 = bitcast<i32>(attrib.glyph.x);
    let glyph_u = extractBits(data0, 16u, 16u);
    let glyph_v = extractBits(data0, 0u, 16u);

    vertex.color = attrib.color;
    vertex.positions_em = vec2(f32(glyph_u), f32(glyph_v));
    vertex.glyph_index = attrib.glyph.y;

    return vertex;
}


fn CalcRootCode(y1: f32, y2: f32, y3: f32) -> u32 {
    let i1 = bitcast<u32>(y1) >> 31u;
    let i2 = bitcast<u32>(y2) >> 30u;
    let i3 = bitcast<u32>(y3) >> 29u;
    let shift = (i3 & 4u) | (((i2 & 2u) | (i1 & ~2u)) & ~4u);
    return ((0x2E74u >> shift) & 0x0101u);
}

fn SolveHorizPoly(p1: vec2<f32>, p2: vec2<f32>, p3: vec2<f32>) -> vec2<f32> {
    let a = p1 - p2 * 2.0 + p3;
    let b = p1 - p2;
    let ra = 1.0 / a.y;
    let rb = 0.5 / b.y;

    let d = sqrt(max(b.y * b.y - a.y * p1.y, 0.0));
    var t1 = (b.y - d) * ra;
    var t2 = (b.y + d) * ra;

    if (abs(a.y) < 1.0 / 65536.0) {
        t1 = p1.y * rb;
        t2 = p1.y * rb;
    }

    return vec2<f32>((a.x * t1 - b.x * 2.0) * t1 + p1.x, (a.x * t2 - b.x * 2.0) * t2 + p1.x);
}

fn SolveVertPoly(p1: vec2<f32>, p2: vec2<f32>, p3: vec2<f32>) -> vec2<f32> {
    let a = p1 - p2 * 2.0 + p3;
    let b = p1 - p2;
    let ra = 1.0 / a.x;
    let rb = 0.5 / b.x;

    let d = sqrt(max(b.x * b.x - a.x * p1.x, 0.0));
    var t1 = (b.x - d) * ra;
    var t2 = (b.x + d) * ra;

    if (abs(a.x) < 1.0 / 65536.0) {
        t1 = p1.x * rb;
        t2 = p1.x * rb;
    }

    return vec2<f32>((a.y * t1 - b.y * 2.0) * t1 + p1.y, (a.y * t2 - b.y * 2.0) * t2 + p1.y);
}

fn CalcCoverage(xcov: f32, ycov: f32, xwgt: f32, ywgt: f32, flags: i32) -> f32 {
    var coverage = max(abs(xcov * xwgt + ycov * ywgt) / max(xwgt + ywgt, 1.0 / 65536.0), min(abs(xcov), abs(ycov)));

    if (SLUG_EVENODD) {
        if ((flags & 0x1000) == 0) {
            // Using nonzero fill rule here.

            coverage = saturate(coverage);
        } else {
            // Using even-odd fill rule here.

            coverage = 1.0 - abs(1.0 - fract(coverage * 0.5) * 2.0);
        }
    } else {
        // Using nonzero fill rule here.

        coverage = saturate(coverage);
    }

    // If SLUG_WEIGHT is defined during compilation, then take a square root to boost optical weight.

    if (SLUG_WEIGHT) {
        coverage = sqrt(coverage);
    }

    return coverage;
}


/// Returns the curve indice range from the glyph data at the current band
fn fetchCurveIndicesRange(positions_em: vec2f, glyph_index: u32) -> vec4<u32> {
    const band_count = 8;

    let p_glyph = &glyph_data[glyph_index];

    let data_x = (*p_glyph).data.x;
    let data_y = (*p_glyph).data.y;

    let bbox_min = vec2<f32>(
        f32(extractBits(data_x, 0u, 16u)), 
        f32(extractBits(data_x, 16u, 16u))
    );
    let bbox_max = vec2<f32>(
        f32(extractBits(data_y, 0u, 16u)), 
        f32(extractBits(data_y, 16u, 16u))
    );

    let size = max(bbox_max - bbox_min, vec2<f32>(0.0001));
    let band_index_f = (positions_em - bbox_min) * (band_count / size);
    let band_index = vec2<u32>(clamp(band_index_f, vec2<f32>(0.0), vec2<f32>(band_count-1)));

    let hband_indices = (*p_glyph).hband[band_index.y];
    let vband_indices = (*p_glyph).vband[band_index.x];

    let indices = vec2<u32>(hband_indices, vband_indices);
    let starts = indices & vec2<u32>(0xFFFFFFu);
    let counts = indices >> vec2<u32>(24u);

    return vec4<u32>(starts.x, counts.x, starts.y, counts.y);
}

fn SlugRender(render_coord: vec2<f32>, curve_indices_range: vec4<u32>) -> f32 {
    var curve_index_offset: u32;

    let ems_per_pixel = fwidth(render_coord);
    let pixels_per_em = 1.0 / ems_per_pixel;

    // Loop over all curves in the horizontal band
    var xcov: f32 = 0.0;
    var xwgt: f32 = 0.0;
    let hband_base: u32 = curve_indices_range.x;
    let hband_count: u32 = curve_indices_range.y;
    for (curve_index_offset = 0; curve_index_offset < hband_count; curve_index_offset++) {
        // Fetch the three 2D control points for the current curve from the curve texture.
        // Subtracting the render coordinates makes the curve relative to the sample position. 
        // The quadratic Bézier curveC(t) is given by
        //
        //     C(t) = (1 - t)^2 p1 + 2t(1 - t) p2 + t^2 p3
        let curve = curves[curve_indices[hband_base + curve_index_offset]];
        let p0 = curve[0] - render_coord;
        let p1 = curve[1] - render_coord;
        let p2 = curve[2] - render_coord;
        if (max(max(p0.x, p1.x), p2.x) * pixels_per_em.x < -0.5) {
            break;
        }

        let code = CalcRootCode(p0.y, p1.y, p2.y);
        if (code != 0u) {
            // At least one root makes a contribution. Calculate them and scale so
            // that the current pixel corresponds to the range [0,1].

            let r = SolveHorizPoly(p0, p1, p2) * pixels_per_em.y;

            // Bits in code tell which roots make a contribution.

            if ((code & 1u) != 0u) {
                xcov += saturate(r.x + 0.5);
                xwgt = max(xwgt, saturate(1.0 - abs(r.x) * 2.0));
            }

            if (code > 1u) {
                xcov -= saturate(r.y + 0.5);
                xwgt = max(xwgt, saturate(1.0 - abs(r.y) * 2.0));
            }
        }
    }

    // Loop over all curves in the vertical band.
    var ycov: f32 = 0.0;
    var ywgt: f32 = 0.0;
    let vband_base: u32 = curve_indices_range.z;
    let vband_count: u32 = curve_indices_range.w;
    for (curve_index_offset = 0; curve_index_offset < vband_count; curve_index_offset++) {
        let curve = curves[curve_indices[vband_base + curve_index_offset]];
        let p0 = curve[0] - render_coord;
        let p1 = curve[1] - render_coord;
        let p2 = curve[2] - render_coord;
        if (max(max(p0.y, p1.y), p2.y) * pixels_per_em.y < -0.5) {
            break;
        }

        let code = CalcRootCode(p0.x, p1.x, p2.x);
        if (code != 0u) {
            let r = SolveVertPoly(p0, p1, p2) * pixels_per_em.x;

            if ((code & 1u) != 0u) {
                ycov -= saturate(r.x + 0.5);
                ywgt = max(ywgt, saturate(1.0 - abs(r.x) * 2.0));
            }

            if (code > 1u) {
                ycov += saturate(r.y + 0.5);
                ywgt = max(ywgt, saturate(1.0 - abs(r.y) * 2.0));
            }
        }
    }

    return CalcCoverage(xcov, ycov, xwgt, ywgt, 0); // Glyph data is not used for now
}


@fragment
fn fragment_main(vertex: VertexStruct) -> FragmentOutput {
    var out: FragmentOutput;

    let curve_indices_range = fetchCurveIndicesRange(vertex.positions_em, vertex.glyph_index);
    let coverage = SlugRender(vertex.positions_em, curve_indices_range);

    out.color = coverage * vertex.color;

    return out;
}

use wasm_bindgen::prelude::*;

#[allow(unused_macros)]
macro_rules! log {
    ($($arg:tt)*) => {
        crate::base::log(format!("[LOG][{}:{}] {}", file!(), line!(), format!($($arg)*)))
    };
}

#[repr(C)]
#[derive(Default, Copy, Clone)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba8 {
    pub fn splat(&self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone, Debug)]
pub struct AABB {
  pub xmin: f32,
  pub ymin: f32,
  pub xmax: f32,
  pub ymax: f32
}

impl AABB {
    #[inline(always)]
    pub fn splat(self) -> [f32; 4] {
        [self.xmin, self.ymin, self.xmax, self.ymax]
    }
}

/// i16 is usefull to store Em-Space coordinates
#[repr(C)]
#[derive(Default, Copy, Clone, Debug)]
pub struct AABBi16 {
  pub xmin: i16,
  pub ymin: i16,
  pub xmax: i16,
  pub ymax: i16
}

impl AABBi16 {
    #[inline(always)]
    pub fn splat(self) -> [i16; 4] {
        [self.xmin, self.ymin, self.xmax, self.ymax]
    }

    pub fn width(&self) -> i16 {
        self.xmax - self.xmin
    }

    pub fn height(&self) -> i16 {
        self.ymax - self.ymin
    }
    
    #[inline(always)]
    pub fn splat_f32(self) -> [f32; 4] {
        [self.xmin as f32, self.ymin as f32, self.xmax as f32, self.ymax as f32]
    }

    #[inline(always)]
    pub fn scale_splat_f32(self, scale: f32) -> [f32; 4] {
        [self.xmin as f32 * scale, self.ymin as f32 * scale, self.xmax as f32 * scale, self.ymax as f32 * scale]
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[inline(always)]
pub fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

#[inline(always)]
pub fn rgba8(r: u8, g: u8, b: u8, a: u8) -> Rgba8 {
    Rgba8{ r, g, b, a }
}

#[inline(always)]
pub fn aabb(xmin: f32, ymin: f32, xmax: f32, ymax: f32) -> AABB {
    AABB { xmin, ymin, xmax, ymax }
}

#[inline(always)]
pub fn aabb_i16(xmin: i16, ymin: i16, xmax: i16, ymax: i16) -> AABBi16 {
    AABBi16 { xmin, ymin, xmax, ymax }
}

#[inline(always)]
pub fn line_delta(x0: f32, y0: f32, x1: f32, y1: f32) -> [f32; 2] {
    [x1-x0, y1-y0]
}

#[inline(always)]
pub fn line_is_significant(x0: f32, y0: f32, x1: f32, y1: f32) -> bool {
    let [dx, dy] = line_delta(x0, y0, x1, y1);
    dx.abs() >= 0.1 || dy.abs() >= 0.1
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: String);
}



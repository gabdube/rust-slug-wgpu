# A font rendering demo using Rust, WebGPU, and the Slug Algorithm

Try the demo without cloning this repo here: https://gabdube.github.io/articles/rust_slug/rust_slug.html

## Overview

A demo showing a WebGPU implementation of the Slug algorithm, originally created by Eric Lengyel. Text preprocessing is done in Rust.

This is a demo, not a rust "crate". The code is not production ready. The preprocessing logic is in `slug.rs`, feel free to build from it.

There are two things that differ from the reference implementation:

* The original HLSL shader uses textures to share the font data with the shader, my version uses storage buffers.
* I did not port the dynamic dilation in the vertex shader

## Demo Controls

- **Mouse drag** - Pan the view horizontally and vertically
- **Mouse wheel** - Zoom in and out
- **Left/Right arrow keys** - Cycle through loaded fonts and apply to all text
- **Drag and drop .ttf/.otf files** - Load new fonts into the demo
- **Drag and drop .txt files** - Update the displayed text content
- **Mouse interaction** - Pauses the automatic text scrolling animation

## File Structure

### Rust Source

- **`rust-slug/src/lib.rs`** - Main WASM library entry point with the `RustSlugDemo` public API for text rendering and font management.
- **`rust-slug/src/slug.rs`** - Slug algorithm implementation including glyph atlas building, curve extraction, and text string processing.
- **`rust-slug/src/base.rs`** - Base utilities and data structures (AABB, Point, color types, matrix operations).
- **`rust-slug/src/shared.rs`** - Shared message types and data structures for communication between Rust and TypeScript.
- **`local-server/src/main.rs`** - Local development server for serving the demo application during development.

### TypeScript Source

- **`rust-slug-demo/demo.ts`** - Main demo application with WebGPU renderer and user controls.
- **`rust-slug-demo/generated.ts`** - Generated TypeScript bindings for WASM message types and serialization functions.

## Running it locally

Because the wasm binaries and assets are included in the repo, the first two steps are optional.

1. Build the rust wasm binary

```
cd rust-slug
wasm-pack build --out-dir ./build --target web
cp ./build/rust_slug.js ../build/
cp ./build/rust_slug_bg.wasm ../build/
```

2. Build typescript source

```
npm install
npx rollup --config rollup.config.mjs --watch
```

3. Start the local server

```
cargo run --release -p local-server
```


### Acknowledgements
Thanks to Eric Lengyel for creating the Slug algorithm and releasing it into the public domain.

### Sources
- https://github.com/EricLengyel/Slug
- https://terathon.com/blog/decade-slug.html

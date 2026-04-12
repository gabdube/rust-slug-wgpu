import type { RustSlugDemo, RustSlugDemoInit, InitOutput } from "../rust-slug/build/rust_slug";
import { MessageType, read_output_messages, read_indices, read_vertices, read_font_data } from "./generated";

const WASM_PATH = "./rust_slug.js";
const INITIAL_TEXT_SIZE = 50.0;

interface RustSlugModule {
  RustSlugDemo: typeof RustSlugDemo;
  RustSlugDemoInit: typeof RustSlugDemoInit;
  default(): Promise<InitOutput>;
}

interface RustSlugClient {
  module: RustSlugModule;
  memory: WebAssembly.Memory;
  instance: RustSlugDemo;
}

interface Assets {
  fonts: { name: string, path: string }[];
}

interface FontBuffer {
  buffer: GPUBuffer;
  buffer_size: number;
  bind_group: GPUBindGroup;
}

interface GpuResources {
  vertex: GPUBuffer;
  vertex_buffer_total_size: number;
  vertices_offset: number;
  vertices_size: number;
  indices_offset: number;
  indices_size: number;

  uniforms: GPUBuffer;
  uniforms_size: number;
  uniforms_mvp_offset: number;
  globals: Float32Array,

  fonts: Map<number, FontBuffer>;
  text_instances: DrawText[];

  pipeline: GPURenderPipeline;
  global_bindgroup: GPUBindGroup;
  font_group_layout: GPUBindGroupLayout,

  min_storage_buffer_offset_alignment: number
}

interface DrawText {
  indices_count: number;
  first_index: number;
  font_id: number,
}

interface UserText {
  value: string,
  font: string,
  size: number,
  id: number|null
}

interface DemoUserState {
  offset_x: number,
  offset_y: number,
  view_width: number,
  view_height: number,
  zoom: number,
  text_instances: UserText[],
  fonts: string[],
  default_font: number,
  animate: boolean,
}

interface DemoStats {
  vertex_buffer_total_size: number,
  vertices_size: number,
  indices_size: number,
  fonts_size: {total_size: number, glyphs: number, curves: number, curve_indices: number}[],
  last_text_processing_time: number,
}

interface Demo {
  canvas: HTMLCanvasElement;
  device: GPUDevice;
  ctx: GPUCanvasContext;
  client: RustSlugClient;
  resources: GpuResources;
  user_state: DemoUserState,
  assets: Assets,
  stats: DemoStats,
  reload: boolean,
}

function align(value: number, n: number): number {
  return (value + n - 1) & ~(n - 1);
}

function computeMvp(user_state: DemoUserState, out: Float32Array) {
  const scaleX = 2.0 / user_state.view_width;
  const scaleY = -2.0 / user_state.view_height;
  const translateX = (user_state.offset_x * scaleX) - 1.0;
  const translateY = (user_state.offset_y * scaleY) + 1.0;
  out[0] = scaleX * user_state.zoom;
  out[5] = scaleY * user_state.zoom;
  out[8] = translateX;
  out[9] = translateY;
  out[10] = 1.0;
}

function updateMvp(demo: Demo) {
  const rc = demo.resources;
  computeMvp(demo.user_state, rc.globals);
  demo.device.queue.writeBuffer(rc.uniforms, rc.uniforms_mvp_offset, rc.globals.buffer, 0, rc.globals.byteLength);
}

function updateTextInstanceFonts(demo: Demo) {
  const state = demo.user_state;
  const fontName = state.fonts[state.default_font];
  if (!fontName) return;

  for (const text_instance of state.text_instances) {
    if (text_instance.id === null) continue;
    text_instance.font = fontName;
    demo.client.instance.update_text_font(text_instance.id, fontName);
  }
}

function resize(demo: Demo) {
  const width = document.body.clientWidth * devicePixelRatio;
  const height = document.body.clientHeight * devicePixelRatio;
  if (demo.canvas.width !== width || demo.canvas.height !== height) {
    demo.canvas.width = width;
    demo.canvas.height = height;
    demo.user_state.view_width = width;
    demo.user_state.view_height = height;    
    updateMvp(demo);
  }
}

function update(demo: Demo) {
  const output_index_ptr = demo.client.instance.update();
  
  
  for (const msg of read_output_messages(demo.client.memory.buffer, output_index_ptr)) {
    switch (msg.ty) {
      case MessageType.InvalidateText: {
        demo.resources.text_instances = [];
        break;
      }
      case MessageType.DrawText: {
        const data = msg.draw_text;
        demo.resources.text_instances.push({
          indices_count: data.indices_count,
          first_index: data.first_index,
          font_id: data.font_id,
        });
        break;
      }
      case MessageType.UpdateFontAtlas: {
        const data = msg.update_font_atlas;

        console.log(`Font buffer for Font(${data.font_id}) updated!`);
        console.log("Total size: ", data.data_size);
        console.log("Glyph size: ", data.glyphs_size);
        console.log("Curves size: ", data.curves_size);
        console.log("Curves indices size: ", data.curves_indices_size);
        demo.stats.fonts_size[data.font_id] = {
          total_size: data.data_size,
          glyphs: data.glyphs_size,
          curves: data.curves_size,
          curve_indices: data.curves_indices_size,
        };
        
        const font = demo.resources.fonts.get(data.font_id);
        let buffer: GPUBuffer;
        let buffer_size: number;
        if (font === undefined || font.buffer_size < data.data_size) {
          if (font?.buffer) {
            font.buffer.destroy();
          }

          buffer_size = align(data.data_size, 8192);
          buffer = demo.device.createBuffer({
            size: buffer_size,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
          });
        } else {
          buffer_size = font.buffer_size;
          buffer = font.buffer;
        } 

        const font_data = read_font_data(demo.client.memory.buffer, output_index_ptr, data.data_offset, data.data_size);
        demo.device.queue.writeBuffer(buffer, 0, font_data.buffer, font_data.byteOffset, data.data_size);

        const bind_group = demo.device.createBindGroup({
          layout: demo.resources.font_group_layout,
          entries: [
            { binding: 0, resource: { buffer, offset: data.curves_offset, size: data.curves_size } },
            { binding: 1, resource: { buffer, offset: data.curves_indices_offset, size: data.curves_indices_size } },
            { binding: 2, resource: { buffer, offset: data.glyphs_offset, size: data.glyphs_size } },
          ],
        });

        demo.resources.fonts.set(data.font_id, {
          buffer,
          bind_group,
          buffer_size,
        });
       
        break;
      }
      case MessageType.UpdateVertexBuffer: {
        const rc = demo.resources;
        const data = msg.update_vertex;

        console.log("Text buffer updated!");
        console.log("Total size: ", data.data_size);
        console.log("Vertices size: ", data.vertices_size);
        console.log("Indices size: ", data.indices_size);
        demo.stats.vertex_buffer_total_size = data.data_size;
        demo.stats.vertices_size = data.vertices_size;
        demo.stats.indices_size = data.indices_size;

        // Realloc vertex buffer if capacity was busted
        if (data.data_size > demo.resources.vertex_buffer_total_size) {
          rc.vertex.destroy();
          rc.vertex_buffer_total_size = align(data.data_size, 8192);
          rc.vertex = demo.device.createBuffer({
            size: rc.vertex_buffer_total_size,
            usage: GPUBufferUsage.VERTEX | GPUBufferUsage.INDEX | GPUBufferUsage.COPY_DST,
          });
          rc.indices_offset = 0;
          rc.indices_size = 0;
          rc.vertices_offset = 0;
          rc.vertices_size = 0;
        }

        rc.indices_offset = 0;
        rc.indices_size = data.indices_size;
        rc.vertices_offset = align(rc.indices_offset + rc.indices_size, 4);
        rc.vertices_size = data.vertices_size;

        const indices_data = read_indices(demo.client.memory.buffer, output_index_ptr, 0, data.indices_size);
        demo.device.queue.writeBuffer(rc.vertex, rc.indices_offset, indices_data.buffer, indices_data.byteOffset, rc.indices_size);

        const vertices_data = read_vertices(demo.client.memory.buffer, output_index_ptr, 0, data.vertices_size);
        demo.device.queue.writeBuffer(rc.vertex, rc.vertices_offset, vertices_data.buffer, vertices_data.byteOffset, rc.vertices_size);

        break;
      }
    }
  }
}

function render(demo: Demo) {
  const device = demo.device;

  const encoder = device.createCommandEncoder();
  const pass = encoder.beginRenderPass({
    colorAttachments: [
      {
        view: demo.ctx.getCurrentTexture().createView(),
        clearValue: { r: 15.0/255.0, g: 15.0/255.0, b: 15.0/255.0, a: 1 },
        loadOp: "clear",
        storeOp: "store",
      },
    ],
  });

  for (const text of demo.resources.text_instances) {
    const { vertex, vertices_offset, vertices_size, indices_offset, indices_size, pipeline, global_bindgroup } = demo.resources;
    const font_bindgroup = demo.resources.fonts.get(text.font_id)?.bind_group;
    if (!font_bindgroup) {
      console.error(`Font id ${text.font_id} not found`);
      continue;
    }

    pass.setPipeline(pipeline);
    pass.setBindGroup(0, global_bindgroup);
    pass.setBindGroup(1, font_bindgroup);
    pass.setVertexBuffer(0, vertex, vertices_offset, vertices_size);
    pass.setIndexBuffer(vertex, "uint32", indices_offset, indices_size);
    pass.drawIndexed(text.indices_count, 1, text.first_index);
  }

  pass.end();
  device.queue.submit([encoder.finish()]);
}

async function reload(demo: Demo) {
  demo.user_state = await initial_user_state();
  demo.client.instance.free();
  demo.client = await loadDemo(demo.assets, demo.resources, demo.user_state, demo.stats);
  demo.reload = false;
}

let boundedRun = () => {};
function run(demo: Demo) {
  if (demo.user_state.animate) {
    demo.user_state.offset_y -= 1.0;
    updateMvp(demo);
  }

  resize(demo);
  update(demo);
  render(demo);

  if (demo.reload) {
    reload(demo)
      .then(() => requestAnimationFrame(boundedRun) );
  } else {
    requestAnimationFrame(boundedRun);
  }
}

//
// Init
//

async function initGpuResources(device: GPUDevice, target_format: GPUTextureFormat): Promise<GpuResources> {
  const vertex_buffer_total_size = 1024 * 1024 * 10; // 10 MB is enough to fit the whole (rasterized) bee movie script
  const vertex = device.createBuffer({
    size: vertex_buffer_total_size,
    usage: GPUBufferUsage.VERTEX | GPUBufferUsage.INDEX | GPUBufferUsage.COPY_DST,
  })

  
  const globals = new Float32Array(16);
  const uniforms_size = globals.byteLength;
  const uniforms_mvp_offset = 0;
  const uniforms = device.createBuffer({
    size: uniforms_size,
    usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
  });

  // Global data bind layout @group(0)
  const global_group_layout = device.createBindGroupLayout({
    entries: [
      { binding: 0, visibility: GPUShaderStage.VERTEX, buffer: { type: "uniform" } },
    ],
  });

  // Font data bind layout @group(1)
  const font_group_layout = device.createBindGroupLayout({
    entries: [
      { binding: 0, visibility: GPUShaderStage.FRAGMENT, buffer: { type: "read-only-storage" } }, // Curves
      { binding: 1, visibility: GPUShaderStage.FRAGMENT, buffer: { type: "read-only-storage" } }, // Curves indices
      { binding: 2, visibility: GPUShaderStage.FRAGMENT, buffer: { type: "read-only-storage" } }, // Font glyph
    ],
  });

  const global_bindgroup = device.createBindGroup({
    layout: global_group_layout,
    entries: [
      { binding: 0, resource: { buffer: uniforms } },
    ],
  });

  const shader_source = await (await fetch(`slug.wgsl`)).text();
  const shader_module = device.createShaderModule({ code: shader_source });
  const pipeline = device.createRenderPipeline({
    layout: device.createPipelineLayout({ bindGroupLayouts: [global_group_layout, font_group_layout] }),
    vertex: {
      module: shader_module,
      entryPoint: "vertex_main",
      buffers: [
        {
          arrayStride: 28,
          attributes: [
            { shaderLocation: 0, offset: 0, format: "float32x4" }, // position & normals
            { shaderLocation: 1, offset: 16, format: "uint32x2" }, // packed glyph data
            { shaderLocation: 2, offset: 24, format: "unorm8x4" }, // color
          ],
        },
      ],
    },
    fragment: {
      module: shader_module,
      entryPoint: "fragment_main",
      targets: [
        {
          format: target_format,
          blend: {
            color: { srcFactor: "src-alpha", dstFactor: "one-minus-src-alpha" },
            alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha" },
          },
        },
      ],
    },
    primitive: { topology: "triangle-list" },
  });

  return {
    vertex,
    vertex_buffer_total_size,
    vertices_offset: 0,
    vertices_size: 0,
    indices_offset: 0,
    indices_size: 0,

    uniforms,
    uniforms_size,
    uniforms_mvp_offset,
    globals,

    fonts: new Map(), // Font buffers are lazily created in `update`
    text_instances: [],

    pipeline,
    global_bindgroup,
    font_group_layout,

    // (adapter.limits.minStorageBufferOffsetAlignment cannot be trusted)
    // My firefox reports 32, but then crashes telling the actual min offset alignment is 256.
    min_storage_buffer_offset_alignment: 256, 
  };
}

async function loadDemo(assets: Assets, resources: GpuResources, user_state: DemoUserState, stats: DemoStats): Promise<RustSlugClient> {
  const module = await import(WASM_PATH) as RustSlugModule;
  const init_output = await module.default() as InitOutput;
  const memory = init_output.memory;

  const init = module.RustSlugDemoInit.new();
  init.set_min_storage_alignment(resources.min_storage_buffer_offset_alignment);
 
  const instance = module.RustSlugDemo.initialize(init);
  if (!instance) {
    throw "Failed to initialize rust-slug instance";
  }

  const fontData = await Promise.all(assets.fonts.map(async (font) => (
    {name: font.name, data: (await (await fetch(font.path)).arrayBuffer()) }
  )));
  
  if (fontData) {
    for (const font of fontData) {
      instance.add_font(font.name, new Uint8Array(font.data));
      user_state.fonts.push(font.name);
    }
  }

  const add_text_start = performance.now();
  for (const text of user_state.text_instances) {
    const text_id = instance.add_text(text.value, text.font, INITIAL_TEXT_SIZE, 0xEEEEEEFF);
    text.id = text_id === 0xFFFFFFFF ? null : text_id;
  }
  stats.last_text_processing_time = performance.now() - add_text_start;
  console.log(`Initial text processing time: ${stats.last_text_processing_time} ms`);

  return {
    module,
    memory,
    instance
  };
}

function initUserControls(demo: Demo) {
  let isPressed = false;
  let lastX = 0;
  let lastY = 0;

  document.addEventListener("mousedown", (e) => {
    if (e.target === demo.canvas) {
      isPressed = e.button == 0;
      lastX = e.clientX;
      lastY = e.clientY;
    }
  });

  document.addEventListener("mousemove", (e) => {
    if (isPressed) {
      const deltaX = e.clientX - lastX;
      const deltaY = e.clientY - lastY;
      demo.user_state.offset_x += deltaX;
      demo.user_state.offset_y += deltaY;
      lastX = e.clientX;
      lastY = e.clientY;
      updateMvp(demo);

      if (e.target === demo.canvas) {
        demo.user_state.animate = false;
      }
    }
  });

  document.addEventListener("mouseup", (e) => {
    isPressed = !(e.button == 0);
  });

  demo.canvas.addEventListener("wheel", (e) => {
    e.preventDefault();
    const zoomSpeed = 0.1 + (demo.user_state.zoom / 30.0);
    if (e.deltaY < 0) {
      demo.user_state.zoom += zoomSpeed;
    } else {
      demo.user_state.zoom -= zoomSpeed;
    }
    demo.user_state.zoom = Math.max(0.1, demo.user_state.zoom);
    updateMvp(demo);
  });

  demo.canvas.addEventListener("dragover", (e) => {
    e.preventDefault();
    e.dataTransfer!.dropEffect = "copy";
  });

  demo.canvas.addEventListener("drop", (e) => {
    e.preventDefault();
    const files = e.dataTransfer?.files;
    if (!files) return;
    if (files.length !== 1) {
      console.log("Only one file can be uploaded at a time.");
      return;
    }

    const fontExtensions = [".ttf", ".otf"];
    const textExtensions = [".txt"];
    const file = files[0];

    const baseName = file.name.substring(0, file.name.lastIndexOf("."));
    const extension = file.name.substring(file.name.lastIndexOf(".")).toLowerCase();

    if (fontExtensions.includes(extension)) {
      const reader = new FileReader();
      reader.onload = (event) => {
        const data = new Uint8Array(event.target?.result as ArrayBuffer);
        demo.user_state.fonts.push(baseName);
        demo.user_state.default_font = demo.user_state.fonts.length - 1;
        demo.client.instance.add_font(baseName, data);
        updateTextInstanceFonts(demo);
        console.log(`Added ${file.name} to the app. Iterate fonts using the arrow keys.`);
      };
      reader.readAsArrayBuffer(file);
    } else if (textExtensions.includes(extension)) {
      const reader = new FileReader();
      reader.onload = (event) => {
        const textContent = event.target?.result as string;
        const default_text_instance = demo.user_state.text_instances[0];
        if (!default_text_instance || default_text_instance.id === null) { return; }

        const add_text_start = performance.now();
        demo.client.instance.update_text_value(default_text_instance.id, textContent);
        demo.stats.last_text_processing_time = performance.now() - add_text_start;
        console.log(`Text processing time: ${demo.stats.last_text_processing_time} ms`);

        demo.user_state.offset_x = 0;
        demo.user_state.offset_y = 0;
        updateMvp(demo);

        console.log(`Added text from ${file.name} to the app.`);
      };
      reader.readAsText(file);
    } else {
      console.error("File extension not supported:", extension);
    }
  });

  document.addEventListener("keydown", (e) => {
    const state = demo.user_state;
    if (e.key === "ArrowLeft") {
      state.default_font = (state.default_font - 1 + state.fonts.length) % state.fonts.length;
      updateTextInstanceFonts(demo);
    } else if (e.key === "ArrowRight") {
      state.default_font = (state.default_font + 1) % state.fonts.length;
      updateTextInstanceFonts(demo);
    }
  });
}

async function initial_user_state() {
  const canvas = document.getElementById("canvas") as HTMLCanvasElement;
  const the_entire_bee_movie_script = await (await fetch("bee.txt")).text();
  return {
    offset_x: 30,
    offset_y: canvas.height / 2.0,
    view_width: canvas.width,
    view_height: canvas.height,
    zoom: 1.0,
    text_instances: [
      {value: the_entire_bee_movie_script, font: "OpenSans", size: INITIAL_TEXT_SIZE, id: null}
    ],
    fonts: [],
    default_font: 0,
    animate: true,  // Gently scroll up the text, turns to false if the use interacts with the mouse controls
  };
}

async function init() {
  const canvas = document.getElementById("canvas") as HTMLCanvasElement;
  canvas.width = document.body.clientWidth * devicePixelRatio;
  canvas.height = document.body.clientHeight * devicePixelRatio;

  // WebGPU init
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) throw new Error("No WebGPU adapter");
  const device = await adapter.requestDevice();
  const ctx = canvas.getContext("webgpu")!;
  const format = navigator.gpu.getPreferredCanvasFormat();
  ctx.configure({ device, format, alphaMode: "premultiplied" });

  // Initial state
  const user_state = await initial_user_state();

  const stats = {
    vertex_buffer_total_size: 0,
    vertices_size: 0,
    indices_size: 0,
    fonts_size: [],
    last_text_processing_time: 0
  }

  // Gpu resources
  const resources = await initGpuResources(device, format);

  // Rust wasm app
  const initial_assets = { 
    fonts: [
      { name: "OpenSans", path: "opensans.ttf" },
    ]
  };
  const client = await loadDemo(initial_assets, resources, user_state, stats);

  const demo = {
    canvas,
    device,
    ctx,
    client,
    resources,
    user_state,
    stats,
    assets: initial_assets,
    reload: false,
  };

  initUserControls(demo);
  updateMvp(demo);
  (window as any).demo = demo;

  boundedRun = run.bind(null, demo);
  boundedRun();
}

init().catch(console.error);

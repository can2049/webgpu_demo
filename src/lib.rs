//! # WebGPU Demo — Rust + WASM / Native
//!
//! 使用 Rust 编写的 WebGPU 渲染演示，支持两种运行方式:
//! - **浏览器 (WASM)**: 编译为 WebAssembly，通过 `wasm-pack` 构建，运行在浏览器中
//! - **桌面 (Native)**: 直接 `cargo run`，使用 winit 窗口 + Vulkan/Metal/DX12 后端
//!
//! ## 模块结构
//!
//! ```text
//! lib.rs          ← WASM 入口（本文件）
//! main.rs         ← Native 桌面入口（winit 事件循环）
//! ├── gpu.rs      ← WebGPU 设备初始化（平台适配层）
//! ├── params.rs   ← Uniform 参数定义
//! ├── pipeline.rs ← Compute/Render 管线创建
//! ├── texture.rs  ← 纹理和 bind group 管理
//! └── state.rs    ← 应用状态（平台无关，组装上述模块）
//! ```

pub mod gpu;
pub mod params;
pub mod pipeline;
pub mod state;
pub mod texture;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

// ─────────────────────────────────────────────
//  WASM 入口和浏览器事件集成
// ─────────────────────────────────────────────

/// WASM 模块的主入口函数，由 JavaScript 端显式调用并 await。
///
/// 完成以下工作:
/// 1. 设置 panic hook（让 Rust panic 信息显示在浏览器控制台）
/// 2. 初始化日志系统
/// 3. 获取 HTML Canvas 元素
/// 4. 创建渲染状态（初始化 WebGPU）
/// 5. 注册 window resize 事件监听
/// 6. 启动 `requestAnimationFrame` 动画循环
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn start() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Info).expect("Failed to init logger");

    let window = web_sys::window().expect("No window");
    let document = window.document().expect("No document");
    let canvas = document
        .get_element_by_id("webgpu-canvas")
        .expect("No canvas element with id 'webgpu-canvas'")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .expect("Element is not a canvas");

    let width = canvas.client_width() as u32;
    let height = canvas.client_height() as u32;
    canvas.set_width(width);
    canvas.set_height(height);

    // WASM 路径: Canvas → GpuContext → State
    let ctx = gpu::init_gpu_wasm(canvas.clone(), width, height).await;
    let state = state::State::new(ctx, width, height);

    // 使用 Rc<RefCell<>> 共享 state，因为多个 JS 回调都需要访问它。
    // WASM 是单线程的，所以 Rc 就够了，不需要 Arc。
    let state = std::rc::Rc::new(std::cell::RefCell::new(state));

    setup_resize_handler(&window, &canvas, &state);
    start_animation_loop(&state);
}

/// 注册浏览器窗口 resize 事件监听器。
#[cfg(target_arch = "wasm32")]
fn setup_resize_handler(
    window: &web_sys::Window,
    canvas: &web_sys::HtmlCanvasElement,
    state: &std::rc::Rc<std::cell::RefCell<state::State>>,
) {
    let state = state.clone();
    let canvas = canvas.clone();
    let closure = Closure::<dyn FnMut()>::new(move || {
        let w = canvas.client_width() as u32;
        let h = canvas.client_height() as u32;
        if w > 0 && h > 0 {
            canvas.set_width(w);
            canvas.set_height(h);
            state.borrow_mut().resize(w, h);
        }
    });
    window
        .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
        .expect("Failed to add resize listener");
    closure.forget();
}

/// 启动 `requestAnimationFrame` 驱动的渲染循环。
///
/// 使用 `Rc<RefCell<Option<Closure>>>` 模式让闭包可以递归地注册下一帧回调。
#[cfg(target_arch = "wasm32")]
fn start_animation_loop(state: &std::rc::Rc<std::cell::RefCell<state::State>>) {
    let f: std::rc::Rc<std::cell::RefCell<Option<Closure<dyn FnMut(f64)>>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));
    let g = f.clone();
    let state = state.clone();

    *g.borrow_mut() = Some(Closure::new(move |timestamp: f64| {
        let time_sec = (timestamp / 1000.0) as f32;
        state.borrow().render(time_sec);
        request_animation_frame(f.borrow().as_ref().unwrap());
    }));

    request_animation_frame(g.borrow().as_ref().unwrap());
}

#[cfg(target_arch = "wasm32")]
fn request_animation_frame(f: &Closure<dyn FnMut(f64)>) {
    web_sys::window()
        .unwrap()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("Failed to call requestAnimationFrame");
}

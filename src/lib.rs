//! # WebGPU Demo — Rust + WASM
//!
//! 一个使用 Rust 编写、编译为 WebAssembly 运行在浏览器中的 WebGPU 渲染演示。
//!
//! ## 模块结构
//!
//! ```text
//! lib.rs          ← WASM 入口和动画循环（本文件）
//! ├── gpu.rs      ← WebGPU 设备初始化
//! ├── params.rs   ← Uniform 参数定义
//! ├── pipeline.rs ← Compute/Render 管线创建
//! ├── texture.rs  ← 纹理和 bind group 管理
//! └── state.rs    ← 应用状态（组装上述模块）
//! ```
//!
//! ## 渲染架构
//!
//! 每帧执行两个 GPU pass:
//! 1. **Compute Pass** — 运行 `compute.wgsl`，在 GPU 上并行计算每个像素的颜色
//! 2. **Render Pass** — 运行 `render.wgsl`，将计算结果绘制到屏幕

mod gpu;
mod params;
mod pipeline;
mod state;
mod texture;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

// ─────────────────────────────────────────────
//  WASM 入口和浏览器事件集成
// ─────────────────────────────────────────────

/// WASM 模块的入口函数，浏览器加载 WASM 后自动调用。
///
/// 完成以下工作:
/// 1. 设置 panic hook（让 Rust panic 信息显示在浏览器控制台）
/// 2. 初始化日志系统
/// 3. 获取 HTML Canvas 元素
/// 4. 创建渲染状态（初始化 WebGPU）
/// 5. 注册 window resize 事件监听
/// 6. 启动 `requestAnimationFrame` 动画循环
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub async fn start() {
    // console_error_panic_hook 将 Rust panic 转发到浏览器控制台，
    // 默认情况下 WASM 中的 panic 只会显示 "unreachable" 错误。
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Info).expect("Failed to init logger");

    // 从 DOM 获取 canvas 元素，所有 WebGPU 渲染都输出到这个 canvas
    let window = web_sys::window().expect("No window");
    let document = window.document().expect("No document");
    let canvas = document
        .get_element_by_id("webgpu-canvas")
        .expect("No canvas element with id 'webgpu-canvas'")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .expect("Element is not a canvas");

    // 设置 canvas 像素尺寸与 CSS 布局尺寸一致，避免模糊
    let width = canvas.client_width() as u32;
    let height = canvas.client_height() as u32;
    canvas.set_width(width);
    canvas.set_height(height);

    let state = state::State::new(canvas.clone(), width, height).await;

    // 使用 Rc<RefCell<>> 共享 state，因为多个 JS 回调（resize、animation）都需要访问它。
    // WASM 是单线程的，所以 Rc 就够了，不需要 Arc。
    let state = std::rc::Rc::new(std::cell::RefCell::new(state));

    setup_resize_handler(&window, &canvas, &state);
    start_animation_loop(&state);
}

/// 注册浏览器窗口 resize 事件监听器。
///
/// 当用户调整浏览器窗口大小时，同步更新 canvas 像素尺寸并重建 GPU 资源。
/// `closure.forget()` 让闭包的生命周期跟随整个页面，不会被 Rust 的 drop 回收。
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
/// 浏览器每帧（通常 60fps）回调一次，传入高精度时间戳。
///
/// ## 实现细节
///
/// 使用 `Rc<RefCell<Option<Closure>>>` 模式让闭包可以递归地注册下一帧回调。
/// 这是 Rust WASM 中实现 requestAnimationFrame 循环的标准做法，
/// 因为闭包需要引用自身来注册下一帧。
#[cfg(target_arch = "wasm32")]
fn start_animation_loop(state: &std::rc::Rc<std::cell::RefCell<state::State>>) {
    let f: std::rc::Rc<std::cell::RefCell<Option<Closure<dyn FnMut(f64)>>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));
    let g = f.clone();
    let state = state.clone();

    // `f` 持有闭包本身，闭包内部通过 `f` 引用自己来注册下一帧
    *g.borrow_mut() = Some(Closure::new(move |timestamp: f64| {
        let time_sec = (timestamp / 1000.0) as f32;
        state.borrow().render(time_sec);
        request_animation_frame(f.borrow().as_ref().unwrap());
    }));

    // 触发第一帧
    request_animation_frame(g.borrow().as_ref().unwrap());
}

/// 调用浏览器的 `window.requestAnimationFrame()` API。
#[cfg(target_arch = "wasm32")]
fn request_animation_frame(f: &Closure<dyn FnMut(f64)>) {
    web_sys::window()
        .unwrap()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("Failed to call requestAnimationFrame");
}

# WebGPU Demo — Rust + WASM / Native

使用 Rust 编写的 WebGPU 演示，支持**两种运行方式**：

- **浏览器 (WASM)**: 编译为 WebAssembly，通过 WebGPU API 运行
- **桌面 (Native)**: 直接 `cargo run`，使用 Vulkan / Metal / DX12 后端，无需浏览器

同时展示了 **Compute Shader（通用计算）** 和 **Render Pipeline（渲染管线）** 两种 GPU 能力。

### 两条路径对比

| | Native 桌面 | WASM 浏览器 |
|---|---|---|
| **启动命令** | `cargo run` | `./build.sh` + 浏览器打开 |
| **入口文件** | `src/main.rs` | `src/lib.rs` |
| **窗口系统** | winit (OS 原生窗口) | HTML Canvas |
| **GPU 后端** | Vulkan / Metal / DX12 | WebGPU 浏览器 API |
| **事件循环** | winit `EventLoop` | `requestAnimationFrame` |
| **调试方式** | GDB/LLDB, `RUST_LOG`, 终端日志 | 浏览器 DevTools |
| **修改 shader 后** | `cargo run` 即可 | 需重新 `./build.sh` + 刷新浏览器 |
| **额外依赖** | 无 | wasm-pack + WebGPU 浏览器 |

两条路径共享全部渲染逻辑（`state.rs`、`pipeline.rs`、`texture.rs`、`params.rs`、两个 `.wgsl` shader），渲染效果完全一致。平台差异仅在 `gpu.rs` 的初始化层处理。

## 效果说明

- **Compute Shader** 在 GPU 上并行计算每个像素的颜色（基于分形色彩算法），输出到一张 storage texture
- **Render Pipeline** 将 compute 生成的纹理采样并绘制到屏幕上（全屏四边形 blit）
- 动画效果随时间变化，窗口可自适应缩放

## 前置条件

| 工具 | 最低版本 | 安装方式 | 用途 |
|------|---------|---------|------|
| Rust | 1.85+ | [rustup.rs](https://rustup.rs/) | Native + WASM |
| wasm-pack | 0.13+ | `cargo install wasm-pack` | 仅 WASM |
| 支持 WebGPU 的浏览器 | Chrome 113+ / Edge 113+ / Firefox Nightly | — | 仅 WASM |

> WASM 构建还需安装 wasm32 target: `rustup target add wasm32-unknown-unknown`

## 运行方式一：桌面 Native（推荐调试用）

无需浏览器，直接在桌面窗口中运行。使用操作系统原生的 GPU 后端（Linux: Vulkan, macOS: Metal, Windows: DX12）。

```bash
# 直接运行（debug 模式，编译快，便于调试）
cargo run

# 带日志输出
RUST_LOG=info cargo run

# release 模式（性能更好）
cargo run --release
```

启动后会弹出一个 800×600 的桌面窗口，显示与浏览器版本相同的分形动画效果。支持窗口缩放。

> **实测环境：** Ubuntu 20.04 + NVIDIA RTX 3060 + 驱动 550 + Vulkan loader 1.2.131，`cargo run` 正常运行。

## 运行方式二：浏览器 WASM

```bash
# 构建 WASM
./build.sh

# 启动本地 HTTP 服务器
cd web
python3 -m http.server 8080 --bind localhost
```

然后在支持 WebGPU 的浏览器中打开 **http://localhost:8080**

> **必须使用 `localhost`**，不能用 `127.0.0.1` 或局域网 IP。WebGPU API (`navigator.gpu`) 只在 [Secure Context](https://developer.mozilla.org/en-US/docs/Web/Security/Secure_Contexts) 下暴露，对于 HTTP 协议只有 `localhost` 被视为安全上下文。

> 也可以使用任意其他静态文件服务器，如 `npx serve web` 或 `caddy file-server --root web --listen :8080`

### Linux 下启用 WebGPU

Chrome 在 Linux 上对 WebGPU 的 Vulkan 后端采用保守策略，即使 `chrome://gpu` 显示 "WebGPU: Hardware accelerated"，`navigator.gpu.requestAdapter()` 仍可能返回 `null`。这是因为 Chrome 维护了一份 GPU 驱动 blocklist，较老的 Vulkan loader 或未经验证的驱动+内核组合会被拦截。

**需要在 `chrome://flags` 中启用以下 flag：**

| Flag | 设置 | 说明 |
|------|------|------|
| `#enable-unsafe-webgpu` | **Enabled** | 跳过 GPU blocklist 检查，允许 WebGPU 在未验证的驱动上运行 |

修改后重启 Chrome 生效。

也可以通过命令行参数一次性启用：

```bash
google-chrome --enable-unsafe-webgpu http://localhost:8080
```

> **实测环境：** Ubuntu 20.04 + NVIDIA RTX 3060 + 驱动 550 + Chrome 148，Vulkan loader 版本 1.2.131。在此环境下必须启用 `enable-unsafe-webgpu` 才能获取到 GPU adapter。

## 调试

### 0. Native 桌面调试（推荐）

Native 模式下可以直接使用 Rust 生态的调试工具，无需浏览器：

```bash
# 带日志运行
RUST_LOG=info cargo run

# 使用 GDB/LLDB 调试
cargo build && gdb target/debug/webgpu_demo

# wgpu 验证层详细日志
RUST_LOG=wgpu=warn cargo run
```

修改 shader 后只需 `cargo run` 即可看到效果（无需 wasm-pack 构建 + 浏览器刷新）。

### 1. 浏览器 DevTools 控制台

Rust 端的 `log::info!` / `log::warn!` 等日志会输出到浏览器控制台。打开 DevTools (F12) → Console 查看。

如果 WASM 代码 panic，`console_error_panic_hook` 会将完整的 Rust panic 信息（含堆栈）打印到控制台。

### 2. WebGPU 错误

Chrome DevTools → Console 会显示 WebGPU 验证错误。常见问题：
- `GPUValidationError`: shader 绑定或格式不匹配
- `Lost device`: GPU 设备丢失，需要刷新

### 3. Shader 调试

- Chrome: 安装 [WebGPU Inspector](https://chromewebstore.google.com/detail/webgpu-inspector/holcfbmeagmapjlgioacjbpadlnjoeda) 扩展，可以实时查看 GPU 资源、pipeline、draw call
- 修改 `src/compute.wgsl` 后重新 `./build.sh` 并刷新浏览器

### 4. Rust 源码级调试 (DWARF)

构建 debug 版本的 WASM 以获得源码映射：

```bash
wasm-pack build --dev --target web --out-dir web/pkg
```

Chrome DevTools → Sources → 可以看到 Rust 源文件并设置断点。需要在 DevTools 设置中启用 **WebAssembly Debugging: Enable DWARF support**。

### 5. 性能分析

- Chrome DevTools → Performance 面板录制帧，可以看到 WASM 函数耗时
- `chrome://gpu` 页面查看 WebGPU 后端状态和功能支持

## 项目结构

```
webgpu_demo/
├── Cargo.toml              # Rust 依赖配置
├── build.sh                # WASM 构建脚本
├── src/
│   ├── main.rs             # [Native 独占] 桌面入口 (winit 事件循环)
│   ├── lib.rs              # [WASM 独占]   浏览器入口 (requestAnimationFrame)
│   ├── gpu.rs              # [平台适配层]   GPU 初始化，含 native/wasm 两条路径
│   ├── params.rs           # [共享] Uniform 参数结构定义 (时间、分辨率)
│   ├── pipeline.rs         # [共享] Compute/Render Pipeline 创建和配置
│   ├── texture.rs          # [共享] 纹理、Bind Group、Sampler 管理
│   ├── state.rs            # [共享] 应用状态: 平台无关的渲染逻辑
│   ├── compute.wgsl        # [共享] Compute shader: GPU 并行生成分形图像
│   └── render.wgsl         # [共享] Render shader: 全屏纹理采样 blit
└── web/
    ├── index.html           # [WASM 独占] 浏览器入口 HTML
    └── pkg/                 # wasm-pack 构建输出 (git ignored)
```

### 代码隔离关系

```
           ┌──────────────────┐         ┌──────────────────┐
           │  Native 独占代码   │         │   WASM 独占代码    │
           │                  │         │                  │
           │  main.rs         │         │  lib.rs          │
           │  (winit 窗口/    │         │  (Canvas/        │
           │   事件循环)       │         │   AnimationFrame) │
           └────────┬─────────┘         └────────┬─────────┘
                    │                            │
                    ▼                            ▼
           ┌────────────────┐          ┌──────────────────┐
           │ gpu.rs          │          │ gpu.rs            │
           │ init_gpu_native │          │ init_gpu_wasm     │
           │ #[cfg(native)]  │          │ #[cfg(wasm32)]    │
           └────────┬────────┘          └────────┬─────────┘
                    │                            │
                    └────────────┬───────────────┘
                                 ▼
                    ┌────────────────────────┐
                    │ gpu.rs: init_gpu_common │
                    │ (平台无关的 GPU 初始化)  │
                    └────────────┬───────────┘
                                 ▼
              ┌──────────────────────────────────┐
              │        完全共享的渲染代码           │
              │                                  │
              │  state.rs    — 渲染状态与帧循环    │
              │  pipeline.rs — GPU 管线创建        │
              │  texture.rs  — 纹理/BindGroup     │
              │  params.rs   — Uniform 参数        │
              │  compute.wgsl — Compute Shader    │
              │  render.wgsl  — Render Shader     │
              └──────────────────────────────────┘
```

## 架构

```
┌─────────────────────────────────────────────────────────────┐
│                    平台适配层 (gpu.rs)                        │
│                                                             │
│  Native (main.rs)              WASM (lib.rs)                │
│  ┌───────────────┐             ┌────────────────────┐       │
│  │ winit Window   │             │ HTML Canvas         │       │
│  │ Vulkan/Metal   │             │ WebGPU API          │       │
│  │ /DX12          │             │ requestAnimFrame    │       │
│  └───────┬───────┘             └─────────┬──────────┘       │
│          └──────────┬────────────────────┘                  │
│                     ▼                                       │
│              GpuContext (device, queue, surface)             │
└─────────────────────┬───────────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────┐
│        平台无关渲染逻辑 (state.rs)                │
│                                                  │
│  每一帧:                                          │
│  1. 更新 uniform buffer (时间、分辨率)             │
│                                                  │
│  2. Compute Pass                                 │
│     ┌──────────────────────────┐                 │
│     │ compute.wgsl             │                 │
│     │ @workgroup_size(16, 16)  │                 │
│     │ 并行计算每像素 → storage texture │           │
│     └──────────────────────────┘                 │
│              │                                   │
│              ▼                                   │
│  3. Render Pass                                  │
│     ┌──────────────────────────┐                 │
│     │ render.wgsl              │                 │
│     │ 全屏三角带 → 采样 texture  │                │
│     │ → 输出到 surface          │                │
│     └──────────────────────────┘                 │
│              │                                   │
│              ▼                                   │
│  4. queue.submit → present                       │
└─────────────────────────────────────────────────┘
```

## 常见问题

### Native 桌面

**Q: `cargo run` 报 "No wgpu backend feature"**
A: wgpu 没有启用 GPU 后端。确认 `Cargo.toml` 中 native target 的 wgpu 依赖包含 `vulkan`（Linux）、`metal`（macOS）或 `dx12`（Windows）feature。

**Q: `cargo run` 报 "Failed to find adapter"**
A: 系统没有可用的 GPU 或驱动未正确安装。Linux 上确认已安装 Vulkan 驱动：`vulkaninfo | head` 应有输出。NVIDIA 用户安装 `nvidia-driver-xxx`，AMD 用户安装 `mesa-vulkan-drivers`。

**Q: `cargo run` 报 "Too many bindings of type StorageTextures, limit is 0"**
A: Device limits 过于保守。本项目的 compute shader 需要 storage texture，不能使用 `downlevel_webgl2_defaults()`（它将 storage texture 数量设为 0）。Native 平台应使用 `Limits::default()`。

**Q: `cargo run` 窗口弹出但黑屏**
A: 运行 `RUST_LOG=warn cargo run` 查看 wgpu 验证层日志。常见原因是 GPU 不支持 storage texture 的 `Rgba8Unorm` 格式（老旧集成显卡可能不支持）。

**Q: Native 和 WASM 渲染效果不一致**
A: 两个路径使用完全相同的 shader 和渲染逻辑。如果颜色略有差异，通常是 surface 格式不同导致的 gamma 校正差异（Native 可能选到非 sRGB 格式）。功能上没有区别。

### WASM 浏览器

**Q: 页面显示 "Insecure Context"**
A: 你没有通过 `localhost` 访问。WebGPU 只在安全上下文中可用，`http://127.0.0.1` 和 `http://<IP>` 都不算。请使用 `http://localhost:8080`。

**Q: 页面显示 "WebGPU Not Available"**
A: 当前浏览器不支持 WebGPU。请使用 Chrome 113+、Edge 113+ 或 Firefox Nightly。

**Q: 页面显示 "No GPU Adapter"**
A: `navigator.gpu` 存在但 `requestAdapter()` 返回了 `null`，通常是 Linux 上 Chrome 的 GPU blocklist 拦截导致。解决方法：`chrome://flags` 中启用 `#enable-unsafe-webgpu`，重启 Chrome。详见上方 [Linux 下启用 WebGPU](#linux-下启用-webgpu)。

**Q: 画面黑屏但没有报错**
A: 检查 canvas 尺寸是否为 0。在 DevTools Console 执行 `document.getElementById('webgpu-canvas').width` 确认。

**Q: 修改 shader 后没有变化**
A: 确保重新执行 `./build.sh`，浏览器硬刷新 (Ctrl+Shift+R) 清除缓存。Native 模式下 `cargo run` 会自动重新编译，无需额外操作。

# WebGPU Demo — Rust + WASM

使用 Rust 编写、编译为 WASM 运行在浏览器中的 WebGPU 演示。同时展示了 **Compute Shader（通用计算）** 和 **Render Pipeline（渲染管线）** 两种 GPU 能力。

## 效果说明

- **Compute Shader** 在 GPU 上并行计算每个像素的颜色（基于分形色彩算法），输出到一张 storage texture
- **Render Pipeline** 将 compute 生成的纹理采样并绘制到屏幕上（全屏四边形 blit）
- 动画效果随时间变化，窗口可自适应缩放

## 前置条件

| 工具 | 最低版本 | 安装方式 |
|------|---------|---------|
| Rust | 1.85+ | [rustup.rs](https://rustup.rs/) |
| wasm-pack | 0.13+ | `cargo install wasm-pack` |
| 支持 WebGPU 的浏览器 | Chrome 113+ / Edge 113+ / Firefox Nightly | — |

> 确认已安装 wasm32 target: `rustup target add wasm32-unknown-unknown`

## 构建

```bash
# 一键构建
./build.sh

# 或手动执行
wasm-pack build --target web --out-dir web/pkg
```

构建产物在 `web/pkg/` 目录下。

## 运行

构建完成后，需要通过 HTTP 服务器访问（WASM 模块不能通过 `file://` 协议加载）：

```bash
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
├── build.sh                # 构建脚本
├── src/
│   ├── lib.rs              # WASM 入口、模块声明、浏览器事件集成
│   ├── gpu.rs              # WebGPU 设备初始化 (Instance → Adapter → Device)
│   ├── params.rs           # Uniform 参数结构定义 (时间、分辨率)
│   ├── pipeline.rs         # Compute/Render Pipeline 创建和配置
│   ├── texture.rs          # 纹理、Bind Group、Sampler 管理
│   ├── state.rs            # 应用状态: 组装上述模块，resize/render 逻辑
│   ├── compute.wgsl        # Compute shader: GPU 并行生成分形图像
│   └── render.wgsl         # Render shader: 全屏纹理采样 blit
└── web/
    ├── index.html           # 入口 HTML
    └── pkg/                 # wasm-pack 构建输出 (git ignored)
```

## 架构

```
┌─────────────────────────────────────────────────┐
│  每一帧 (requestAnimationFrame)                  │
│                                                  │
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

**Q: 页面显示 "Insecure Context"**
A: 你没有通过 `localhost` 访问。WebGPU 只在安全上下文中可用，`http://127.0.0.1` 和 `http://<IP>` 都不算。请使用 `http://localhost:8080`。

**Q: 页面显示 "WebGPU Not Available"**
A: 当前浏览器不支持 WebGPU。请使用 Chrome 113+、Edge 113+ 或 Firefox Nightly。

**Q: 页面显示 "No GPU Adapter"**
A: `navigator.gpu` 存在但 `requestAdapter()` 返回了 `null`，通常是 Linux 上 Chrome 的 GPU blocklist 拦截导致。解决方法：`chrome://flags` 中启用 `#enable-unsafe-webgpu`，重启 Chrome。详见上方 [Linux 下启用 WebGPU](#linux-下启用-webgpu)。

**Q: 画面黑屏但没有报错**
A: 检查 canvas 尺寸是否为 0。在 DevTools Console 执行 `document.getElementById('webgpu-canvas').width` 确认。

**Q: 修改 shader 后没有变化**
A: 确保重新执行 `./build.sh`，浏览器硬刷新 (Ctrl+Shift+R) 清除缓存。

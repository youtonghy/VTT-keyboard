# VTT Keyboard

**Language:** [English](#english) | [简体中文](#simplified-chinese)

<a id="english"></a>

## English

VTT Keyboard is a desktop voice-to-text keyboard tool built with Tauri. It combines a React frontend with a Rust backend, supports cloud transcription providers and local model runtimes, and is designed for quickly turning speech into text from a desktop app.

## Requirements

- Rust toolchain
- Bun
- Docker (Docker Desktop on Windows/macOS)
- NVIDIA GPU and Docker GPU runtime for CUDA-only local models such as Voxtral and Qwen3-ASR

## SenseVoice Local Deployment (Docker)

SenseVoice local mode runs with Docker only. A Python runtime on the host machine is not required.

1. Start Docker Desktop or the Docker daemon.
2. Open the app settings and choose `SenseVoice (Local)`.
3. Click `Download & Enable`.
   - The first run builds the `vtt-sensevoice:local` image and downloads the model.
4. Click `Start Service` to run the containerized service.

## Local Models

Local mode supports switching between four local model runtimes:

- `SenseVoice` (default)
- `Sherpa-ONNX SenseVoice` (native runtime, disabled on Windows ARM64 builds)
- `mistralai/Voxtral-Mini-4B-Realtime-2602` (via vLLM Docker image)
- `Qwen3-ASR` (via vLLM Docker image, default variant `Qwen/Qwen3-ASR-1.7B`)

When switching local models while the service is running, the app stops the previous container first, then starts the new one automatically.

### Voxtral Runtime Notes

- Docker image: `vllm/vllm-openai:nightly`
- API endpoint: `POST /v1/audio/transcriptions`
- Voxtral is CUDA-only via Docker GPU runtime (`--runtime nvidia --gpus all`), with FlashAttention disabled (`--attention-backend TRITON_ATTN`).
- Service bootstrap installs runtime dependency automatically: `mistral-common[soundfile]>=1.9.0`.
- CPU fallback is disabled for Voxtral.
- Model weights are pulled on first service start and cached under the local model directory.

### Qwen3-ASR Runtime Notes

- Docker image: `vllm/vllm-openai:nightly`
- API endpoint: `POST /v1/audio/transcriptions`
- Preset model variants:
  - `Qwen/Qwen3-ASR-1.7B` (default)
  - `Qwen/Qwen3-ASR-0.6B`
  - `Qwen/Qwen3-ForcedAligner-0.6B`
- CUDA GPU is required via Docker GPU runtime (`--runtime nvidia --gpus all`).

## Release and Updater Signing

GitHub Actions release builds require updater signing secrets:

- `TAURI_SIGNING_PRIVATE_KEY`: updater private key content or a path available in CI
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: password matching that private key

The workflow runs a small `tauri signer sign` smoke test before `tauri build`, so secret mismatches fail fast instead of failing only after packaging completes.

Release CI also pre-downloads the `sherpa-onnx` native static archive and exposes it through `SHERPA_ONNX_ARCHIVE_DIR`, so `sherpa-onnx-sys` does not need to fetch prebuilt libraries during its own build script.

If a local build hits upstream GitHub release instability, you can reuse the same fallback:

- Set `SHERPA_ONNX_ARCHIVE_DIR` to a directory containing the expected `sherpa-onnx` archive for your target.
- Or set `SHERPA_ONNX_LIB_DIR` to an already extracted `lib` directory for that target.

The app links Sherpa-ONNX statically on supported desktop targets, so Windows installers do not need to ship `sherpa-onnx-c-api.dll` separately at runtime.

## Runtime Notes

- Container name: `vtt-sensevoice-service`
- Image tag: `vtt-sensevoice:local`
- Model cache directory: app local data `sensevoice/models`
- Runtime log path: `sensevoice/runtime/server.log`

## Code Structure

```text
.
├─ src/                          # React UI layer
│  ├─ components/                # Reusable UI components
│  ├─ hooks/                     # UI state and side-effect hooks
│  ├─ i18n/                      # i18n bootstrap and locale resources
│  │  └─ locales/
│  ├─ types/                     # Shared TypeScript types
│  ├─ App.tsx                    # Main app entry component
│  └─ main.tsx                   # Frontend bootstrap
├─ src-tauri/                    # Tauri/Rust backend
│  ├─ src/
│  │  ├─ main.rs                 # Tauri app startup
│  │  ├─ lib.rs                  # Command registration and app wiring
│  │  ├─ settings.rs             # Persistent settings management
│  │  ├─ recorder.rs             # Audio capture
│  │  ├─ processing.rs           # Audio processing pipeline
│  │  ├─ transcription_dispatcher.rs # Transcription routing
│  │  ├─ openai.rs               # OpenAI provider integration
│  │  ├─ volcengine.rs           # Volcengine provider integration
│  │  └─ sensevoice/             # SenseVoice local mode manager/client
│  ├─ native/                    # Platform-specific native overlay code
│  ├─ capabilities/              # Tauri capability definitions
│  └─ tauri.conf.json            # Tauri app config
├─ public/                       # Static assets
└─ package.json                  # Frontend scripts/dependencies
```

## Development

```bash
bun install
bun run tauri dev
```

## Validation

```bash
bun run build
cd src-tauri
cargo check
```

<a id="simplified-chinese"></a>

## 简体中文

VTT Keyboard 是一款桌面语音转文字键盘工具，基于 Tauri 构建。项目使用 React 作为前端、Rust 作为后端，支持云端转写服务和本地模型运行时，适合在桌面端快速把语音转换成文本。

## 环境要求

- Rust toolchain
- Bun
- Docker（Windows/macOS 使用 Docker Desktop）
- 对于 Voxtral、Qwen3-ASR 等仅支持 CUDA 的本地模型，需要 NVIDIA GPU 和 Docker GPU runtime

## SenseVoice 本地部署（Docker）

SenseVoice 本地模式仅通过 Docker 运行，不再要求宿主机安装 Python 运行时。

1. 启动 Docker Desktop 或 Docker daemon。
2. 打开应用设置，选择 `SenseVoice (Local)`。
3. 点击 `Download & Enable`。
   - 首次运行会构建 `vtt-sensevoice:local` 镜像并下载模型。
4. 点击 `Start Service` 启动容器化服务。

## 本地模型

本地模式支持在四种本地模型运行时之间切换：

- `SenseVoice`（默认）
- `Sherpa-ONNX SenseVoice`（原生运行时，在 Windows ARM64 构建中禁用）
- `mistralai/Voxtral-Mini-4B-Realtime-2602`（通过 vLLM Docker 镜像运行）
- `Qwen3-ASR`（通过 vLLM Docker 镜像运行，默认变体为 `Qwen/Qwen3-ASR-1.7B`）

如果服务正在运行时切换本地模型，应用会先停止之前的容器，再自动启动新的模型服务。

### Voxtral 运行说明

- Docker 镜像：`vllm/vllm-openai:nightly`
- API endpoint：`POST /v1/audio/transcriptions`
- Voxtral 仅支持通过 Docker GPU runtime 使用 CUDA（`--runtime nvidia --gpus all`），并禁用 FlashAttention（`--attention-backend TRITON_ATTN`）。
- 服务启动流程会自动安装运行时依赖：`mistral-common[soundfile]>=1.9.0`。
- Voxtral 不启用 CPU fallback。
- 模型权重会在首次启动服务时拉取，并缓存在本地模型目录下。

### Qwen3-ASR 运行说明

- Docker 镜像：`vllm/vllm-openai:nightly`
- API endpoint：`POST /v1/audio/transcriptions`
- 预设模型变体：
  - `Qwen/Qwen3-ASR-1.7B`（默认）
  - `Qwen/Qwen3-ASR-0.6B`
  - `Qwen/Qwen3-ForcedAligner-0.6B`
- 必须通过 Docker GPU runtime 使用 CUDA GPU（`--runtime nvidia --gpus all`）。

## 发布与更新器签名

GitHub Actions 发布构建需要配置更新器签名密钥：

- `TAURI_SIGNING_PRIVATE_KEY`：更新器私钥内容，或 CI 中可访问的私钥路径
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：与该私钥匹配的密码

工作流会在 `tauri build` 之前运行一个小型 `tauri signer sign` 冒烟测试。因此签名密钥不匹配时会更早失败，而不是等到打包结束后才报错。

发布 CI 也会预下载 `sherpa-onnx` 原生静态归档文件，并通过 `SHERPA_ONNX_ARCHIVE_DIR` 暴露给构建流程。这样 `sherpa-onnx-sys` 在自己的构建脚本中不需要再拉取预编译库。

如果本地构建遇到上游 GitHub release 不稳定，可以使用同样的 fallback：

- 将 `SHERPA_ONNX_ARCHIVE_DIR` 设置为一个包含目标平台所需 `sherpa-onnx` 归档文件的目录。
- 或者将 `SHERPA_ONNX_LIB_DIR` 设置为已经解压好的目标平台 `lib` 目录。

应用会在受支持的桌面平台上静态链接 Sherpa-ONNX，因此 Windows 安装包运行时不需要额外携带 `sherpa-onnx-c-api.dll`。

## 运行时说明

- 容器名称：`vtt-sensevoice-service`
- 镜像标签：`vtt-sensevoice:local`
- 模型缓存目录：应用本地数据目录下的 `sensevoice/models`
- 运行日志路径：`sensevoice/runtime/server.log`

## 代码结构

```text
.
├─ src/                          # React UI 层
│  ├─ components/                # 可复用 UI 组件
│  ├─ hooks/                     # UI 状态与副作用 hooks
│  ├─ i18n/                      # i18n 初始化与语言资源
│  │  └─ locales/
│  ├─ types/                     # 共享 TypeScript 类型
│  ├─ App.tsx                    # 应用主入口组件
│  └─ main.tsx                   # 前端启动入口
├─ src-tauri/                    # Tauri/Rust 后端
│  ├─ src/
│  │  ├─ main.rs                 # Tauri 应用启动
│  │  ├─ lib.rs                  # 命令注册与应用装配
│  │  ├─ settings.rs             # 持久化设置管理
│  │  ├─ recorder.rs             # 音频采集
│  │  ├─ processing.rs           # 音频处理流程
│  │  ├─ transcription_dispatcher.rs # 转写路由
│  │  ├─ openai.rs               # OpenAI provider 集成
│  │  ├─ volcengine.rs           # Volcengine provider 集成
│  │  └─ sensevoice/             # SenseVoice 本地模式管理器/客户端
│  ├─ native/                    # 平台相关原生 overlay 代码
│  ├─ capabilities/              # Tauri capability 定义
│  └─ tauri.conf.json            # Tauri 应用配置
├─ public/                       # 静态资源
└─ package.json                  # 前端脚本与依赖
```

## 开发

```bash
bun install
bun run tauri dev
```

## 验证

```bash
bun run build
cd src-tauri
cargo check
```

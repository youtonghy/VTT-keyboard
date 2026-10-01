# VTT Keyboard

**Language:** [English](#english) | [简体中文](#simplified-chinese)

<a id="english"></a>

## English

VTT Keyboard is a desktop voice-to-text keyboard tool built with Tauri. It combines a React frontend with a Rust backend, supports cloud transcription providers and local model runtimes, and is designed for quickly turning speech into text from a desktop app.

## Requirements

- Rust toolchain
- Bun

## Local Speech Recognition (sherpa-onnx)

Local mode runs ONNX models in-process via sherpa-onnx. Docker and a Python runtime are not required.

1. Open the app settings and choose `SenseVoice (Local)` as the provider.
2. Pick a model in the `Local Model` dropdown — the model package is downloaded automatically after selection.
3. Click `Load Model` to load it into memory, or just start recording: the model is loaded on demand.

## Local Models

All local models are pre-packaged ONNX archives published by the sherpa-onnx project (GitHub Releases, with a ModelScope mirror fallback):

- `SenseVoice-Small int8` (default, ~158MB) — Chinese, English, Japanese, Korean, Cantonese
- `Paraformer-zh-small` (~74MB) — lightest Chinese-focused model
- `FireRedASR2-CTC` (~497MB) — Chinese/English with dialect support
- `Fun-ASR-Nano` (~803MB) — newest high-quality Chinese model, slower on CPU

Downloads are stored under the app local data `sensevoice/models` directory. Switching back to a previously downloaded model reuses the local files without re-downloading.

### Upgrading from the Docker-based version

Older releases ran local models inside Docker containers. The runtime is now fully native, and leftover Docker resources are not removed automatically. You can clean them up manually:

```bash
docker rm -f vtt-docker-service vtt-sensevoice-service
docker rmi vtt-sensevoice:local vllm/vllm-openai:nightly
```

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

- Model cache directory: app local data `sensevoice/models`

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
│  │  └─ sensevoice/             # Local ASR model manager/client (sherpa-onnx)
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

## 本地语音识别（sherpa-onnx）

本地模式通过 sherpa-onnx 在进程内运行 ONNX 模型，不再需要 Docker 或 Python 运行时。

1. 打开应用设置，将转写引擎切换为 `SenseVoice (Local)`。
2. 在“本地模型”下拉中选择模型 —— 选中后会自动下载对应的模型包。
3. 点击“加载模型”将其载入内存，或直接开始录音：模型会按需自动加载。

## 本地模型

所有本地模型均为 sherpa-onnx 项目官方预打包的 ONNX 归档（主用 GitHub Releases，失败时回退 ModelScope 镜像）：

- `SenseVoice-Small int8`（默认，约 158MB）——支持中/英/日/韩/粤语
- `Paraformer-zh-small`（约 74MB）——最轻量的中文模型
- `FireRedASR2-CTC`（约 497MB）——中英文并支持多种方言
- `Fun-ASR-Nano`（约 803MB）——最新的高质量中文模型，CPU 上较慢

模型下载后存放在应用本地数据目录的 `sensevoice/models` 下。切换回已下载过的模型会直接复用本地文件，无需重新下载。

### 从 Docker 版本升级

旧版本通过 Docker 容器运行本地模型，现已完全改为原生推理。遗留的 Docker 资源不会自动清理，如有需要可手动删除：

```bash
docker rm -f vtt-docker-service vtt-sensevoice-service
docker rmi vtt-sensevoice:local vllm/vllm-openai:nightly
```

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

- 模型缓存目录：应用本地数据目录下的 `sensevoice/models`

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
│  │  └─ sensevoice/             # 本地 ASR 模型管理器/客户端（sherpa-onnx）
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

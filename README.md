# VTT Keyboard

Desktop voice-to-text keyboard tool built with Tauri (Rust backend + React frontend).

## Requirements

- Rust toolchain
- Node.js + pnpm
- Docker (Docker Desktop on Windows/macOS)

## SenseVoice Local Deployment (Docker)

SenseVoice local mode now runs with Docker only. Python runtime on host is no longer required.

1. Start Docker Desktop (or Docker daemon).
2. Open app settings and choose `SenseVoice (Local)`.
3. Click `Download & Enable`.
   - First run will build image `vtt-sensevoice:local` and download model.
4. Click `Start Service` to run the containerized service.

## Local Models (SenseVoice / Voxtral)

Local mode supports switching between two models:

- `SenseVoice` (default)
- `mistralai/Voxtral-Mini-4B-Realtime-2602` (via vLLM Docker image)

When switching local models while the service is running, the app will stop the previous container first, then start the new one automatically.

Voxtral runtime notes:

- Docker image: `vllm/vllm-openai:nightly`
- API endpoint: `POST /v1/audio/transcriptions`
- Voxtral is CUDA-only via Docker GPU runtime (`--runtime nvidia --gpus all`), with FlashAttention disabled (`--attention-backend TRITON_ATTN`).
- Service bootstrap installs runtime dependency automatically: `mistral-common[soundfile]>=1.9.0`.
- CPU fallback is disabled for Voxtral.
- Model weights are pulled on first service start and cached under local model directory.

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
pnpm install
pnpm tauri dev
```

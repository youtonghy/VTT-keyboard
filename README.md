# VTT Keyboard

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

## Runtime Notes

- Container name: `vtt-sensevoice-service`
- Image tag: `vtt-sensevoice:local`
- Model cache directory: app local data `sensevoice/models`
- Runtime log path: `sensevoice/runtime/server.log`

#!/bin/bash
set -e

PIP_DONE="/tmp/.pip_done"
if [ ! -f "$PIP_DONE" ]; then
    echo "[entrypoint] Installing vllm[audio] and mistral-common..."
    pip install --no-cache-dir "vllm[audio]" "mistral-common[soundfile]>=1.9.0"
    touch "$PIP_DONE"
    echo "[entrypoint] pip install complete."
fi

if [ ! -f "/config/model.conf" ]; then
    echo "[entrypoint] ERROR: /config/model.conf not found!"
    exit 1
fi
source /config/model.conf

if [ -z "$MODEL_ID" ]; then
    echo "[entrypoint] ERROR: MODEL_ID not set in config!"
    exit 1
fi

echo "[entrypoint] Starting vLLM with model: $MODEL_ID"
# shellcheck disable=SC2086
exec vllm serve "$MODEL_ID" --host 0.0.0.0 --port ${VLLM_PORT:-8000} \
    --enforce-eager --gpu-memory-utilization ${VLLM_GPU_MEM:-0.8} $VLLM_EXTRA_ARGS

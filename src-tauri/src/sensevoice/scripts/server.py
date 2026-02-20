import os
import sys
import tempfile
import threading
import traceback
from contextlib import asynccontextmanager
from pathlib import Path

from fastapi import FastAPI, File, Form, HTTPException, UploadFile

MODEL = None
POSTPROCESS = None
MODEL_READY = False
MODEL_LOADING = False
MODEL_ERROR = None
MODEL_LOCK = threading.Lock()
LOG_LOCK = threading.Lock()
HF_DEFAULT_MODEL_ID = "FunAudioLLM/SenseVoiceSmall"
MS_DEFAULT_MODEL_ID = "iic/SenseVoiceSmall"


def log(message: str):
    with LOG_LOCK:
        print(f"[sensevoice] {message}", file=sys.stderr, flush=True)


def get_funasr_runtime():
    from funasr import AutoModel
    from funasr.utils.postprocess_utils import rich_transcription_postprocess

    return AutoModel, rich_transcription_postprocess


def resolve_device(value: str) -> str:
    if value != "auto":
        return value
    try:
        import torch  # type: ignore

        if torch.cuda.is_available():
            return "cuda:0"
    except Exception:
        pass
    return "cpu"


def resolve_model_id_for_hub(model_id: str, hub: str) -> str:
    normalized_hub = hub.strip().lower()
    if model_id in {HF_DEFAULT_MODEL_ID, MS_DEFAULT_MODEL_ID}:
        if normalized_hub == "hf":
            return HF_DEFAULT_MODEL_ID
        if normalized_hub == "ms":
            return MS_DEFAULT_MODEL_ID
    return model_id


def build_model():
    auto_model, rich_transcription_postprocess = get_funasr_runtime()
    model_id = os.getenv("SENSEVOICE_MODEL_ID", MS_DEFAULT_MODEL_ID)
    model_dir = os.getenv("SENSEVOICE_MODEL_DIR", "")
    device = resolve_device(os.getenv("SENSEVOICE_DEVICE", "auto"))
    hub = os.getenv("SENSEVOICE_HUB", "hf")
    selected_model_id = resolve_model_id_for_hub(model_id, hub)

    if model_dir:
        model_root = os.path.abspath(model_dir)
        os.makedirs(model_root, exist_ok=True)
        os.environ["HF_HOME"] = os.path.join(model_root, "hf_home")
        os.environ["MODELSCOPE_CACHE"] = os.path.join(model_root, "ms_cache")

    if selected_model_id != model_id:
        log(f"normalized model id from {model_id} to {selected_model_id} for hub={hub}")

    # trust_remote_code=True 但不指定 remote_code 路径，
    # 让 funasr 自动从模型目录解析 model.py（如存在），
    # 兼容不同版本的 funasr 及 ModelScope / HuggingFace 下载的模型文件。
    model = auto_model(
        model=selected_model_id,
        hub=hub,
        trust_remote_code=True,
        vad_model="fsmn-vad",
        vad_kwargs={"max_single_segment_time": 30000},
        device=device,
    )
    return model, rich_transcription_postprocess


def load_model_worker():
    global MODEL, POSTPROCESS, MODEL_READY, MODEL_LOADING, MODEL_ERROR
    try:
        log("model warmup started")
        model, postprocess = build_model()
        with MODEL_LOCK:
            MODEL = model
            POSTPROCESS = postprocess
            MODEL_READY = True
            MODEL_ERROR = None
        log("model warmup finished")
    except BaseException as exc:
        with MODEL_LOCK:
            MODEL = None
            POSTPROCESS = None
            MODEL_READY = False
            MODEL_ERROR = str(exc)
        log(f"model warmup failed: {exc}")
        traceback.print_exc()
    finally:
        with MODEL_LOCK:
            MODEL_LOADING = False


def ensure_model_loading():
    global MODEL_LOADING, MODEL_ERROR
    with MODEL_LOCK:
        if MODEL_READY or MODEL_LOADING:
            return
        MODEL_LOADING = True
        MODEL_ERROR = None

    worker = threading.Thread(target=load_model_worker, daemon=True)
    worker.start()


def get_model_runtime():
    ensure_model_loading()
    with MODEL_LOCK:
        if MODEL_READY and MODEL is not None and POSTPROCESS is not None:
            return MODEL, POSTPROCESS
        if MODEL_ERROR:
            raise HTTPException(
                status_code=503,
                detail=f"SenseVoice model warmup failed: {MODEL_ERROR}",
            )

    raise HTTPException(
        status_code=503,
        detail="SenseVoice model is warming up, please retry shortly",
    )


@asynccontextmanager
async def lifespan(_app: FastAPI):
    log("lifespan startup begin")
    ensure_model_loading()
    log("lifespan startup done")
    yield
    log("lifespan shutdown")


app = FastAPI(lifespan=lifespan)


@app.get("/health")
def health():
    with MODEL_LOCK:
        return {
            "status": "ok",
            "ready": MODEL_READY,
            "loading": MODEL_LOADING,
            "error": MODEL_ERROR,
        }


@app.post("/api/v1/asr")
async def asr(file: UploadFile = File(...), language: str = Form("auto")):
    model, rich_transcription_postprocess = get_model_runtime()
    suffix = Path(file.filename or "audio.wav").suffix
    if not suffix:
        suffix = ".wav"

    with tempfile.NamedTemporaryFile(delete=False, suffix=suffix) as tmp:
        tmp.write(await file.read())
        tmp_path = tmp.name

    try:
        result = model.generate(
            input=tmp_path,
            cache={},
            language=language,
            use_itn=True,
            batch_size_s=60,
        )
        text = ""
        if isinstance(result, list) and result:
            item = result[0]
            if isinstance(item, dict):
                text = str(item.get("text", ""))
            else:
                text = str(item)
        else:
            text = str(result)
        text = rich_transcription_postprocess(text).strip()
        return {"text": text}
    except Exception as exc:
        raise HTTPException(status_code=500, detail=str(exc))
    finally:
        try:
            os.remove(tmp_path)
        except OSError:
            pass


if __name__ == "__main__":
    import uvicorn

    host = os.getenv("SENSEVOICE_HOST", "127.0.0.1")
    port = int(os.getenv("SENSEVOICE_PORT", "8765"))
    log(f"starting uvicorn on {host}:{port}")
    try:
        uvicorn.run(app, host=host, port=port, log_level="info")
        log("uvicorn exited normally")
    except Exception as exc:
        log(f"uvicorn crashed: {exc}")
        traceback.print_exc()
        raise

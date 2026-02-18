import os
import tempfile
from pathlib import Path

from fastapi import FastAPI, File, Form, HTTPException, UploadFile
from funasr import AutoModel
from funasr.utils.postprocess_utils import rich_transcription_postprocess

MODEL = None


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


def load_model():
    global MODEL
    if MODEL is not None:
        return MODEL

    model_id = os.getenv("SENSEVOICE_MODEL_ID", "iic/SenseVoiceSmall")
    model_dir = os.getenv("SENSEVOICE_MODEL_DIR", "")
    device = resolve_device(os.getenv("SENSEVOICE_DEVICE", "auto"))
    hub = os.getenv("SENSEVOICE_HUB", "hf")

    if model_dir:
        model_root = os.path.abspath(model_dir)
        os.makedirs(model_root, exist_ok=True)
        os.environ["HF_HOME"] = os.path.join(model_root, "hf_home")
        os.environ["MODELSCOPE_CACHE"] = os.path.join(model_root, "ms_cache")

    MODEL = AutoModel(
        model=model_id,
        hub=hub,
        trust_remote_code=True,
        remote_code="./model.py",
        vad_model="fsmn-vad",
        vad_kwargs={"max_single_segment_time": 30000},
        device=device,
    )
    return MODEL


app = FastAPI()


@app.get("/health")
def health():
    return {"status": "ok", "ready": MODEL is not None}


@app.post("/api/v1/asr")
async def asr(file: UploadFile = File(...), language: str = Form("auto")):
    model = load_model()
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
    load_model()
    uvicorn.run(app, host=host, port=port, log_level="warning")

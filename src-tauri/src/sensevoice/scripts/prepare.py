import argparse
import json
import os
import sys
import traceback


HF_DEFAULT_MODEL_ID = "FunAudioLLM/SenseVoiceSmall"
MS_DEFAULT_MODEL_ID = "iic/SenseVoiceSmall"


def get_auto_model():
    from funasr import AutoModel as ImportedAutoModel

    return ImportedAutoModel


def resolve_device(device: str) -> str:
    if device != "auto":
        return device
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


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--model-id", required=True)
    parser.add_argument("--model-dir", required=True)
    parser.add_argument("--device", default="auto")
    parser.add_argument("--hubs", default="hf,ms")
    parser.add_argument("--state-path", required=True)
    args = parser.parse_args()
    auto_model = get_auto_model()

    model_dir = os.path.abspath(args.model_dir)
    os.makedirs(model_dir, exist_ok=True)
    os.environ["HF_HOME"] = os.path.join(model_dir, "hf_home")
    os.environ["MODELSCOPE_CACHE"] = os.path.join(model_dir, "ms_cache")

    selected_device = resolve_device(args.device)
    hubs = [item.strip() for item in args.hubs.split(",") if item.strip()]
    errors = []

    for hub in hubs:
        selected_model_id = resolve_model_id_for_hub(args.model_id, hub)
        try:
            print(f"[sensevoice] trying hub={hub}, model={selected_model_id}, device={selected_device}")
            # trust_remote_code=True 不指定 remote_code，
            # 让 funasr 自动从模型目录解析 model.py（兼容不同版本及下载源）
            model = auto_model(
                model=selected_model_id,
                hub=hub,
                trust_remote_code=True,
                vad_model="fsmn-vad",
                vad_kwargs={"max_single_segment_time": 30000},
                device=selected_device,
            )
            del model
            with open(args.state_path, "w", encoding="utf-8") as fp:
                json.dump(
                    {
                        "hub": hub,
                        "model_id": selected_model_id,
                        "device": selected_device,
                    },
                    fp,
                    ensure_ascii=False,
                    indent=2,
                )
            print(f"[sensevoice] model download complete via hub={hub}")
            return 0
        except Exception as exc:
            detail = f"{hub}: {exc}"
            errors.append(detail)
            print(f"[sensevoice] {detail}", file=sys.stderr)
            traceback.print_exc()

    print("[sensevoice] failed to download model from all hubs", file=sys.stderr)
    for item in errors:
        print(f"[sensevoice] {item}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())

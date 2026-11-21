# asr-bench

用应用同版本的 sherpa-onnx（`=2.0.2`，静态链接）横向对比本地 ASR 引擎的准确率、速度、松键延迟和内存。独立 crate，不参与应用构建。

## 用法

```bash
cd tools/asr-bench

# 1. 下载要测的模型包到任意目录（解压后的目录名见 src/main.rs 的 ENGINES）
mkdir -p /tmp/asr-models && cd /tmp/asr-models
curl -fL https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2 | tar xj
cd -

# 2. 构建（已构建过应用时可复用其预编译库，避免重复下载）
SHERPA_ONNX_LIB_DIR=../../src-tauri/target/sherpa-onnx-prebuilt/sherpa-onnx-v2.0.2-osx-arm64-static-lib/lib \
  cargo build --release

# 3. 用 macOS `say` 合成测试集；可追加真实录音（只测速度）
./gen-testset.sh path/to/real1.wav path/to/real2.wav

# 4. 跑全部已下载的引擎（默认 2 线程，与应用一致；前导静音 0.3s）
./run-all.sh /tmp/asr-models

# 单个引擎并打印识别文本
./target/release/asr-bench --engine sensevoice-int8 --models-dir /tmp/asr-models \
  --manifest data/manifest.tsv --lead-silence 0.3 --show-text
```

## 指标

- `error`：混合分词错误率。汉字逐字、英文逐词，忽略大小写和标点；中文即 CER、英文即 WER、中英混合即 MER。
- `rtf`：总推理耗时 / 音频时长。
- `release`：音频结束（松开快捷键）到拿到最终文本的耗时。离线模型等于整段解码耗时；流式模型按 100ms 分块边说边解码，只计尾部 flush。
- `peak_rss`：进程峰值内存，包含最长音频的解码。

合成语音比真人录音干净，绝对错误率偏低，只适合横向比较。

## 参考结果

2026-11-20，Apple M5 Pro，2 线程，前导静音 0.3s，sherpa-onnx 1.13.8；升级到 2.0.2 后重跑，各引擎错误率不变，耗时差异在 10% 以内。测试集：zh 8 条、中英混合 5 条、en 8 条（TTS），另有 5 条中文录音（含一条 92s 长音频，只测速度）。

| 引擎 | zh | mix | en | RTF | 松键延迟 p50 | 峰值内存 | 包体 |
|---|---|---|---|---|---|---|---|
| SenseVoice int8（现有默认） | 0.7% | 21.1% | 2.1% | 0.056 | 0.24s | 0.9GB | 155MB |
| Parakeet TDT 0.6B v3 int8 | 不支持 | 不支持 | 0.0% | 0.111 | 0.49s | 2.3GB | 465MB |
| Nemotron EN streaming 560ms int8 | 不支持 | 不支持 | 0.0% | 0.317 | 0.17s | 1.1GB | 442MB |
| Nemotron 3.5 streaming 560ms int8 | 10.8% | 26.8% | 4.1% | 0.294 | 0.16s | 1.2GB | 453MB |
| Moonshine v2 tiny-en | 不支持 | 不支持 | 0.0% | 0.010 | 0.05s | 1.5GB | 28MB |
| Moonshine v2 base-en | 不支持 | 不支持 | 0.0% | 0.026 | 0.12s | 1.9GB | 106MB |
| Moonshine v2 base-zh | 4.7% | 33.8% | 9.3% | 0.031 | 0.14s | 1.9GB | 95MB |

已知问题：

- 流式模型（Nemotron）在音频零前导静音时会丢掉第一个词，应用接入时需要在录音前补静音。
- Moonshine v2 不能直接处理长音频：1.13.8 在 92s 音频上报错返回空结果，2.0.2 不再报错但输出大量重复词，需要先切成短段；base-zh 的输出有时在每个汉字间插入空格，需要后处理。
- Nemotron 3.5 指定 `--language zh` 并不比 `auto` 更准（zh 11.5%、mix 31.0%）。
- Moonshine Streaming 目前没有 sherpa-onnx 预打包模型，未纳入测试。

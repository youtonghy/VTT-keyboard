#!/usr/bin/env bash
# 依次以独立进程跑所有已下载的引擎（保证峰值内存互不影响），结果汇总到 data/results.tsv。
# 用法：./run-all.sh <models-dir> [threads] [lead-silence-secs]
# 默认在每段音频前补 0.3 秒静音，模拟按下快捷键到开口的停顿；流式模型在零前导静音时会丢首词。
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
models_dir="${1:?usage: run-all.sh <models-dir> [threads] [lead-silence-secs]}"
threads="${2:-2}"
lead="${3:-0.3}"
bin="$here/target/release/asr-bench"
manifest="$here/data/manifest.tsv"
results="$here/data/results.tsv"

[[ -x "$bin" ]] || { echo "build first: cargo build --release" >&2; exit 1; }
[[ -f "$manifest" ]] || { echo "generate test set first: ./gen-testset.sh" >&2; exit 1; }

printf 'engine\tgroup\tcases\terror\trtf\trelease_p50\trelease_max\n' > "$results"
# --help 以非零状态退出，单独取输出再解析引擎列表
help_text=$("$bin" --help 2>&1 || true)
engines=$(sed -n 's/^engines: //p' <<< "$help_text" | tr -d ',')
for engine in $engines; do
  echo "== $engine"
  if ! output=$("$bin" --engine "$engine" --models-dir "$models_dir" \
      --manifest "$manifest" --threads "$threads" --lead-silence "$lead" 2>&1); then
    echo "   skipped: $(tail -1 <<< "$output")"
    continue
  fi
  grep '^engine=' <<< "$output"
  grep '^RESULT' <<< "$output" | cut -f2- | sed -E 's/(n|err|rtf|release_p50|release_max)=//g' >> "$results"
done

echo
column -t -s $'\t' "$results"

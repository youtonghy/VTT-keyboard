#!/usr/bin/env bash
# 用 macOS `say` 把 testset.tsv 合成为 16kHz 单声道 WAV，并生成基准清单 data/manifest.tsv。
# 合成语音只用于可复现的横向对比，绝对错误率会比真人录音低。
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
out="$here/data"
mkdir -p "$out/wav"
manifest="$out/manifest.tsv"
: > "$manifest"

while IFS=$'\t' read -r id group voice reference; do
  [[ -z "$id" || "$id" == \#* ]] && continue
  wav="$out/wav/$id.wav"
  say -v "$voice" -o "$wav" --file-format=WAVE --data-format=LEI16@16000 "$reference"
  printf 'wav/%s.wav\t%s\t%s\n' "$id" "$group" "$reference" >> "$manifest"
done < "$here/testset.tsv"

# 可选：追加无参考文本的真实录音（只统计速度），每个参数是一个 WAV 路径
for extra in "$@"; do
  printf '%s\treal\t\n' "$(cd "$(dirname "$extra")" && pwd)/$(basename "$extra")" >> "$manifest"
done

echo "wrote $(wc -l < "$manifest" | tr -d ' ') cases to $manifest"

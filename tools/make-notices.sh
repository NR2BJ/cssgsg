#!/usr/bin/env bash
# 앱에 들어가는 다른 소프트웨어·데이터의 고지문을 모은다 → THIRD_PARTY_NOTICES.txt (앱 번들 Resources에도 들어간다)
#   bash tools/make-notices.sh          다시 만든다
#   bash tools/make-notices.sh --check  지금 파일이 최신인지 본다(release.sh가 부른다)
# Mozc 쪽은 build/mozc(tools/mozc/build.sh)와 Bazel 외부 저장소에서 읽는다.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/THIRD_PARTY_NOTICES.txt"
MOZC="$ROOT/build/mozc"
[ -d "$MOZC/src" ] || { echo "build/mozc가 없다. 먼저 bash tools/mozc/build.sh"; exit 1; }
EXT="$(cd "$MOZC/src" && bazelisk info output_base 2>/dev/null)/external"

section() {  # 제목, 주소, 파일
  printf '\n================================================================================\n%s\n%s\n================================================================================\n\n' "$1" "$2"
  cat "$3"
}

TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT
{
  echo "cssgsg (MIT, https://github.com/NR2BJ/cssgsg)에 들어 있는 다른 소프트웨어와 데이터의 저작권·라이선스 고지."
  echo "Third-party notices for software and data included in cssgsg."
  section "Mozc (Japanese input method engine)" "https://github.com/google/mozc  commit $(git -C "$MOZC" rev-parse HEAD)" "$MOZC/LICENSE"
  section "Mozc open source dictionary data (IPAdic, ICOT, Okinawa dictionary)" "https://github.com/google/mozc/tree/master/src/data/dictionary_oss" "$MOZC/src/data/dictionary_oss/README.txt"
  section "Japanese Usage Dictionary" "https://github.com/hiroyuki-komatsu/japanese-usage-dictionary" "$EXT/+http_archive+ja_usage_dict/LICENSE"
  section "Abseil C++" "https://github.com/abseil/abseil-cpp" "$EXT/abseil-cpp+/LICENSE"
  section "Protocol Buffers" "https://github.com/protocolbuffers/protobuf" "$EXT/protobuf+/LICENSE"
  section "zlib" "https://zlib.net" "$EXT/zlib+/LICENSE"
  section "Graphite keyboard layout" "https://github.com/rdavison/graphite-layout" "$ROOT/layouts/en/official/LICENSE"
  section "Shingetsu (新月配列) layout data" "https://github.com/nagamine-git/shingetsu-layout" "$ROOT/layouts/ja/shingetsu/LICENSE"
} > "$TMP"

if [ "${1:-}" = "--check" ]; then
  if cmp -s "$TMP" "$OUT"; then echo "THIRD_PARTY_NOTICES.txt 최신"; else echo "THIRD_PARTY_NOTICES.txt가 낡았다: bash tools/make-notices.sh"; exit 1; fi
else
  cp "$TMP" "$OUT"
  echo "→ $OUT ($(wc -l < "$OUT") 줄)"
fi

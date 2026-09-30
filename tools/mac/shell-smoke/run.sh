#!/usr/bin/env bash
# 맥 셸 스모크 테스트: 셸의 Swift 계층(NSEvent → KeyTranslation → CoreEngine → C ABI)을 검증한다.
#
# 1. 자체 점검: 수식키 좌우·눌림/뗌 판정, NSEvent 시각 단위로 탭 판정, 빠른 탭 전환 보정(잡아 둔 글자와 타이머),
#    자동 반복, ABC 배열 표, 한자 변환(조합 중인 글자 하나: 후보, 취소, 확정, 기호, 조합 없을 때).
# 2. 무작위 키열을 모드마다 러스트 시뮬레이터(cssgsg-cli batch)와 셸 코드에 같이 넣고 화면을 줄마다 비교한다.
#    셸 쪽 글자는 입력기와 같은 TextApplier가 가짜 문서(NSTextInputClient 규칙)에 넣는다. {A-ent}는 한자 변환.
#    글자 → 맥 키코드는 macOS ABC 배열 데이터에서 얻으므로, 코어의 맥 키코드 표도 같이 검증된다.
#
# bash tools/mac/shell-smoke/run.sh [모드별 줄 수, 기본 3000]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
OUT="$ROOT/build/mac-smoke"
N="${1:-3000}"
export PATH="$HOME/.cargo/bin:$PATH"
cd "$ROOT"
cargo build --release -q -p cssgsg-core -p cssgsg-cli
mkdir -p "$OUT"
swiftc -O -module-name shellsmoke \
  -import-objc-header mac/cssgsg/cssgsg-Bridging-Header.h -I core/include \
  mac/Shared/Cssgsg.swift mac/cssgsg/Engine/CoreEngine.swift mac/cssgsg/Engine/KeyTranslation.swift mac/cssgsg/Engine/TextApplier.swift \
  mac/cssgsg/System/TextInputGeometry.swift mac/cssgsg/System/DeveloperLogger.swift \
  tools/mac/shell-smoke/Typist.swift tools/mac/shell-smoke/main.swift \
  -L build/cargo/release -lcssgsg_core -o "$OUT/shell-smoke"

echo "=== 자체 점검 ==="
"$OUT/shell-smoke" --self-test

echo "=== 러스트 시뮬레이터와 비교 (모드별 $N줄 × 입력 조건 4가지) ==="
# 입력 조건: 기기 비트 있음/없음 × flagsChanged 한 번/두 번. macOS 27 IMKit은 비트 없음·두 번이다.
fail=0
for mode in en ko ja; do
  node tools/mac/shell-smoke/corpus.mjs "$mode" "$N" > "$OUT/corpus-$mode.txt"
  build/cargo/release/cssgsg-cli batch --mode "$mode" < "$OUT/corpus-$mode.txt" > "$OUT/sim-$mode.jsonl"
  for variant in "" "--no-device-bits" "--duplicate-flags" "--no-device-bits --duplicate-flags"; do
    name="${variant:-기본}"
    # shellcheck disable=SC2086
    "$OUT/shell-smoke" --mode "$mode" $variant < "$OUT/corpus-$mode.txt" > "$OUT/shell-$mode.jsonl"
    if cmp -s "$OUT/sim-$mode.jsonl" "$OUT/shell-$mode.jsonl"; then
      echo "$mode [$name]: $N줄 모두 같음"
    else
      fail=1
      count=$(paste "$OUT/sim-$mode.jsonl" "$OUT/shell-$mode.jsonl" | awk -F'\t' '$1 != $2' | wc -l | tr -d ' ')
      echo "$mode [$name]: ${count}줄 다름 (키열 / 시뮬레이터 / 셸)"
      paste "$OUT/corpus-$mode.txt" "$OUT/sim-$mode.jsonl" "$OUT/shell-$mode.jsonl" \
        | awk -F'\t' '$2 != $3 { print "  " $1 "\n    sim   " $2 "\n    shell " $3 }' | head -12
    fi
  done
done
exit $fail

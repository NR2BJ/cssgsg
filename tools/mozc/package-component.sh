#!/usr/bin/env bash
# build/mozc-out을 입력기가 받는 Mozc 엔진 묶음으로 만든다(MozcUpdater, NRIME와 같은 모양):
#   build/mozc-component/cssgsg-mozc.zip   libcssgsg_mozc.dylib, mozc.data, manifest.json
#   build/mozc-component/TAG               mozc-<C API 판>-<yyyymmdd>-<커밋 7자리>-w<래퍼 판>, 릴리스 태그
#                                          (래퍼 판이 0이면 -w 없이. 윈도우 묶음도 같은 태그라 한 릴리스에 같이 싣는다)
#   build/mozc-component/TITLE             "Mozc <버전> (<날짜>)"
# manifest.json에는 C API 판, Mozc 커밋·날짜·버전, 래퍼 판, 파일마다 SHA-256이 있다. 입력기는 받은 뒤 이것을 모두 확인한다.
# 래퍼 판이 붙은 태그는 맥 0.7.2와 윈도우 엔진 호스트(0.2.0)부터 읽는다. 그전 판은 이 태그를 건너뛰고 앞의 엔진을 계속 쓴다
# (CONCEPT §6.3).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$ROOT/build/mozc-out"
DEST="$ROOT/build/mozc-component"

read -r COMMIT DATE VERSION WRAPPER < "$OUT/MOZC_VERSION"
WRAPPER="${WRAPPER:-0}"
[[ "$WRAPPER" =~ ^[0-9]{1,6}$ ]] || { echo "MOZC_VERSION의 래퍼 판이 숫자가 아니다: $WRAPPER"; exit 1; }
WRAPPER="$((10#$WRAPPER))"
ABI="$(awk '/^#define CSSGSG_MOZC_ABI_VERSION/ {print $3}' "$ROOT/mozc/cssgsg/cssgsg_mozc.h")"
[ -n "$ABI" ] || { echo "CSSGSG_MOZC_ABI_VERSION을 찾지 못했다"; exit 1; }

rm -rf "$DEST"
mkdir -p "$DEST/stage"
cp "$OUT/lib/libcssgsg_mozc.dylib" "$OUT/data/mozc.data" "$DEST/stage/"
LIB_SHA="$(shasum -a 256 "$DEST/stage/libcssgsg_mozc.dylib" | cut -d' ' -f1)"
DATA_SHA="$(shasum -a 256 "$DEST/stage/mozc.data" | cut -d' ' -f1)"
cat > "$DEST/stage/manifest.json" <<JSON
{"abi": $ABI, "commit": "$COMMIT", "date": "$DATE", "version": "$VERSION", "wrapper": $WRAPPER,
 "files": {"libcssgsg_mozc.dylib": "$LIB_SHA", "mozc.data": "$DATA_SHA"}}
JSON
ditto -c -k --norsrc --noextattr "$DEST/stage" "$DEST/cssgsg-mozc.zip"
rm -rf "$DEST/stage"

TAG="mozc-$ABI-${DATE//-/}-${COMMIT:0:7}"
if [ "$WRAPPER" -gt 0 ]; then TAG="$TAG-w$WRAPPER"; fi
echo "$TAG" > "$DEST/TAG"
echo "Mozc $VERSION ($DATE)" > "$DEST/TITLE"
echo "$(cat "$DEST/TAG"): $(cat "$DEST/TITLE") → $DEST/cssgsg-mozc.zip ($(du -h "$DEST/cssgsg-mozc.zip" | cut -f1))"

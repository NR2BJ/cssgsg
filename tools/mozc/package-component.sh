#!/usr/bin/env bash
# build/mozc-out을 입력기가 받는 Mozc 엔진 묶음으로 만든다(MozcUpdater, NRIME와 같은 모양):
#   build/mozc-component/cssgsg-mozc.zip   libcssgsg_mozc.dylib, mozc.data, manifest.json
#   build/mozc-component/TAG               mozc-<C API 판>-<yyyymmdd>-<커밋 7자리>, 릴리스 태그
#   build/mozc-component/TITLE             "Mozc <버전> (<날짜>)"
# manifest.json에는 C API 판, Mozc 커밋·날짜·버전, 래퍼 판, 파일마다 SHA-256이 있다. 입력기는 받은 뒤 이것을 모두 확인한다.
# 태그는 아직 래퍼 판이 없는 모양이다. 래퍼 판을 넣은 태그(mozc-<판>-<yyyymmdd>-<커밋 7자리>-w<래퍼 판>)는 맥 0.7.2와
# 윈도우 엔진 호스트가 둘 다 읽게 된 뒤에 쓴다(CONCEPT §12).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$ROOT/build/mozc-out"
DEST="$ROOT/build/mozc-component"

read -r COMMIT DATE VERSION WRAPPER < "$OUT/MOZC_VERSION"
WRAPPER="${WRAPPER:-0}"
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

echo "mozc-$ABI-${DATE//-/}-${COMMIT:0:7}" > "$DEST/TAG"
echo "Mozc $VERSION ($DATE)" > "$DEST/TITLE"
echo "$(cat "$DEST/TAG"): $(cat "$DEST/TITLE") → $DEST/cssgsg-mozc.zip ($(du -h "$DEST/cssgsg-mozc.zip" | cut -f1))"

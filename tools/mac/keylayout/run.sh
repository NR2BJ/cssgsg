#!/usr/bin/env bash
# Graphite 자판(mac/KeyboardLayout/cssgsg-Graphite.bundle)을 엔진 데이터로 만든다. --check면 쓰지 않고 최신인지 본다.
# 자판 DTD(/System/Library/DTDs/KeyboardLayout.dtd)로 검사하고, 원작자 배포본(layouts/en/official)과 글자 층을 대조한다.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
OUT="$ROOT/build/keylayout"
export PATH="$HOME/.cargo/bin:$PATH"
cd "$ROOT"
cargo build --release -q -p cssgsg-core
mkdir -p "$OUT"
swiftc -O -module-name keylayout -import-objc-header mac/cssgsg/cssgsg-Bridging-Header.h -I core/include \
  tools/mac/keylayout/main.swift build/cargo/release/libcssgsg_core.a -o "$OUT/keylayout"
"$OUT/keylayout" "$ROOT" "$@"
# xmllint(libxml2)는 XML 1.1의 제어 문자 참조(&#x0004; 등, 애플 자판 파일 표준)를 읽지 못한다. 검사 사본에서만 바꾼다.
sed -e 's/version="1.1"/version="1.0"/' -e 's/&#x00[01][0-9A-F];/?/g' -e 's/&#x007F;/?/g' \
  "mac/KeyboardLayout/cssgsg-Graphite.bundle/Contents/Resources/Graphite (cssgsg).keylayout" > "$OUT/check.keylayout"
xmllint --noout --valid "$OUT/check.keylayout"
plutil -lint -s "mac/KeyboardLayout/cssgsg-Graphite.bundle/Contents/Info.plist"
echo "자판 DTD·Info.plist 검사 통과"

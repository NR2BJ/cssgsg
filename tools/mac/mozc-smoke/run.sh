#!/usr/bin/env bash
# Mozc 엔진 스모크 테스트(main.swift): 입력기의 Mozc 코드(mac/cssgsg/Mozc)와 코어(mozc 기능)를 그대로 붙여 빌드하고,
# build/mozc-out의 엔진과 그것을 묶은 build/mozc-component/cssgsg-mozc.zip으로 돌린다(없거나 낡았으면 묶는다).
# 엔진 워크플로(.github/workflows/mozc-component.yml)도 새로 빌드한 엔진을 내기 전에 이것을 돌린다.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
OUT="$ROOT/build/mac-smoke"
export PATH="$HOME/.cargo/bin:$PATH"
cd "$ROOT"
[ -f build/mozc-out/lib/libcssgsg_mozc.dylib ] || { echo "build/mozc-out이 없다. bash tools/mozc/build.sh를 먼저"; exit 1; }
ZIP=build/mozc-component/cssgsg-mozc.zip
if [ ! -f "$ZIP" ] || [ build/mozc-out/lib/libcssgsg_mozc.dylib -nt "$ZIP" ] || [ build/mozc-out/MOZC_VERSION -nt "$ZIP" ]; then
  bash tools/mozc/package-component.sh
fi
cargo build --release -q -p cssgsg-core --features mozc
mkdir -p "$OUT"
swiftc -O -module-name mozcsmoke \
  -import-objc-header mac/cssgsg/cssgsg-Bridging-Header.h -I core/include \
  mac/Shared/*.swift mac/cssgsg/Mozc/*.swift mac/cssgsg/Engine/CoreEngine.swift mac/cssgsg/System/DeveloperLogger.swift \
  tools/mac/mozc-smoke/main.swift \
  -L build/cargo/release -lcssgsg_core -o "$OUT/mozc-smoke"
"$OUT/mozc-smoke" "$ROOT"

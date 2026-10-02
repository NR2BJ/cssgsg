#!/usr/bin/env bash
# 설정 앱 스모크 테스트: 단축키 녹화(가짜 키 이벤트), 화면 글자, 사전 읽기 정리.
#   bash tools/mac/settings-smoke/run.sh            시험만
#   bash tools/mac/settings-smoke/run.sh --shots    시험 + 탭 × 화면 언어 스냅숏(build/settings-shots/*.png)
# 설정 앱 소스(SettingsApp.swift의 @main만 빼고)와 설정 라이브러리(config-ffi)를 그대로 붙여 빌드한다.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
OUT="$ROOT/build/mac-smoke"
export PATH="$HOME/.cargo/bin:$PATH"
cd "$ROOT"
cargo build --release -q -p cssgsg-config
mkdir -p "$OUT"
SOURCES=()
while IFS= read -r f; do SOURCES+=("$f"); done < <(find mac/Settings -name '*.swift' ! -name SettingsApp.swift | sort)
swiftc -O -module-name settingssmoke \
  -import-objc-header mac/Settings/Settings-Bridging-Header.h -I config-ffi/include \
  mac/Shared/*.swift "${SOURCES[@]}" tools/mac/settings-smoke/main.swift \
  build/cargo/release/libcssgsg_config.a -framework WebKit -o "$OUT/settings-smoke"
if [ "${1:-}" = "--shots" ]; then
  "$OUT/settings-smoke" --shots "$ROOT/build/settings-shots"
else
  "$OUT/settings-smoke"
fi

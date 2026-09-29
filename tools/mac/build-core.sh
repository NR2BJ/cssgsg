#!/usr/bin/env bash
# Xcode 빌드 전에 러스트 코어 정적 라이브러리(build/cargo/release/libcssgsg_core.a)를 만든다.
set -euo pipefail
cd "$(dirname "$0")/../.."
export PATH="$HOME/.cargo/bin:$PATH"
# Mozc(일본어 한자 변환) 엔진: 없거나 C API 소스·고정 커밋이 바뀌었으면 먼저 빌드한다(처음은 몇 분, 그다음은 바뀐 것만).
# 앱이 dylib를 Frameworks에 담는다(mac/project.yml). 코어는 링크하지 않고 실행 중에 읽는다.
LIB=build/mozc-out/lib/libcssgsg_mozc.dylib
if [ ! -f "$LIB" ] || [ ! -f build/mozc-out/data/mozc.data ] || [ ! -f build/mozc-out/MOZC_VERSION ] \
  || [ -n "$(find mozc/cssgsg tools/mozc/MOZC_COMMIT -newer "$LIB" -type f 2>/dev/null)" ]; then
  # Mozc 헤더가 경고를 많이 낸다. 기록은 파일로 두고, 실패하면 끝부분만 보인다.
  if ! bash tools/mozc/build.sh > build/mozc-build.log 2>&1; then
    tail -40 build/mozc-build.log
    exit 1
  fi
fi
cargo build --release -q -p cssgsg-core --features mozc

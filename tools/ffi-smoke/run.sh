#!/usr/bin/env bash
# core·config-ffi 정적 라이브러리를 빌드하고 C 스모크 테스트를 돌린다.
set -euo pipefail
cd "$(dirname "$0")/../.."
export PATH="$HOME/.cargo/bin:$PATH"
cargo build -q -p cssgsg-core
out=build/ffi-smoke
mkdir -p "$out"
clang -std=c11 -Wall -Wextra -Werror -I core/include tools/ffi-smoke/main.c \
  build/cargo/debug/libcssgsg_core.a -o "$out/ffi-smoke"
"$out/ffi-smoke"

# 설정 앱용 라이브러리(config-ffi). 러스트 정적 라이브러리 둘을 한 프로그램에 넣으면 겹치니 따로 만든다.
cargo build -q -p cssgsg-config
clang -std=c11 -Wall -Wextra -Werror -I config-ffi/include tools/ffi-smoke/config.c \
  build/cargo/debug/libcssgsg_config.a -o "$out/config-smoke"
"$out/config-smoke"

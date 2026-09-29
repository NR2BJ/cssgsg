#!/usr/bin/env bash
# core 정적 라이브러리를 빌드하고 C 스모크 테스트를 돌린다.
set -euo pipefail
cd "$(dirname "$0")/../.."
export PATH="$HOME/.cargo/bin:$PATH"
cargo build -q -p cssgsg-core
out=build/ffi-smoke
mkdir -p "$out"
clang -std=c11 -Wall -Wextra -Werror -I core/include tools/ffi-smoke/main.c \
  build/cargo/debug/libcssgsg_core.a -o "$out/ffi-smoke"
"$out/ffi-smoke"

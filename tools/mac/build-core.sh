#!/usr/bin/env bash
# Xcode 빌드 전에 러스트 코어 정적 라이브러리(build/cargo/release/libcssgsg_core.a)를 만든다.
set -euo pipefail
cd "$(dirname "$0")/../.."
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release -q -p cssgsg-core

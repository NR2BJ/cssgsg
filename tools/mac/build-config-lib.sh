#!/usr/bin/env bash
# Xcode 빌드 전에 설정 앱용 러스트 라이브러리(build/cargo/release/libcssgsg_config.a)를 만든다.
# 입력기 코어(Mozc 포함)는 설정 앱에 들어가지 않는다: 이 크레이트만 따로 빌드하면 코어는 mozc 기능 없이 빌드된다.
set -euo pipefail
cd "$(dirname "$0")/../.."
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release -q -p cssgsg-config

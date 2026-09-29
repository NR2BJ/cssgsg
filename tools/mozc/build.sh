#!/usr/bin/env bash
# Mozc(구글 일본어 변환 엔진)를 cssgsg용으로 빌드한다 → build/mozc-out/
#   lib/libcssgsg_mozc.dylib  입력기가 실행 중에 읽는 엔진(Mozc와 의존성 전부 + 우리 C API)
#   include/cssgsg_mozc.h     C API
#   data/mozc.data            사전 데이터(OSS)
#   bin/cssgsg_mozc_main      확인용 명령줄 도구
#   MOZC_VERSION              "<커밋> <커밋 날짜> <Mozc 버전>"(입력기가 앱에 든 엔진의 판을 안다)
#
# - Mozc 커밋은 tools/mozc/MOZC_COMMIT에 고정한다. 환경 변수 MOZC_COMMIT이 있으면 그것(엔진 워크플로가
#   upstream 최신을 빌드할 때). MOZC_BAZEL_FLAGS로 Bazel 옵션을 더한다(워크플로의 --disk_cache).
# - Mozc 소스는 build/mozc에 받는다. ~/Documents는 Syncthing 동기화 폴더라서 build/(동기화 제외)에 둔다.
# - 우리 C API(mozc/cssgsg)는 Mozc 트리의 src/cssgsg로 복사해서 같이 빌드한다.
# - 빌드 도구는 bazelisk(brew install bazelisk). Bazel 버전은 Mozc가 고정한다(src/.bazeliskrc).
# - 처음에는 의존성을 받느라 몇 분 걸린다. 그다음부터는 바뀐 것만 빌드한다.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
MOZC_COMMIT="${MOZC_COMMIT:-$(tr -d '[:space:]' < "$ROOT/tools/mozc/MOZC_COMMIT")}"
SRC="$ROOT/build/mozc"
OUT="$ROOT/build/mozc-out"

command -v bazelisk >/dev/null 2>&1 || { echo "bazelisk가 없다: brew install bazelisk"; exit 1; }
if [ ! -d "$SRC/.git" ]; then
  git clone -q --depth 1 https://github.com/google/mozc.git "$SRC"
fi
if [ "$(git -C "$SRC" rev-parse HEAD)" != "$MOZC_COMMIT" ]; then
  git -C "$SRC" fetch -q --depth 1 origin "$MOZC_COMMIT"
  git -C "$SRC" checkout -q --detach "$MOZC_COMMIT"
fi
rsync -a --delete "$ROOT/mozc/cssgsg/" "$SRC/src/cssgsg/"

cd "$SRC/src"
# Mozc는 세션 처리기 등을 정해진 패키지(ios, server…)에만 보이게 해 둔다. 소스를 고치지 않고 검사만 끈다.
# 앱(mac/project.yml)과 같은 macOS 13 기준으로 빌드한다(안 하면 이 컴퓨터의 macOS 버전이 기준이 된다).
FLAGS=(--config oss_macos --config release_build --nocheck_visibility --macos_minimum_os=13.0)
if [ -n "${MOZC_BAZEL_FLAGS:-}" ]; then
  read -r -a EXTRA <<< "$MOZC_BAZEL_FLAGS"
  FLAGS+=("${EXTRA[@]}")
fi
bazelisk build "${FLAGS[@]}" //cssgsg:libcssgsg_mozc.dylib //cssgsg:cssgsg_mozc_main //data_manager/oss:mozc.data
BIN="$(bazelisk info "${FLAGS[@]}" bazel-bin 2>/dev/null)"

rm -rf "$OUT/lib"
mkdir -p "$OUT/lib" "$OUT/include" "$OUT/data" "$OUT/bin"
cp -f "$BIN/cssgsg/libcssgsg_mozc.dylib" "$OUT/lib/libcssgsg_mozc.dylib"
cp -f "$ROOT/mozc/cssgsg/cssgsg_mozc.h" "$OUT/include/"
cp -f "$BIN/data_manager/oss/mozc.data" "$OUT/data/"
cp -f "$BIN/cssgsg/cssgsg_mozc_main" "$OUT/bin/"
chmod u+w "$OUT"/lib/* "$OUT"/include/* "$OUT"/data/* "$OUT"/bin/*

VERSION="$(awk -F' = ' '/^MAJOR/{a=$2} /^MINOR/{b=$2} /^BUILD_OSS/{c=$2} /^REVISION/{d=$2} END{printf "%s.%s.%s.%d", a, b, c, d + 1}' version.bzl)"
DATE="$(git -C "$SRC" show -s --format=%cs HEAD)"
echo "$MOZC_COMMIT $DATE $VERSION" > "$OUT/MOZC_VERSION"
echo "Mozc $VERSION ($MOZC_COMMIT, $DATE) → $OUT"
ls -la "$OUT/lib" "$OUT/data" | grep -v "^total"

#!/usr/bin/env bash
# 업데이트 코드 스모크 테스트. 셸의 업데이트 코드를 그대로 붙여 빌드하고 돌린다.
#   bash tools/mac/update-smoke/run.sh                     오프라인 확인
#   bash tools/mac/update-smoke/run.sh --live <pkg> <버전>  올라간 릴리스를 앱과 같은 코드로 확인(release.sh가 부른다)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
OUT="$ROOT/build/mac-smoke"
mkdir -p "$OUT"
swiftc -O -module-name updatesmoke \
  "$ROOT/mac/cssgsg/Update/SemanticVersion.swift" "$ROOT/mac/cssgsg/Update/Updater.swift" \
  "$ROOT/tools/mac/update-smoke/main.swift" -o "$OUT/update-smoke"
if [ "${1:-}" = "--live" ]; then
  "$OUT/update-smoke" --live "$2" "$3"
else
  "$OUT/update-smoke" "$ROOT/tools/mac/update-smoke/fixture-release.json"
fi

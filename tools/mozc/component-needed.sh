#!/usr/bin/env bash
# 엔진 워크플로가 새 Mozc 엔진을 빌드할지 정한다. $GITHUB_OUTPUT용으로 찍는다:
#   commit=<upstream master SHA>, build=true|false, reason=<까닭>
#
# upstream Mozc의 버전(src/version.bzl)이나 사전 데이터(src/data/, mozc.data가 된다)가 가장 최근에 낸 엔진 뒤로
# 바뀌었을 때만 빌드한다. 커밋 대부분은 다른 플랫폼·시험·도구를 고칠 뿐이다. 낸 엔진이 아직 없거나 강제일 때도 빌드한다.
# 래퍼 판(mozc/cssgsg/WRAPPER_REVISION)이 가장 최근에 낸 엔진(태그 끝 -w<판>, 없으면 0)보다 크면 upstream이 그대로여도
# 빌드한다: 래퍼만 고친 엔진도 입력기에 가야 한다(CONCEPT §6.3).
# NRIME의 Tools/mozc/component-needed.sh와 같다(래퍼 판은 cssgsg만).
#
# 쓰기: component-needed.sh [force]   (GITHUB_REPOSITORY, GH_TOKEN은 워크플로가 준다)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FORCE="${1:-false}"
REPO="${GITHUB_REPOSITORY:-NR2BJ/cssgsg}"
WRAPPER="$(tr -d '[:space:]' < "$ROOT/mozc/cssgsg/WRAPPER_REVISION")"
[[ "$WRAPPER" =~ ^[0-9]{1,6}$ ]] || { echo "mozc/cssgsg/WRAPPER_REVISION은 숫자여야 한다: $WRAPPER" >&2; exit 1; }

LATEST="$(git ls-remote https://github.com/google/mozc.git refs/heads/master | cut -f1)"
[ -n "$LATEST" ] || { echo "upstream Mozc를 읽지 못했다" >&2; exit 1; }
echo "commit=$LATEST"

LAST_TAG="$(gh release list --repo "$REPO" --limit 100 --json tagName,createdAt \
  --jq '[.[] | select(.tagName | startswith("mozc-"))] | sort_by(.createdAt) | reverse | .[0].tagName // ""')"
if [ "$FORCE" = "true" ]; then
  echo "build=true"; echo "reason=forced"; exit 0
fi
if [ -z "$LAST_TAG" ]; then
  echo "build=true"; echo "reason=no component published yet"; exit 0
fi
# mozc-<판>-<yyyymmdd>-<커밋 7자리>[-w<래퍼 판>]: 래퍼 판을 떼고 커밋을 읽는다.
LAST_BASE="$LAST_TAG"
LAST_WRAPPER=0
if [[ "$LAST_TAG" =~ ^(.*)-w([0-9]+)$ ]]; then
  LAST_BASE="${BASH_REMATCH[1]}"
  LAST_WRAPPER="${BASH_REMATCH[2]}"
fi
LAST_SHA="${LAST_BASE##*-}"
if [ "$((10#$WRAPPER))" -gt "$((10#$LAST_WRAPPER))" ]; then
  echo "build=true"; echo "reason=wrapper revision $WRAPPER is newer than $LAST_TAG"; exit 0
fi
if [ "${LATEST:0:${#LAST_SHA}}" = "$LAST_SHA" ]; then
  echo "build=false"; echo "reason=$LAST_TAG is upstream master"; exit 0
fi

FILES="$(gh api "repos/google/mozc/compare/$LAST_SHA...$LATEST" --jq '.files[].filename')"
COUNT="$(printf '%s\n' "$FILES" | grep -c . || true)"
RELEVANT="$(printf '%s\n' "$FILES" | grep -E '^src/version\.bzl$|^src/data/' | grep -v '^src/data/test/' || true)"
if [ -n "$RELEVANT" ]; then
  echo "build=true"; echo "reason=version or data changed since $LAST_TAG"
elif [ "$COUNT" -ge 300 ]; then
  # compare API는 파일을 300개까지만 보인다. 알 수 없으니 빌드한다.
  echo "build=true"; echo "reason=too many changes since $LAST_TAG to tell"
else
  echo "build=false"; echo "reason=only code outside version and data changed since $LAST_TAG"
fi

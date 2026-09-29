#!/usr/bin/env bash
# 검사를 모두 돌리고 pkg를 만들어 GitHub 릴리스로 올린다(NRIME Tools/release.sh를 따른다).
#
#   bash tools/mac/release.sh 0.1.1 --notes-file docs/releases/v0.1.1.md [--yes]
#
# - 버전에 -가 붙으면(0.2.0-beta.1) prerelease로 올린다. 설정 앱의 베타 채널만 받는다(정식 채널은 /releases/latest).
# - 올라간 코드와 같은 코드로 만들도록, 커밋하지 않았거나 푸시하지 않은 변경이 있으면 멈춘다.
# - 검사를 모두 통과해야 만든다: cargo test·clippy·fmt, FFI·셸·업데이트·설정 앱 스모크, 학습 페이지, Mozc 변환, 고지문.
# - 파일 이름은 cssgsg.pkg로 고정한다. https://github.com/NR2BJ/cssgsg/releases/latest/download/cssgsg.pkg 가 늘 최신이다.
# - 올린 뒤 앱과 같은 코드로 최신 릴리스를 읽고, 파일을 받아 해시를 맞춰 본다.
# - CSSGSG_COMMIT_TRAILER가 있으면 버전 커밋 메시지 끝에 붙인다.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
REPO="NR2BJ/cssgsg"
VERSION=""
NOTES_FILE=""
ASSUME_YES=0

while [ $# -gt 0 ]; do
  case "$1" in
    --notes-file) NOTES_FILE="${2:-}"; [ -n "$NOTES_FILE" ] || { echo "--notes-file에 파일이 없다"; exit 1; }; shift 2 ;;
    --yes|-y) ASSUME_YES=1; shift ;;
    -*) echo "알 수 없는 옵션: $1"; exit 1 ;;
    *) [ -z "$VERSION" ] || { echo "버전을 두 번 줬다 ($VERSION, $1)"; exit 1; }; VERSION="$1"; shift ;;
  esac
done
[ -n "$VERSION" ] || { echo "사용법: bash tools/mac/release.sh <버전> [--notes-file 파일] [--yes]"; exit 1; }
VERSION="${VERSION#v}"
if ! printf '%s' "$VERSION" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$'; then
  echo "버전 형식이 아니다: $VERSION (예: 0.1.1, 0.2.0-beta.1)"
  exit 1
fi
case "$VERSION" in
  *-*) PRERELEASE=1 ;;
  *) PRERELEASE=0 ;;
esac
TAG="v$VERSION"
cd "$ROOT"

echo "=== cssgsg 릴리스 $TAG $([ $PRERELEASE = 1 ] && echo "(prerelease)") ==="
command -v gh >/dev/null || { echo "gh가 없다"; exit 1; }
gh auth status >/dev/null 2>&1 || { echo "gh 로그인이 안 돼 있다(gh auth login)"; exit 1; }
if gh release view "$TAG" --repo "$REPO" >/dev/null 2>&1; then
  echo "$TAG 릴리스가 이미 있다. 버전을 올린다(같은 버전을 다시 올리지 않는다)."
  exit 1
fi
if [ -n "$NOTES_FILE" ] && [ ! -f "$NOTES_FILE" ]; then echo "릴리스 노트 파일이 없다: $NOTES_FILE"; exit 1; fi
if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
  echo "커밋하지 않은 변경이 있다:"
  git status --short --untracked-files=no
  exit 1
fi
BRANCH="$(git rev-parse --abbrev-ref HEAD)"
git fetch -q origin "$BRANCH"
if [ -n "$(git log --oneline "origin/$BRANCH..HEAD")" ]; then echo "푸시하지 않은 커밋이 있다"; exit 1; fi

echo "=== 검사 ==="
export PATH="$HOME/.cargo/bin:$PATH"
LOG="$ROOT/build/release-checks.log"
mkdir -p "$ROOT/build"
: >"$LOG"
step() {
  local name="$1"
  shift
  if "$@" >>"$LOG" 2>&1; then
    echo "  ok   $name"
  else
    echo "  FAIL $name (전체 기록: $LOG)"
    tail -30 "$LOG"
    exit 1
  fi
}
step "cargo test" cargo test -q --workspace
step "cargo clippy" cargo clippy -q --workspace --all-targets -- -D warnings
step "cargo fmt" cargo fmt --check
step "FFI 스모크" bash tools/ffi-smoke/run.sh
step "셸 스모크" bash tools/mac/shell-smoke/run.sh
step "업데이트 스모크" bash tools/mac/update-smoke/run.sh
step "설정 앱 스모크" bash tools/mac/settings-smoke/run.sh
step "학습 페이지" node tools/learn/build.mjs --check
step "Mozc 빌드" bash tools/mozc/build.sh
step "Mozc 변환" cargo test -q -p cssgsg-core --features mozc --test mozc
step "Mozc 엔진 스모크" bash tools/mac/mozc-smoke/run.sh
step "고지문" bash tools/make-notices.sh --check

if [ "$ASSUME_YES" -ne 1 ]; then
  printf "%s를 공개 릴리스로 올린다. 계속할까? [y/N] " "$TAG"
  read -r reply
  case "$reply" in [yY]|[yY][eE][sS]) ;; *) echo "그만둔다"; exit 1 ;; esac
fi

echo "=== 버전 $VERSION ==="
CURRENT_BUILD="$(grep -E '^[[:space:]]+CURRENT_PROJECT_VERSION:' mac/project.yml | head -1 | sed -E 's/.*"([0-9]+)".*/\1/')"
NEXT_BUILD=$((CURRENT_BUILD + 1))
/usr/bin/sed -i '' -E \
  -e "s/^([[:space:]]+MARKETING_VERSION:).*/\1 \"$VERSION\"/" \
  -e "s/^([[:space:]]+CURRENT_PROJECT_VERSION:).*/\1 \"$NEXT_BUILD\"/" \
  mac/project.yml
grep -E "MARKETING_VERSION|CURRENT_PROJECT_VERSION" mac/project.yml | sed 's/^/  /'

bash tools/mac/build-pkg.sh
PKG="$ROOT/build/pkg/cssgsg-$VERSION.pkg"
[ -f "$PKG" ] || { echo "pkg가 없다: $PKG"; exit 1; }

MESSAGE="Release $VERSION"
[ -n "${CSSGSG_COMMIT_TRAILER:-}" ] && MESSAGE="$MESSAGE"$'\n\n'"$CSSGSG_COMMIT_TRAILER"
git add mac/project.yml
git commit -q -m "$MESSAGE"
git push -q origin "$BRANCH"
echo "버전 커밋을 올렸다: $(git rev-parse --short HEAD)"

echo "=== GitHub 릴리스 ==="
UPLOAD="$ROOT/build/pkg/upload"
rm -rf "$UPLOAD"
mkdir -p "$UPLOAD"
cp "$PKG" "$UPLOAD/cssgsg.pkg"
ARGS=("$TAG" "$UPLOAD/cssgsg.pkg" --repo "$REPO" --title "cssgsg $VERSION" --target "$(git rev-parse HEAD)")
[ "$PRERELEASE" -eq 1 ] && ARGS+=(--prerelease)
if [ -n "$NOTES_FILE" ]; then ARGS+=(--notes-file "$NOTES_FILE"); else ARGS+=(--generate-notes); fi
gh release create "${ARGS[@]}"

if [ "$PRERELEASE" -eq 0 ]; then
  echo "=== 올라간 릴리스 확인(앱과 같은 코드) ==="
  # 릴리스 목록(/releases, 베타 채널)은 GitHub가 최대 60초 캐시한다(0.5.1: 30초 안에 안 보였다). 2분까지 기다린다.
  for attempt in 1 2 3 4 5 6 7 8 9 10 11 12; do
    if "$ROOT/build/mac-smoke/update-smoke" --live "$PKG" "$VERSION"; then
      break
    fi
    [ "$attempt" -lt 12 ] || { echo "올라간 릴리스 확인 실패"; exit 1; }
    echo "  GitHub 반영을 기다린다…"
    sleep 10
  done
fi
echo "완료: https://github.com/$REPO/releases/tag/$TAG"

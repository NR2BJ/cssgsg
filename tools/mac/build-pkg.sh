#!/usr/bin/env bash
# cssgsg 설치 pkg를 만든다 → build/pkg/cssgsg-<버전>.pkg (버전은 mac/project.yml의 MARKETING_VERSION)
#
# NRIME(Tools/build_pkg.sh)에서 겪은 것을 따른다.
# - 서명은 빌드 때 한다(postinstall에서 하면 권한 창이 뜬다). 안에 든 번들이 없어서 앱 하나만 서명한다(--deep 금지).
# - cp -R 대신 ditto(._* 파일이 섞이지 않게).
# - BundleIsRelocatable=false. 기본값(true)이면 설치기가 디스크 전체에서 옛 번들을 찾다가 Documents 권한 창을 띄운다.
#
# 서명은 "cssgsg Code Signing" 인증서로 한다(tools/mac/make-signing-identity.sh). 없으면 멈춘다.
# 인증서 없는 컴퓨터에서 시험 삼아 만들려면 CSSGSG_SIGN=adhoc (그 pkg로 업데이트하면 손쉬운 사용 권한이 풀린다).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$ROOT/build/pkg"
WORK="$OUT/work"
BUILT="$ROOT/build/mac/Release/cssgsg.app"
IDENTITY_NAME="cssgsg Code Signing"

echo "=== 빌드 ==="
cd "$ROOT/mac"
xcodegen generate --quiet
mkdir -p "$OUT"
# Xcode가 이 macOS에서 CoreDevice·시뮬레이터 경고를 잔뜩 내서 기록은 파일로 남긴다.
if ! xcodebuild -project cssgsg.xcodeproj -scheme cssgsg -configuration Release \
  SYMROOT="$ROOT/build/mac" build >"$OUT/xcodebuild.log" 2>&1; then
  grep -E "error:" "$OUT/xcodebuild.log" | head -20
  echo "빌드 실패. 전체 기록: $OUT/xcodebuild.log"
  exit 1
fi
[ -d "$BUILT" ] || { echo "빌드 결과가 없다: $BUILT"; exit 1; }
VERSION="$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$BUILT/Contents/Info.plist")"
echo "버전 $VERSION"

echo "=== 담을 것 준비 ==="
rm -rf "$WORK"
mkdir -p "$WORK/payload/Library/Input Methods" "$WORK/scripts"
APP="$WORK/payload/Library/Input Methods/cssgsg.app"
ditto "$BUILT" "$APP"
find "$WORK/payload" -name ".syncthing.*" -delete
cp "$ROOT/tools/mac/pkg/postinstall" "$WORK/scripts/postinstall"
chmod +x "$WORK/scripts/postinstall"

echo "=== 서명 ==="
if [ "${CSSGSG_SIGN:-}" = "adhoc" ]; then
  SIGN_ID="-"
  echo "ad-hoc 서명(시험용)"
else
  SIGN_ID="$(security find-identity -p codesigning | awk -v n="\"$IDENTITY_NAME\"" 'index($0, n) {print $2; exit}')"
  if [ -z "$SIGN_ID" ]; then
    echo "서명 인증서 \"$IDENTITY_NAME\"이 없다. bash tools/mac/make-signing-identity.sh 로 만든다."
    exit 1
  fi
fi
codesign --force --sign "$SIGN_ID" --timestamp=none "$APP"
codesign --verify --strict "$APP"
REQUIREMENT="$(codesign -d -r- "$APP" 2>&1 | grep designated)"
echo "  $REQUIREMENT"
if [ "$SIGN_ID" != "-" ] && ! grep -q "certificate leaf" <<<"$REQUIREMENT"; then
  echo "서명 요구조건에 인증서가 없다. 업데이트 때 권한이 풀린다."
  exit 1
fi

echo "=== pkg ==="
pkgbuild --analyze --root "$WORK/payload" "$WORK/component.plist" >/dev/null
# 요즘 pkgbuild는 이 키를 빼고 내보낸다(빠지면 기본값 = 재배치 허용). 번들마다 false를 직접 넣는다.
i=0
while /usr/libexec/PlistBuddy -c "Print :$i:RootRelativeBundlePath" "$WORK/component.plist" >/dev/null 2>&1; do
  /usr/libexec/PlistBuddy -c "Delete :$i:BundleIsRelocatable" "$WORK/component.plist" >/dev/null 2>&1 || true
  /usr/libexec/PlistBuddy -c "Add :$i:BundleIsRelocatable bool false" "$WORK/component.plist"
  i=$((i + 1))
done
[ "$i" -gt 0 ] || { echo "component.plist에 번들이 없다"; exit 1; }
echo "  재배치 끔: 번들 ${i}개"
pkgbuild --quiet --root "$WORK/payload" --scripts "$WORK/scripts" --component-plist "$WORK/component.plist" \
  --identifier com.cssgsg.inputmethod.pkg --version "$VERSION" --install-location / \
  "$WORK/cssgsg-component.pkg"
sed "s/__VERSION__/$VERSION/g" "$ROOT/tools/mac/pkg/distribution.xml" > "$WORK/distribution.xml"
PKG="$OUT/cssgsg-$VERSION.pkg"
rm -f "$PKG"
productbuild --quiet --distribution "$WORK/distribution.xml" --package-path "$WORK" "$PKG"
rm -rf "$WORK"
echo "완료: $PKG ($(du -h "$PKG" | cut -f1))"

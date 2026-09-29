#!/usr/bin/env bash
# cssgsg 입력기를 빌드해서 ~/Library/Input Methods에 설치한다. NRIME와 번들 ID가 달라서 같이 설치된다.
#
# 처음 설치하면 입력 소스 목록에 안 보일 수 있다. 그러면 로그아웃했다 다시 로그인한 뒤
# 시스템 설정 → 키보드 → 입력 소스 → 편집 → + 에서 cssgsg를 추가한다.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
APP_NAME="cssgsg.app"
DEST="$HOME/Library/Input Methods"
SOURCE_ID="com.cssgsg.inputmethod.app.en"

echo "=== 학습 페이지 확인 ==="
node "$ROOT/tools/learn/build.mjs" --check

echo "=== 빌드 ==="
cd "$ROOT/mac"
xcodegen generate --quiet
xcodebuild -project cssgsg.xcodeproj -scheme cssgsg -configuration Release \
  SYMROOT="$ROOT/build/mac" build -quiet

echo "=== 설치: $DEST/$APP_NAME ==="
mkdir -p "$DEST"
rm -rf "$DEST/$APP_NAME"
cp -R "$ROOT/build/mac/Release/$APP_NAME" "$DEST/"
# cp한 파일에 붙는 격리 속성을 지운다(안 지우면 입력 소스 목록에서 흐리게 나온다)
xattr -cr "$DEST/$APP_NAME"
chmod +x "$DEST/$APP_NAME/Contents/MacOS/cssgsg"

LSREGISTER="/System/Library/Frameworks/CoreServices.framework/Versions/Current/Frameworks/LaunchServices.framework/Versions/Current/Support/lsregister"
"$LSREGISTER" -f "$DEST/$APP_NAME"

# 설치한 새 바이너리로 다시 뜨게 한다
killall cssgsg 2>/dev/null || true

echo "=== 입력 소스 등록 ==="
swift - "$DEST/$APP_NAME" "$SOURCE_ID" <<'SWIFT'
import Carbon
import Foundation
let appURL = URL(fileURLWithPath: CommandLine.arguments[1]) as CFURL
let sourceID = CommandLine.arguments[2]
let reg = TISRegisterInputSource(appURL)
print("  등록: \(reg == noErr ? "성공" : "실패(\(reg))")")
let cond = [kTISPropertyInputSourceID: sourceID] as CFDictionary
guard let list = TISCreateInputSourceList(cond, true)?.takeRetainedValue() as? [TISInputSource], let src = list.first else {
    print("  입력 소스를 아직 찾을 수 없다. 로그아웃했다 다시 로그인한 뒤 시스템 설정에서 추가한다.")
    exit(0)
}
let en = TISEnableInputSource(src)
print("  켜기: \(en == noErr ? "성공 (입력 메뉴에서 cssgsg를 고른다)" : "실패(\(en))")")
SWIFT
echo "완료"

# cssgsg
미친놈을 위한 정신나간 입력기

**C**ham**S**hin **S**ebeolsik(참신세벌식) + **G**raphite + **S**hin**G**etsu(新月配列).
세 배열을 입력 소스 하나에서 전환하며 쓰는 한/영/일 올인원 입력기다. macOS를 먼저 만들었고, 지금 Windows를 만든다.
설계 문서는 [CONCEPT.md](CONCEPT.md).

## 상태

- [x] 코어(Rust, 플랫폼 독립)
  - 참신세벌식 v18(기본)·D v19 갈마들이 조합기. 키 단위 Backspace, ❖ 음절 조합 중지
  - Graphite: OS 레이아웃은 쿼티 그대로 두고 입력기가 글자만 바꾼다. 단축키는 쿼티 자리
  - 新月配列 가나 조합기: ☆/★ 앞치기, ゛ 뒤치기, 3타 단축, Caps Lock 가타카나
  - 모드 전환: 오른쪽 Shift 탭 = 영어 ↔ 직전 비영어, 왼쪽 Shift 탭 = 한 ↔ 일 (NRIME 설정 그대로).
    단축키는 설정 앱에서 녹화해 바꾼다(수식키 탭 또는 수식키+키 조합, 수식키 좌우를 가린다). 입력기를 켜면 한국어로 시작한다
  - C ABI(`core/include/cssgsg.h`)와 C 스모크 테스트
- [~] macOS 입력기 셸 (NRIME 플랫폼층 이식)
  - [x] M2a: IMKit 셸 뼈대. 키 → 코어 → 앱, 메뉴 막대 모드 표시(G/ㅊ/月), 조합 중 밑줄, 후보창
    - NRIME 우회책: ⌘/Ctrl 조합 확정 뒤 키 재전송, Chromium 조합 중 Shift+Enter, 마우스 클릭 확정, 비밀번호 칸은 쿼티
    - 셸 Swift 계층은 스모크 테스트로 검증했다. 실제 앱에서 쳐 보는 확인은 [확인 목록](docs/mac-checklist.md)으로 한다
  - [x] 배포: pkg + GitHub 릴리스 + 설정 앱 정보 탭의 업데이트(정식·베타 채널, NRIME 방식). 자체 서명 인증서로 서명해서 업데이트해도 권한이 남는다
  - [~] M2b: 모드 HUD(모드를 바꿀 때 커서 근처에 G/ㅊ/月, 커서를 모르면 마우스 옆, 설정 `[mac]`) 완료. 입력 소스 복구, 비밀번호 칸 Graphite(나중에)
  - [x] M2c: 설정 앱(0.4.0, 0.5.0에서 다시 짬). 메뉴 막대 cssgsg → "설정…"(메뉴는 설정·다시 시작·종료뿐). 화면 언어 한국어·English·日本語
    - 일반: 단축키 녹화(영어 ↔ 비영어, 한 ↔ 일, 한자), 탭 인식 시간, 빠른 탭 전환 보정(실험적), 후보 글자 크기,
      Shift+Enter 줄바꿈 대기(줄바꿈 넣기·⌘ 단축키, Shift+Enter 다시 보내기, 0부터)와 Shift+Enter를 다시 보낼 앱 목록(기본 Codex), HUD
    - 한국어: 배열, 한자 기억. 일본어: 변환 키(Space·Tab), 기호(구두점·공백 전각/반각·・·¥), 가타카나, 개인 사전(Mozc),
      변환 엔진(Mozc 버전·업데이트), 변환 단축키 설명서
    - 배열 학습(배열마다 따로), 타자 연습(아래), 정보(버전, 입력 소스, 입력기 권한, 업데이트, 화면 언어, 파일)
    - 바꾸면 입력기가 바로 다시 읽는다(다시 시작 없음). 원본은 늘 `config.toml`이고 직접 고쳐도 된다
- [x] 일본어 한자 변환: Mozc를 입력기 프로세스 안에서 쓴다(0.2.0, [mozc/README.md](mozc/README.md)).
  0.6.0부터 엔진은 실행 중에 읽는 dylib이고, 입력기와 따로 업데이트된다(아래 "Mozc 엔진 업데이트")
- [x] 한국어 한자 변환(0.3.1, 왼쪽 Option+Return(설정에서 바꾼다), [CONCEPT.md](CONCEPT.md) §5.4): 조합 중인 글자 하나를 libhangul 사전으로 바꾼다.
  후보창에 뜻(國 나라 국)을 같이 보이고, 자음 하나 + Option+Return은 기호표(ㅁ → ※☆★…). 고른 후보를 기억한다
- [x] 타자 연습(0.7.0부터 설정 앱 타자 연습 탭, 창을 키우거나 전체 화면으로 쓴다): 세 배열을 단계별로 익힌다.
  참신은 받침 없이 홈열 → 윗열 → 아랫열 → 겹모음·된소리, 받침도 같은 순서, 그다음 규칙(ㅗ·ㅜ, 문장부호, ❖).
  단계마다 자리 익히기 → 낱말 치기 → 줄 치기. 틀리면 넘어가지 않고, 모르는 키는 멈추면 보여 준다.
  기록은 `~/Library/Application Support/cssgsg/practice.json`. 같은 페이지(`practice/index.html`)를 브라우저로 열어도 된다(기록은 그 브라우저에)
- [~] Windows TSF 입력기(로드맵 3단계, 윈도우 11 VM에서 개발)
  - [x] M3a: TSF 텍스트 서비스 뼈대(`win/tip`, Rust + windows 크레이트). 글자 키 → Graphite 글자 바로 확정, 단축키는 쿼티 자리,
    등록·해제(`tools/win/tip-dev.ps1`). 패닉·FP 환경(MXCSR)이 앱으로 새지 않게 막는다. 실제 확인은 [확인 목록](docs/win-checklist.md)
  - [~] M3b 코어 엔진 연결: 맥과 같은 코어로 참신·Graphite·新月을 친다. 조합 밑줄(TSF 조합·표시 속성), Shift 톡 전환,
    작업 표시줄 모드 아이콘(G/ㅊ/月, 밝은·어두운 작업 표시줄), 앱 사이에서 같은 모드(TSF 전역 칸). 개발자 기록은 HKCU\Software\cssgsg `DebugLog`=1
  - [~] M3c: 후보창(맥과 같은 목록 9개·격자 5×6·뜻·페이지)과 모드 HUD(모드를 바꿀 때 커서 위 G/ㅊ/月)를 앱 안에서 그린다.
    한국어 한자는 오른쪽 Ctrl 톡(윈도우 기본, 한국 키보드의 한자 키 자리).
    비밀번호 칸은 모드와 상관없이 Graphite로 친다(단축키는 쿼티 자리, 맥보다 앞섬). 남은 것: UILess(게임), 게임 모드, 콘솔
  - [ ] M3d Mozc 일본어 변환 → M3e 설치기·설정 앱(WinUI 3)

## 구조

```
core/       Rust 코어: key, hangul, hanja, latin, kana, engine, ffi, sim
layouts/    배열 데이터 (ko: 참신세벌식 TOML, en: Graphite, ja: 新月 공식 TSV 원본)
dict/       사전 원본 (ko: libhangul 한자·기호 사전, 고치지 않고 코어에 넣는다)
cli/        cssgsg-cli: 터미널에서 쳐보는 도구
config-ffi/ 설정 앱용 C ABI: config.toml ↔ JSON(코어 Config를 그대로 쓴다. 설정 앱은 입력기 코어 대신 이것만 링크)
mac/        macOS 입력기(cssgsg/, Swift + InputMethodKit)와 설정 앱(Settings/, SwiftUI), 둘이 같이 쓰는 Shared/.
            project.yml → xcodegen → cssgsg.xcodeproj(생성물, 커밋 안 함)
win/        윈도우 입력기: tip/(TSF 텍스트 서비스 DLL, Rust + windows 크레이트). 맥에서는 빈 크레이트로 빌드된다
mozc/       Mozc C API 래퍼(cssgsg용, libcssgsg_mozc.dylib). 소스·빌드는 build/mozc, build/mozc-out(tools/mozc/build.sh)
.github/    Mozc 엔진 워크플로(mozc-component.yml): upstream Mozc가 바뀌면 GitHub에서 엔진을 빌드해 낸다
tools/      ffi-smoke(C 헤더 확인), ohi-oracle(오이 차분 비교), crosscheck(배열 데이터·키 코드 표 교차 검증),
            mac(pkg·릴리스·서명 스크립트, 셸·업데이트·설정 앱·Mozc 엔진 스모크 테스트), mozc(엔진 빌드·묶기),
            win(입력기 개발용 설치·제거, 입력칸 시험 페이지), learn(학습 페이지 빌드), practice(타자 연습 페이지 빌드·연습 글)
docs/       verification.md(검증 현황), mac-checklist.md·win-checklist.md(설치 뒤 확인 목록), releases/(릴리스 노트)
learn/      배열 학습 페이지(설정 앱 학습 탭). template.html → tools/learn/build.mjs → index.html
practice/   타자 연습 페이지(세 언어, 단일 HTML). template.html + tools/practice/*.txt → tools/practice/build.mjs → index.html
```

## 개발

Rust는 rustup으로 설치했다(셸 PATH는 건드리지 않음). 버전은 `rust-toolchain.toml`에 고정돼 있어서 rustup이 알아서 맞춘다.
`export PATH="$HOME/.cargo/bin:$PATH"` 후:

```bash
cargo test
cargo run -q -p cssgsg-cli -- type "jfsmtdhfleja"          # 안녕하세요 (한국어 모드가 기본)
cargo run -q -p cssgsg-cli -- type --mode en "jlwwi"       # hello (Graphite)
cargo run -q -p cssgsg-cli -- type --mode ja "ckeuwl"      # にほんご
cargo run -q -p cssgsg-cli -- repl                          # 한 줄씩 쳐 가며 상태 보기
bash tools/ffi-smoke/run.sh
```

### macOS 입력기 설치·업데이트

처음 한 번, 터미널에서(관리자 암호를 묻는다):

```bash
curl -fL -o /tmp/cssgsg.pkg https://github.com/NR2BJ/cssgsg/releases/latest/download/cssgsg.pkg && sudo installer -pkg /tmp/cssgsg.pkg -target / && open -g "/Library/Input Methods/cssgsg.app"
```

- `/Library/Input Methods/cssgsg.app`에 설치되고, 메뉴 막대에 cssgsg 메뉴(G / ㅊ / 月)가 뜨고, 키보드 설정이 한 번 열린다.
  NRIME와 번들 ID가 달라서 같이 설치된다.
- **처음 설치한 뒤 로그아웃했다 다시 로그인한다.** 새로 설치한 입력기는 그래야 추가 목록에 나온다(macOS, 확인함).
- **입력 소스에는 직접 추가한다.** macOS는 입력기가 스스로 입력 소스에 추가되지 못하게 한다.
  시스템 설정 → 키보드 → 텍스트 입력 → 입력 소스 "편집…" → 왼쪽 아래 + → **영어** → cssgsg → 추가.
  추가했는지는 설정 앱 정보 탭에 보인다.
- 브라우저로 받은 pkg는 서명이 없어서 Gatekeeper가 막는다. 그때는 시스템 설정 → 개인정보 보호 및 보안에서 "그래도 열기"를 누른다(위 명령은 해당 없음).
- 업데이트: 메뉴 막대 cssgsg → 설정… → 정보 탭. 정식·베타 채널을 고르고(베타는 시험판도 받는다), 탭을 열 때마다 저절로 확인한다.
  관리자 암호를 한 번 묻고, 끝나면 입력기와 설정 앱이 다시 뜬다. 로그아웃은 필요 없다. (0.4.0까지는 메뉴 막대 메뉴의 "업데이트 확인")
- 입력기 권한: 시스템 설정 → 개인정보 보호 및 보안 → 손쉬운 사용(macOS 27: 기기 제어 및 데이터 접근)에서 cssgsg를 켠다.
  없어도 입력은 되고, 조합을 확정한 뒤 ⌘+키 재전송과 Codex 줄바꿈만 빠진다. 입력기는 스스로 묻지 않는다(0.6.0부터).
  설정 앱 정보 탭 "입력기 권한"에 입력기가 마지막으로 확인한 상태와 "다시 확인 / 권한 요청"·"시스템 설정 열기" 단추가 있다.
  릴리스는 고정 인증서로 서명하므로 업데이트해도 권한이 남는다. 켜져 있는데도 꺼짐으로 보이면 목록에서 지우고(−) 다시 추가한다.

#### Mozc 엔진 업데이트

Mozc(일본어 한자 변환 엔진)는 cssgsg와 따로 업데이트된다(0.6.0, NRIME 1.0.12와 같다).

- `.github/workflows/mozc-component.yml`이 매주 월요일 upstream Mozc를 보고, 버전이나 사전 데이터가 바뀌었으면 GitHub 컴퓨터에서 엔진을
  빌드하고 시험한 뒤 `mozc-<C API 판>-<yyyymmdd>-<커밋 7자리>` 태그의 prerelease로 `cssgsg-mozc.zip`을 낸다(손으로도 돌린다).
  앱 업데이트는 이 릴리스를 보지 않는다(pkg가 없다).
- 입력기는 뜨고 5분 뒤, 그다음은 하루 한 번 GitHub 릴리스 목록을 확인한다(보내는 것은 목록 요청뿐). 더 새 엔진이 있으면 받아서
  GitHub가 적은 SHA-256과 묶음 안 파일별 해시를 확인하고 `~/Library/Application Support/cssgsg/mozc-engines/<커밋>/`에 둔다.
  다음에 입력기가 뜰 때부터 쓴다. 설정 앱 → 일본어 → 변환 엔진에서 지금 확인하고 지금 적용할 수 있다.
- 받은 엔진이 읽히지 않거나, 읽다가 입력기가 죽거나, 10분 안에 깨끗이 끝나지 않은 시작이 세 번이면 그 엔진은 다시 쓰지 않고
  앱에 든 엔진을 쓴다. 학습과 개인 사전(`~/Library/Application Support/cssgsg/mozc/`)은 엔진이 바뀌어도 그대로다.

문제를 재현하려면 개발자 기록을 켠다(키 코드·수식키·시각만 남고 글자 내용은 남지 않는다).

```bash
defaults write com.cssgsg.inputmethod.app developerMode -bool true   # 기록: ~/Library/Application Support/cssgsg/developer.log
```

### 릴리스 만들기(개발)

Xcode, xcodegen·bazelisk(`brew install xcodegen bazelisk`), gh가 필요하다. 서명 인증서는 한 번 만든다(로그인 키체인, 20년).
Mozc는 `tools/mac/build-core.sh`가 없으면 먼저 빌드한다(처음 3~4분). 앱에 들어가는 오픈소스 고지문은 `THIRD_PARTY_NOTICES.txt`(`bash tools/make-notices.sh`).

```bash
bash tools/mac/make-signing-identity.sh                                  # "cssgsg Code Signing" 인증서, 이미 있으면 그대로
bash tools/mac/build-pkg.sh                                              # build/pkg/cssgsg-<버전>.pkg 만들기만
bash tools/mac/release.sh 0.1.1 --notes-file docs/releases/v0.1.1.md    # 검사 → 버전 올림 → pkg → GitHub 릴리스 → 확인
```

`release.sh`는 커밋·푸시하지 않은 변경이 있거나 검사가 하나라도 실패하면 멈춘다. 설치본을 바꾸는 길은 pkg 하나뿐이다
(스크립트로 `~/Library`에 따로 깔면 같은 번들 ID가 두 곳에 생겨 입력 소스가 꼬인다).

### 윈도우 입력기(개발)

윈도우 11 VM에서 한다(스마트 앱 컨트롤 끔, 입력기를 처음 등록하기 전에 VM 스냅숏). 도구는 VS 빌드 도구(C++, Windows SDK, ATL), rustup, Node.js, GitHub CLI.
C 런타임은 정적으로 링크해서(`.cargo/config.toml`) 입력기 DLL은 시스템 DLL에만 기댄다.

```powershell
powershell -ExecutionPolicy Bypass -File tools\win\tip-dev.ps1 install     # 빌드 → Program Files\cssgsg → 등록(관리자 창) → 내 입력 목록(en-US)에 추가
powershell -ExecutionPolicy Bypass -File tools\win\tip-dev.ps1 uninstall   # 목록에서 빼기 → 등록 해제 → 파일·흔적 지우기(쓰는 중인 DLL은 다시 시작 때)
powershell -ExecutionPolicy Bypass -File tools\win\tip-dev.ps1 status
powershell -ExecutionPolicy Bypass -File tools\win\tip-dev.ps1 debuglog-on  # 개발자 기록 %LOCALAPPDATA%\cssgsg\tip-debug.log(키 코드·길이만), debuglog-off로 끔
```

입력기는 en-US 프로필 하나로 등록한다(기반 배치는 US 그대로, [CONCEPT.md](CONCEPT.md) §10.2). 고르는 곳은 Win+Space다.
윈도우 설정에서는 시간 및 언어 → 언어 및 지역 → 영어(미국) → 언어 옵션 → 키보드에 cssgsg로 보인다(언어 팩은 받지 않는다).
그 언어의 키보드가 cssgsg 하나뿐이면 윈도우가 키보드 제거를 막으므로 영어(미국) 언어째 지운다. 한자 단축키는 윈도우 기본이 오른쪽 Ctrl 톡이다
(Alt 조합은 앱 메뉴가 먼저 가져가서 쓸 수 없다).

이 스크립트의 사용자 단계(입력 목록, HKCU 정리, 개발자 기록 설정)는 WMI로 띄운 프로세스에서 한다. MSIX 패키지 앱(Claude 데스크톱 앱 등) 안의 셸은
HKCU·%LOCALAPPDATA% 쓰기가 그 앱의 칸으로 옮겨져서 다른 앱의 입력기가 보지 못한다.

### 키열 문법

보통 글자는 쿼티 자리, 대문자와 Shift 기호는 Shift. `{sp}` `{bs}` `{ent}` `{esc}` `{tab}` `{left}` …,
`{rs}` `{ls}`(오른쪽/왼쪽 Shift 탭), `{caps}`, `{click}`, `{M-c}`(⌘C), `{A-ent}`(왼쪽 Option+Enter, 한자) 같은 식이다.
수식키 조합은 입력기가 받는 순서대로 수식키 누름 → 키 → 수식키 뗌으로 보낸다.
예: `cargo run -q -p cssgsg-cli -- type "ishfsudskre{A-ent}"` → 대한민國(조합 중인 국만 바뀐다).

## 검증

세 배열 모두 써본 적 없이 이 입력기로 배우게 되므로, 동작은 서로 독립된 출처끼리 대조해서 검증한다.
무엇을 무엇과 어떻게 대조했는지와 결과는 [docs/verification.md](docs/verification.md)에 있다.

```bash
cargo test                                         # 전수·공식 파일 대조 포함 (참신 11,172 음절, 新月 공식 표, Graphite 공식 keylayout)
node tools/crosscheck/chamshin.mjs                 # 참신 키 배치 ↔ 타닥 배열도·오이 배열표
node tools/crosscheck/keycodes.mjs                 # 맥 keyCode·윈도우 스캔 코드 표 ↔ Chromium DOM 코드 표
cargo test -p cssgsg-tip                           # 윈도우 입력기(윈도우에서만): 팩토리, FP 환경, 패닉, 키 판정
cd tools/ohi-oracle && npm install && node diff.mjs --count 20000   # 참신 조합 ↔ 오이 (무작위 키열)
node tools/learn/build.mjs --check                # 학습 페이지 예시를 엔진으로 확인, index.html이 최신인지
node tools/practice/build.mjs --check             # 타자 연습 페이지: 낱말·줄·예시를 모두 엔진으로 쳐 보고, index.html이 최신인지
bash tools/mac/shell-smoke/run.sh                  # 맥 셸 Swift 계층(진짜 NSEvent) ↔ 러스트 시뮬레이터
bash tools/mac/update-smoke/run.sh                 # 업데이트 코드: 버전 순서, 채널(정식·베타), GitHub 응답, 해시, 설치 스크립트
bash tools/mac/settings-smoke/run.sh               # 설정 앱: 단축키 녹화(가짜 키 이벤트, 좌우), 화면 글자, 사전 읽기 정리, 타자 연습 기록 왕복 (--shots: 화면 스냅숏)
cargo test -p cssgsg-core --features mozc --test mozc   # 일본어 한자 변환(Mozc, 먼저 bash tools/mozc/build.sh)
bash tools/mac/mozc-smoke/run.sh                   # Mozc 엔진: 고르기·지키기·업데이트 확인·설치, 엔진 묶음을 받아 읽기
```

학습 페이지를 고쳤으면 `node tools/learn/build.mjs`로 `learn/index.html`을 다시 만든다.
타자 연습 페이지나 연습 글(`tools/practice/*.txt`)을 고쳤으면 `node tools/practice/build.mjs`로 `practice/index.html`을 다시 만든다.

빌드 산출물은 `build/` 아래에 둔다(`~/Documents`가 Syncthing 동기화 폴더라서 `.stignore`가 막는 이름을 썼다).

## 라이선스

MIT ([LICENSE](LICENSE)). 배열 데이터의 원래 라이선스는 아래 출처를 따른다.

## 배열 출처

- 참신세벌식: 원작자 공빈의 [확정안](https://cafe.daum.net/3bulsik/JMKX/147)(2026-02-12 수정)과 날개셋 `.ist` 속 사용법. `.ist` 파일 자체는 레포에 넣지 않는다.
- Graphite: [rdavison/graphite-layout](https://github.com/rdavison/graphite-layout) (MIT, 검증용 공식 keylayout은 `layouts/en/official/`)
- 新月配列: [nagamine-git/shingetsu-layout](https://github.com/nagamine-git/shingetsu-layout) (MIT, `layouts/ja/shingetsu/`에 원본 TSV, 검증용 로마자 표, 라이선스)
- 한자·기호 사전: [libhangul](https://github.com/libhangul/libhangul) `data/hanja`의 `hanja.txt`·`mssymbol.txt` (파일마다 BSD-3 고지, `dict/ko/`에 원본과 라이선스). 라이브러리 코드는 쓰지 않는다.
- 앱에 들어가는 다른 소프트웨어·데이터 고지문 전체는 [THIRD_PARTY_NOTICES.txt](THIRD_PARTY_NOTICES.txt)(`bash tools/make-notices.sh`로 만든다).

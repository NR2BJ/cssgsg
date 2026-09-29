# cssgsg
미친놈을 위한 정신나간 입력기

**C**ham**S**hin **S**ebeolsik(참신세벌식) + **G**raphite + **S**hin**G**etsu(新月配列).
세 배열을 입력 소스 하나에서 전환하며 쓰는 한/영/일 올인원 입력기다. macOS를 먼저 만들고 Windows는 나중에 만든다.
설계 문서는 [CONCEPT.md](CONCEPT.md).

## 상태

- [x] 코어(Rust, 플랫폼 독립)
  - 참신세벌식 v18(기본)·D v19 갈마들이 조합기. 키 단위 Backspace, ❖ 음절 조합 중지
  - Graphite: OS 레이아웃은 쿼티 그대로 두고 입력기가 글자만 바꾼다. 단축키는 쿼티 자리
  - 新月配列 가나 조합기: ☆/★ 앞치기, ゛ 뒤치기, 3타 단축, Caps Lock 가타카나
  - 모드 전환: 오른쪽 Shift 탭 = 영어 ↔ 직전 비영어, 왼쪽 Shift 탭 = 한 ↔ 일 (NRIME 설정 그대로)
  - C ABI(`core/include/cssgsg.h`)와 C 스모크 테스트
- [~] macOS 입력기 셸 (NRIME 플랫폼층 이식)
  - [x] M2a: IMKit 셸 뼈대. 키 → 코어 → 앱, 메뉴 막대 모드 표시(G/ㅊ/月), 조합 중 밑줄, 후보창
    - NRIME 우회책: ⌘/Ctrl 조합 확정 뒤 키 재전송, Chromium 조합 중 Shift+Enter, 마우스 클릭 확정, 비밀번호 칸은 쿼티
    - 셸 Swift 계층은 스모크 테스트로 검증했다. 실제 앱에서 쳐 보는 확인은 [확인 목록](docs/mac-checklist.md)으로 한다
  - [x] 배포: pkg + GitHub 릴리스 + 메뉴의 업데이트 확인(NRIME 방식). 자체 서명 인증서로 서명해서 업데이트해도 권한이 남는다
  - [~] M2b: 모드 HUD(커서 근처 G/ㅊ/月, 설정 `[mac]`) 완료. 입력 소스 복구, 비밀번호 칸 Graphite(나중에)
  - [ ] M2c: 설정 앱(학습 탭 포함). 그전까지는 메뉴 막대 → "설정 파일 열기"로 `config.toml`을 고친다
- [x] 일본어 한자 변환: Mozc를 입력기 프로세스 안에 정적 링크(0.2.0, [mozc/README.md](mozc/README.md))
- [ ] 한국어 한자 변환(Option+Enter): 조사 끝, 설계는 [CONCEPT.md](CONCEPT.md) §5.4
- [ ] 설정 앱(학습 탭 포함)
- [ ] Windows TSF (나중)

## 구조

```
core/       Rust 코어: key, hangul, latin, kana, engine, ffi, sim
layouts/    배열 데이터 (ko: 참신세벌식 TOML, en: Graphite, ja: 新月 공식 TSV 원본)
cli/        cssgsg-cli: 터미널에서 쳐보는 도구
mac/        macOS 입력기(Swift + InputMethodKit). project.yml → xcodegen → cssgsg.xcodeproj(생성물, 커밋 안 함)
mozc/       Mozc C API 래퍼(cssgsg용). 소스·빌드는 build/mozc, build/mozc-out(tools/mozc/build.sh)
tools/      ffi-smoke(C 헤더 확인), ohi-oracle(오이 차분 비교), crosscheck(배열 데이터 교차 검증),
            mac(pkg·릴리스·서명 스크립트, 셸·업데이트 스모크 테스트), learn(학습 페이지 빌드)
docs/       verification.md(검증 현황), mac-checklist.md(설치 뒤 확인 목록), releases/(릴리스 노트)
learn/      배열 학습 페이지(설정 앱 학습 탭). template.html → tools/learn/build.mjs → index.html
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
  추가하기 전까지 cssgsg 메뉴에 "⚠︎ 입력 소스에 추가하기…"가 보인다.
- 브라우저로 받은 pkg는 서명이 없어서 Gatekeeper가 막는다. 그때는 시스템 설정 → 개인정보 보호 및 보안에서 "그래도 열기"를 누른다(위 명령은 해당 없음).
- 업데이트: 메뉴 막대 cssgsg 메뉴 → 업데이트 확인(하루 한 번 저절로도 확인한다). 관리자 암호를 한 번 묻고, 로그아웃은 필요 없다.
- 처음 실행 때 한 번 "손쉬운 사용"(키 이벤트 보내기) 권한을 물어본다. 없어도 입력은 되고, 조합 중 ⌘/Option+키 재전송과 Codex 줄바꿈만 빠진다.
  릴리스는 고정 인증서로 서명하므로 업데이트해도 권한이 남는다. 권한이 없으면 메뉴에 "키 보내기 권한 허용…"이 보인다.

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

### 키열 문법

보통 글자는 쿼티 자리, 대문자와 Shift 기호는 Shift. `{sp}` `{bs}` `{ent}` `{esc}` `{tab}` `{left}` …,
`{rs}` `{ls}`(오른쪽/왼쪽 Shift 탭), `{caps}`, `{click}`, `{M-c}`(⌘C) 같은 식이다.

## 검증

세 배열 모두 써본 적 없이 이 입력기로 배우게 되므로, 동작은 서로 독립된 출처끼리 대조해서 검증한다.
무엇을 무엇과 어떻게 대조했는지와 결과는 [docs/verification.md](docs/verification.md)에 있다.

```bash
cargo test                                         # 전수·공식 파일 대조 포함 (참신 11,172 음절, 新月 공식 표, Graphite 공식 keylayout)
node tools/crosscheck/chamshin.mjs                 # 참신 키 배치 ↔ 타닥 배열도·오이 배열표
cd tools/ohi-oracle && npm install && node diff.mjs --count 20000   # 참신 조합 ↔ 오이 (무작위 키열)
node tools/learn/build.mjs --check                # 학습 페이지 예시를 엔진으로 확인, index.html이 최신인지
bash tools/mac/shell-smoke/run.sh                  # 맥 셸 Swift 계층(진짜 NSEvent) ↔ 러스트 시뮬레이터
bash tools/mac/update-smoke/run.sh                 # 업데이트 코드: 버전 순서, GitHub 응답, 해시, 설치 AppleScript
cargo test -p cssgsg-core --features mozc --test mozc   # 일본어 한자 변환(Mozc, 먼저 bash tools/mozc/build.sh)
```

학습 페이지를 고쳤으면 `node tools/learn/build.mjs`로 `learn/index.html`을 다시 만든다.

빌드 산출물은 `build/` 아래에 둔다(`~/Documents`가 Syncthing 동기화 폴더라서 `.stignore`가 막는 이름을 썼다).

## 라이선스

MIT ([LICENSE](LICENSE)). 배열 데이터의 원래 라이선스는 아래 출처를 따른다.

## 배열 출처

- 참신세벌식: 원작자 공빈의 [확정안](https://cafe.daum.net/3bulsik/JMKX/147)(2026-02-12 수정)과 날개셋 `.ist` 속 사용법. `.ist` 파일 자체는 레포에 넣지 않는다.
- Graphite: [rdavison/graphite-layout](https://github.com/rdavison/graphite-layout) (MIT, 검증용 공식 keylayout은 `layouts/en/official/`)
- 新月配列: [nagamine-git/shingetsu-layout](https://github.com/nagamine-git/shingetsu-layout) (MIT, `layouts/ja/shingetsu/`에 원본 TSV, 검증용 로마자 표, 라이선스)

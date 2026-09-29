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
- [ ] macOS 입력기 셸 (NRIME 플랫폼층 이식)
- [ ] Mozc 임베드 (일본어 한자 변환, 지금은 히라가나/가타카나 후보만 내는 임시 변환기)
- [ ] Windows TSF (나중)

## 구조

```
core/       Rust 코어: key, hangul, latin, kana, engine, ffi, sim
layouts/    배열 데이터 (ko: 참신세벌식 TOML, en: Graphite, ja: 新月 공식 TSV 원본)
cli/        cssgsg-cli: 터미널에서 쳐보는 도구
tools/      ffi-smoke(C 헤더 확인), ohi-oracle(오이 차분 비교), crosscheck(배열 데이터 교차 검증)
docs/       verification.md(검증 현황)
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

키열 문법: 보통 글자는 쿼티 자리, 대문자와 Shift 기호는 Shift. `{sp}` `{bs}` `{ent}` `{esc}` `{tab}` `{left}` …,
`{rs}` `{ls}`(오른쪽/왼쪽 Shift 탭), `{caps}`, `{click}`, `{M-c}`(⌘C) 같은 식이다.

## 검증

세 배열 모두 써본 적 없이 이 입력기로 배우게 되므로, 동작은 서로 독립된 출처끼리 대조해서 검증한다.
무엇을 무엇과 어떻게 대조했는지와 결과는 [docs/verification.md](docs/verification.md)에 있다.

```bash
cargo test                                         # 전수·공식 파일 대조 포함 (참신 11,172 음절, 新月 공식 표, Graphite 공식 keylayout)
node tools/crosscheck/chamshin.mjs                 # 참신 키 배치 ↔ 타닥 배열도·오이 배열표
cd tools/ohi-oracle && npm install && node diff.mjs --count 20000   # 참신 조합 ↔ 오이 (무작위 키열)
node tools/learn/build.mjs --check                # 학습 페이지 예시를 엔진으로 확인, index.html이 최신인지
```

학습 페이지를 고쳤으면 `node tools/learn/build.mjs`로 `learn/index.html`을 다시 만든다.

빌드 산출물은 `build/` 아래에 둔다(`~/Documents`가 Syncthing 동기화 폴더라서 `.stignore`가 막는 이름을 썼다).

## 라이선스

MIT ([LICENSE](LICENSE)). 배열 데이터의 원래 라이선스는 아래 출처를 따른다.

## 배열 출처

- 참신세벌식: 원작자 공빈의 [확정안](https://cafe.daum.net/3bulsik/JMKX/147)(2026-02-12 수정)과 날개셋 `.ist` 속 사용법. `.ist` 파일 자체는 레포에 넣지 않는다.
- Graphite: [rdavison/graphite-layout](https://github.com/rdavison/graphite-layout) (MIT, 검증용 공식 keylayout은 `layouts/en/official/`)
- 新月配列: [nagamine-git/shingetsu-layout](https://github.com/nagamine-git/shingetsu-layout) (MIT, `layouts/ja/shingetsu/`에 원본 TSV, 검증용 로마자 표, 라이선스)

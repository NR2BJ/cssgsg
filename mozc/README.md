# mozc — cssgsg용 Mozc C API

일본어 한자 변환에 [Mozc](https://github.com/google/mozc)(구글 일본어 입력의 공개판, BSD-3)를 쓴다.
NRIME처럼 `mozc_server`를 따로 띄우지 않고, 입력기 프로세스 안에 엔진을 정적 링크한다.

- `cssgsg/cssgsg_mozc.h` — C API(시작·명령·확정·취소·학습 켜기). 화면은 JSON 한 줄.
- `cssgsg/cssgsg_mozc.cc` — 구현. Mozc의 `ios/ios_engine.cc`(프로세스 안에서 쓰는 선례)를 따른다.
- `cssgsg/cssgsg_mozc_main.cc` — 확인용 명령줄 도구.
- `cssgsg/BUILD.bazel` — `tools/mozc/build.sh`가 이 폴더를 Mozc 소스 트리의 `src/cssgsg`로 복사해서 빌드한다.

## 흐름

1. 코어가 가진 가나 읽기를 한 글자씩 `key_string`으로 넣는다(가나 직접 입력과 같은 경로).
   `UPDATE_COMPOSITION`은 손글씨 경로로 취급되어 변환 순위가 달라지므로 쓰지 않는다.
2. Space로 변환한다. 첫 변환은 후보창 없이, 다음 후보부터 후보창이 뜬다.
3. 문절 이동과 길이는 MSIME 키맵의 ←→ / Shift+←→, 후보 선택은 `SELECT_CANDIDATE`, 확정은 `SUBMIT`(이때 학습한다).

설정: MSIME 키맵, 입력 중 추천·실시간 변환 끔, 음역 후보는 "そのほかの文字種"로 접지 않고 펼침.
학습 기록은 `~/Library/Application Support/cssgsg/mozc`(NRIME가 쓰는 `~/Library/Application Support/Mozc`와 따로).

## 빌드

```bash
brew install bazelisk
bash tools/mozc/build.sh                                   # → build/mozc-out (lib, include, data, bin)
cargo test -p cssgsg-core --features mozc --test mozc       # 변환 테스트(학습 끔, 한 번에 하나씩)
build/mozc-out/bin/cssgsg_mozc_main build/mozc-out/data/mozc.data /tmp/p にほんご next commit
```

- Mozc 소스는 `build/mozc`에 고정 커밋으로 받는다(`tools/mozc/build.sh`의 `MOZC_COMMIT`). 처음 빌드는 3~4분, 그다음은 바뀐 것만.
- Mozc는 세션 처리기를 정해진 패키지에만 보이게 해 둔다. 소스를 고치지 않고 `--nocheck_visibility`로 검사만 끈다.
- 앱과 같은 macOS 13 기준으로 빌드한다(`--macos_minimum_os=13.0`).
- Mozc는 설정·학습 폴더 같은 전역 상태가 있다. 여러 인스턴스를 여러 스레드에서 동시에 쓰지 않는다(입력기는 하나를 메인 스레드에서만 쓴다).
- 고지문: Mozc, 사전 데이터(IPAdic 등), abseil, protobuf, zlib → `THIRD_PARTY_NOTICES.txt`(`tools/make-notices.sh`).

# 新月配列 (Shingetsu Layout) 원본 데이터

- 출처: https://github.com/nagamine-git/shingetsu-layout (MIT, `LICENSE` 참고)
- 파일
  - `shingetsu-ansi-qwerty.tsv`: hazkey용 표. 엔진이 이 파일을 읽는다(수정하지 않고 그대로 둔다).
  - `shingetsu-romantable.txt`: Google 일본어 입력용 로마자 표. **검증용**이다. 같은 배열을 다른 방식(가나 대신 원래 키열)으로 적은 공식 파일이라,
    `core/tests/verify_shingetsu.rs`가 이 표의 모든 행을 엔진에 쳐서 결과를 대조한다.
- 고정 커밋: `cb9dc2046951ef9a06245177565fbefed50cf234` (2026-09-26T13:43:33Z), 新月配列 v1.1.0

업스트림이 갱신되면 이 파일만 교체하고 `cargo test`로 확인한다.

# 오이 차분 비교 (참신세벌식)

오이(Online Hangeul IME, [Sinseiki/ohi](https://github.com/Sinseiki/ohi))를 헤드리스(jsdom)로 돌려
무작위 키열 결과를 `cssgsg-cli batch`와 비교한다.

- 오이는 GPL이라 **레포에 넣지 않는다.** 처음 실행할 때 고정 커밋을 `build/ohi-oracle/ohi`에 받는다.
- 오이의 참신세벌식 조합표는 2021년 구현이라 옛날 것이다. 실행할 때 사본을 **원작자 최신 규칙(2026-02-12)**으로 고친다.
  - ㅉ = ㅈ+ㅇ (옛: ㄱ+ㅅ)
  - 받침 ㅋ = ㄱ+ㅁ (옛: ㄱ+ㅁ→ㄲ, ㄱ+ㅂ→ㅋ)
  - ㄿ = ㅇ+ㅁ (옛: ㅇ+ㅁ→ㅀ)
- 오이와 일부러 다르게 가는 부분(`KNOWN_DIFFERENCES` in `diff.mjs`)은 따로 센다.

```bash
cd tools/ohi-oracle && npm install
node diff.mjs --count 20000 --seed 1
```

정본은 원작자 문서와 날개셋이다. 오이와 다른 결과가 나오면 날개셋 동작으로 판정한다(윈도우 작업 때).

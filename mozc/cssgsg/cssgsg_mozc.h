/* cssgsg용 Mozc C API. 입력기 프로세스 안에서 Mozc 엔진(SessionHandler)을 직접 쓴다(mozc_server 없음).
 *
 * 흐름: 코어가 가진 가나 읽기를 한 글자씩 key_string으로 넣고(가나 직접 입력과 같은 경로) Space로 변환한다.
 * 문절 이동·길이는 MSIME 키맵의 ←→ / Shift+←→, 후보 선택은 SELECT_CANDIDATE, 확정은 SUBMIT(이때 Mozc가 학습한다).
 *
 * 화면(변환 결과)은 JSON 한 줄로 돌려준다. 돌려준 문자열은 같은 인스턴스의 다음 호출 전까지 유효하다.
 *   {"segments":["日本語"],"focused":0,"candidates":["日本語",…],"selected":0,"page":[1,3]}
 *   후보창이 아직 없으면(첫 변환) "candidates":[],"selected":null,"page":null */
#ifndef CSSGSG_MOZC_H_
#define CSSGSG_MOZC_H_

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct CssgsgMozc CssgsgMozc;

enum {
  CSSGSG_MOZC_NEXT = 0,
  CSSGSG_MOZC_PREV = 1,
  CSSGSG_MOZC_FOCUS_LEFT = 2,
  CSSGSG_MOZC_FOCUS_RIGHT = 3,
  CSSGSG_MOZC_SHRINK = 4,
  CSSGSG_MOZC_EXPAND = 5,
  CSSGSG_MOZC_NEXT_PAGE = 6,
  CSSGSG_MOZC_PREV_PAGE = 7,
  CSSGSG_MOZC_SELECT_ON_PAGE = 8, /* arg: 지금 페이지 안 번호(0부터) */
};

/* data_path: mozc.data. profile_dir: 학습·사용자 사전 폴더(NULL이나 ""이면 Mozc 기본 폴더). 실패하면 NULL. */
CssgsgMozc *cssgsg_mozc_new(const char *data_path, const char *profile_dir);
void cssgsg_mozc_free(CssgsgMozc *m);

/* 읽기로 변환을 시작한다. 화면 JSON, 실패하면 NULL. */
const char *cssgsg_mozc_start(CssgsgMozc *m, const char *reading);
/* 변환 중 명령. 화면 JSON, 실패하면 NULL(화면은 그대로 둔다). */
const char *cssgsg_mozc_command(CssgsgMozc *m, int32_t command, int32_t arg);
/* 지금 결과를 확정하고(학습) 확정 글자를 돌려준다. */
const char *cssgsg_mozc_commit(CssgsgMozc *m);
/* 변환을 버린다(학습하지 않는다). */
void cssgsg_mozc_cancel(CssgsgMozc *m);
/* 학습(확정한 후보를 다음에 먼저 내기)을 켜고 끈다. 기본은 켬. 끄면 결과가 늘 같다(테스트). */
void cssgsg_mozc_set_learning(CssgsgMozc *m, int32_t enabled);

#ifdef __cplusplus
}
#endif

#endif  // CSSGSG_MOZC_H_

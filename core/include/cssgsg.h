/*
 * cssgsg 입력 엔진 C ABI. 구현: core/src/ffi.rs (손으로 맞춰 둔다).
 *
 * - 문자열은 UTF-8, NUL로 끝난다.
 * - 돌려주는 CssgsgOutput과 그 안의 포인터는 같은 엔진의 다음 호출 전까지만 유효하다.
 * - 한 엔진은 한 스레드에서만 쓴다.
 *
 * 셸 규칙(NRIME 교훈)
 * - commit이 비어 있지 않으면 insertText(commit)만 부른다. 그 앞뒤에 setMarkedText("")를 부르지 않는다.
 * - 그다음 preedit_changed이고 preedit가 비어 있지 않으면 setMarkedText(preedit, segments).
 *   commit 없이 preedit_changed이고 preedit가 비어 있으면 조합 중 글자를 지운다.
 * - consumed가 0이면 원래 키를 앱에 넘긴다(handle에서 false).
 */
#ifndef CSSGSG_H
#define CSSGSG_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

enum {
    CSSGSG_MODE_EN = 0,
    CSSGSG_MODE_KO = 1,
    CSSGSG_MODE_JA = 2,
};

enum {
    CSSGSG_MOD_SHIFT_L = 1u << 0,
    CSSGSG_MOD_SHIFT_R = 1u << 1,
    CSSGSG_MOD_CTRL_L = 1u << 2,
    CSSGSG_MOD_CTRL_R = 1u << 3,
    CSSGSG_MOD_ALT_L = 1u << 4,
    CSSGSG_MOD_ALT_R = 1u << 5,
    CSSGSG_MOD_META_L = 1u << 6,
    CSSGSG_MOD_META_R = 1u << 7,
    CSSGSG_MOD_CAPS = 1u << 8,
    CSSGSG_MOD_FN = 1u << 9,
};

typedef struct CssgsgEngine CssgsgEngine;

typedef struct CssgsgKeyEvent {
    uint16_t key;    /* HID 키 코드 (cssgsg_key_from_mac_keycode) */
    uint8_t down;    /* 1 = 눌림, 0 = 뗌. 수식키도 눌림/뗌을 따로 보낸다 */
    uint8_t is_repeat; /* 1 = 키 반복 */
    uint32_t mods;   /* 이 이벤트가 반영된 뒤의 CSSGSG_MOD_* */
    double time;     /* 초 단위 단조 시각 (NSEvent.timestamp) */
} CssgsgKeyEvent;

typedef struct CssgsgContext {
    uint8_t game_mode;      /* 영어 모드를 쿼티 그대로 통과 (윈도우 게임) */
    uint8_t taps_disabled;  /* 수식키 탭 전환 끄기 */
    uint8_t secure_field;   /* 비밀번호 칸 등: 조합 없이 모두 통과 (언어 전환 탭은 된다) */
} CssgsgContext;

typedef struct CssgsgSegment {
    uint32_t start;  /* UTF-16 단위 */
    uint32_t len;    /* UTF-16 단위 */
    uint8_t focused; /* 변환 중 포커스된 문절 */
} CssgsgSegment;

typedef struct CssgsgOutput {
    uint8_t consumed;
    uint8_t preedit_changed;
    uint8_t candidates_changed;
    uint8_t caps_lock_off;       /* Caps Lock을 꺼 달라 */
    int32_t mode;                /* 바뀐 모드, 안 바뀌었으면 -1 */
    const char *commit;          /* 확정할 글자 ("" 가능, NULL 아님) */
    const char *preedit;         /* 조합 중 글자 */
    uint32_t preedit_caret;      /* UTF-16 단위, 항상 끝 */
    const CssgsgSegment *segments;
    uint32_t segment_count;
    const char *const *candidates;
    uint32_t candidate_count;    /* candidates_changed이고 0이면 후보창을 닫는다 */
    int32_t candidate_selected;  /* 없으면 -1 */
    uint32_t candidate_page;     /* 1부터, 모르면 0 */
    uint32_t candidate_pages;
} CssgsgOutput;

/* config_toml이 NULL이면 기본 설정. 설정 오류면 NULL (cssgsg_last_error 참고). */
CssgsgEngine *cssgsg_engine_new(const char *config_toml);
void cssgsg_engine_free(CssgsgEngine *engine);

/* ctx는 NULL 허용. 잘못된 인자면 NULL. */
const CssgsgOutput *cssgsg_engine_handle_key(CssgsgEngine *engine, const CssgsgKeyEvent *event,
                                             const CssgsgContext *ctx);
/* 조합 중인 것을 확정 (포커스 해제 등) */
const CssgsgOutput *cssgsg_engine_commit(CssgsgEngine *engine);
/* 마우스 클릭: 확정 + 진행 중인 수식키 탭 무효 */
const CssgsgOutput *cssgsg_engine_mouse_down(CssgsgEngine *engine);
/* 확정 없이 비우기 (입력기 활성화 때) */
const CssgsgOutput *cssgsg_engine_reset(CssgsgEngine *engine);
const CssgsgOutput *cssgsg_engine_set_mode(CssgsgEngine *engine, int32_t mode);
int32_t cssgsg_engine_mode(const CssgsgEngine *engine);

uint16_t cssgsg_key_from_mac_keycode(uint16_t mac_keycode);
const char *cssgsg_last_error(void);
const char *cssgsg_version(void);

#ifdef __cplusplus
}
#endif

#endif /* CSSGSG_H */

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
    const char *const *candidates; /* 포커스된 문절의 후보 전체. 페이지는 후보창이 나눈다 */
    uint32_t candidate_count;    /* candidates_changed이고 0이면 후보창을 닫는다 */
    int32_t candidate_selected;  /* 전체 목록 기준, 없으면 -1 */
    uint32_t candidate_page;     /* 1부터(지금 목록/격자 기준), 모르면 0 */
    uint32_t candidate_pages;
    uint8_t candidate_grid;      /* 격자(펼친) 모드면 1: 목록 9개/페이지, 격자 5열 × 6행 */
    uint8_t learning_changed;    /* 한자 학습이 바뀌었다(저장한다) */
    const char *const *candidate_notes; /* 후보마다 뜻(candidate_count개, "" 가능). 없는 후보창이면 NULL */
    uint32_t timer_ms;           /* 0이 아니면 이만큼 뒤에 cssgsg_engine_timer를 부른다(빠른 탭 전환 보정) */
} CssgsgOutput;

/* config_toml이 NULL이면 기본 설정. 설정 오류면 NULL (cssgsg_last_error 참고). */
CssgsgEngine *cssgsg_engine_new(const char *config_toml);
void cssgsg_engine_free(CssgsgEngine *engine);

/* ctx는 NULL 허용. 잘못된 인자면 NULL. */
const CssgsgOutput *cssgsg_engine_handle_key(CssgsgEngine *engine, const CssgsgKeyEvent *event,
                                             const CssgsgContext *ctx);
/* 조합 중인 것을 확정 (포커스 해제 등) */
const CssgsgOutput *cssgsg_engine_commit(CssgsgEngine *engine);
/* timer_ms만큼 기다린 뒤 부른다. now는 키 이벤트와 같은 시계(초).
 * held는 지금 실제로 누르고 있는 수식키(CSSGSG_MOD_*). 좌우를 모르면 양쪽 비트를 켠다. */
const CssgsgOutput *cssgsg_engine_timer(CssgsgEngine *engine, double now, uint32_t held);
/* 셸이 엔진에 넘기지 않은 키가 눌렸다(Shift+Enter를 다시 보내기 전에 잡아 둔 키): 진행 중인 수식키 탭 무효 */
void cssgsg_engine_cancel_tap(CssgsgEngine *engine);
/* 변환기(Mozc)의 사용자 사전을 다시 읽는다(설정 앱이 고친 뒤). */
uint8_t cssgsg_engine_reload_dictionary(CssgsgEngine *engine);
/* 마우스 클릭: 확정 + 진행 중인 수식키 탭 무효 */
const CssgsgOutput *cssgsg_engine_mouse_down(CssgsgEngine *engine);
/* 확정 없이 비우기 (입력기 활성화 때) */
const CssgsgOutput *cssgsg_engine_reset(CssgsgEngine *engine);
const CssgsgOutput *cssgsg_engine_set_mode(CssgsgEngine *engine, int32_t mode);
int32_t cssgsg_engine_mode(const CssgsgEngine *engine);

/* 설정을 바꾼다(설정 앱이 설정 파일을 고친 뒤). 조합 중인 것·모드·학습은 그대로 둔다.
 * config_toml이 NULL이면 기본 설정. 성공하면 1, 설정 오류면 0(그대로 두고 cssgsg_last_error). */
uint8_t cssgsg_engine_set_config(CssgsgEngine *engine, const char *config_toml);

/* 한자 학습(TSV)을 불러와 바꾼다. 읽은 항목 수. */
uint32_t cssgsg_engine_hanja_learning_load(CssgsgEngine *engine, const char *tsv);
/* 한자 학습을 TSV로. 다음 이 함수 호출이나 해제 전까지 유효. */
const char *cssgsg_engine_hanja_learning_save(CssgsgEngine *engine);

/* 이 코어가 읽을 수 있는 Mozc 엔진 C API 판(mozc/cssgsg/cssgsg_mozc.h의 CSSGSG_MOZC_ABI_VERSION).
 * mozc 기능 없이 빌드했으면 0. */
int32_t cssgsg_mozc_abi(void);

/* Mozc(일본어 한자 변환)를 켠다: 엔진을 읽고 만든 뒤 낱말 하나를 변환해 본다. 성공하면 1.
 * 실패하면 0(변환기는 그대로, 까닭은 cssgsg_last_error).
 * library_path: libcssgsg_mozc.dylib, data_path: 같이 빌드한 mozc.data, profile_dir: 학습·사용자 사전 폴더. */
uint8_t cssgsg_engine_use_mozc(CssgsgEngine *engine, const char *library_path, const char *data_path,
                               const char *profile_dir);

/* 맥 셸 설정(설정 파일의 [mac]). 엔진은 쓰지 않는다. engine이 NULL이면 기본값. */
typedef struct CssgsgMacSettings {
    uint8_t hud;               /* 모드를 바꿀 때 G/ㅊ/月를 잠깐 보인다 */
    uint8_t hud_at_mouse;      /* 1이면 마우스 옆, 0이면 커서 위 */
    uint32_t candidate_font_size; /* 후보창 글자 크기(포인트) */
    uint32_t newline_insert_wait_ms;    /* 줄바꿈 넣기(웹 기술로 만든 앱)·⌘ 단축키 다시 보내기(모든 앱) 대기 */
    uint32_t newline_key_press_wait_ms; /* Shift+Enter 다시 보내기 대기(아래 목록의 앱) */
} CssgsgMacSettings;
CssgsgMacSettings cssgsg_engine_mac_settings(const CssgsgEngine *engine);
/* Shift+Enter 키를 다시 보낼 앱(번들 ID)을 줄바꿈으로 이은 글자열(빈 목록이면 ""). 다음 호출이나 엔진 해제 전까지 유효.
 * engine이 NULL이면 NULL. */
const char *cssgsg_engine_newline_key_press_apps(CssgsgEngine *engine);

uint16_t cssgsg_key_from_mac_keycode(uint16_t mac_keycode);
const char *cssgsg_last_error(void);
const char *cssgsg_version(void);

#ifdef __cplusplus
}
#endif

#endif /* CSSGSG_H */

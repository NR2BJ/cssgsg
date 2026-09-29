/* 헤더(core/include/cssgsg.h)와 Rust ABI가 맞는지 확인하는 C 스모크 테스트.
 * 실행: bash tools/ffi-smoke/run.sh */
#include <stdio.h>
#include <string.h>
#include "cssgsg.h"

static int fail = 0;
#define CHECK(cond) do { if (!(cond)) { printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #cond); fail = 1; } } while (0)

static CssgsgKeyEvent key(uint16_t mac, double t) {
    CssgsgKeyEvent e = { cssgsg_key_from_mac_keycode(mac), 1, 0, 0, t };
    return e;
}

int main(void) {
    printf("cssgsg %s\n", cssgsg_version());
    CssgsgEngine *e = cssgsg_engine_new(NULL);
    CHECK(e != NULL);
    const CssgsgOutput *o = cssgsg_engine_set_mode(e, CSSGSG_MODE_KO);
    CHECK(o->mode == CSSGSG_MODE_KO);

    CssgsgKeyEvent k = key(0x28, 1.0); /* k → ㄱ */
    cssgsg_engine_handle_key(e, &k, NULL);
    k = key(0x1F, 1.1); /* o → ㅗ (오른손) */
    cssgsg_engine_handle_key(e, &k, NULL);
    k = key(0x03, 1.2); /* f → ㅏ, 겹모음 */
    o = cssgsg_engine_handle_key(e, &k, NULL);
    CHECK(o->consumed == 1);
    CHECK(o->preedit_changed == 1);
    CHECK(strcmp(o->preedit, "과") == 0);
    CHECK(o->segment_count == 1 && o->segments[0].len == 1);

    o = cssgsg_engine_commit(e);
    CHECK(strcmp(o->commit, "과") == 0);

    /* 오른쪽 Shift 탭 → 영어 */
    CssgsgKeyEvent down = { cssgsg_key_from_mac_keycode(0x3C), 1, 0, CSSGSG_MOD_SHIFT_R, 2.0 };
    CssgsgKeyEvent up = { cssgsg_key_from_mac_keycode(0x3C), 0, 0, 0, 2.05 };
    cssgsg_engine_handle_key(e, &down, NULL);
    o = cssgsg_engine_handle_key(e, &up, NULL);
    CHECK(o->mode == CSSGSG_MODE_EN);

    /* 영어: 쿼티 j → Graphite h */
    k = key(0x26, 3.0);
    o = cssgsg_engine_handle_key(e, &k, NULL);
    CHECK(o->consumed == 1 && strcmp(o->commit, "h") == 0);

    CHECK(cssgsg_engine_new("ko_layout = \"x\"") == NULL);
    CHECK(strlen(cssgsg_last_error()) > 0);

    /* 맥 셸 설정: 기본값과 [mac] 표 */
    CssgsgMacSettings m = cssgsg_engine_mac_settings(e);
    CHECK(m.hud == 1 && m.hud_at_mouse == 0 && m.newline_replay_ms == 120);
    m = cssgsg_engine_mac_settings(NULL);
    CHECK(m.hud == 1 && m.newline_replay_ms == 120);
    CssgsgEngine *c = cssgsg_engine_new("[mac]\nhud = false\nhud_position = \"mouse\"\nnewline_replay_ms = 80");
    CHECK(c != NULL);
    m = cssgsg_engine_mac_settings(c);
    CHECK(m.hud == 0 && m.hud_at_mouse == 1 && m.newline_replay_ms == 80);
    cssgsg_engine_free(c);
    CHECK(cssgsg_engine_new("[mac]\nnewline_replay_ms = 5") == NULL);

    /* 한자: Option+Enter → 앱 글자 요청 → hanja_begin → 앞 글자를 끌어온 조합, 후보 뜻, 학습 저장/불러오기.
     * 새 필드는 구조체 끝에 있어서 헤더와 러스트의 배치가 어긋나면 여기서 값이 틀린다. */
    CssgsgEngine *h = cssgsg_engine_new(NULL);
    cssgsg_engine_set_mode(h, CSSGSG_MODE_KO);
    const uint16_t guk[] = { 0x28, 0x0F, 0x0E }; /* k r e → 국 */
    for (int i = 0; i < 3; i++) {
        k = key(guk[i], 4.0 + i * 0.1);
        cssgsg_engine_handle_key(h, &k, NULL);
    }
    CssgsgKeyEvent option_enter = { cssgsg_key_from_mac_keycode(0x24), 1, 0, CSSGSG_MOD_ALT_L, 5.0 };
    o = cssgsg_engine_handle_key(h, &option_enter, NULL);
    CHECK(o->consumed == 1 && o->hanja_context == 2 && o->preedit_changed == 0);
    o = cssgsg_engine_hanja_begin(h, "대한민", NULL);
    CHECK(o->consumed == 1 && o->preedit_changed == 1 && o->preedit_replace_before == 3);
    CHECK(strcmp(o->preedit, "大韓民國") == 0);
    CHECK(o->candidates_changed == 1 && o->candidate_count > 3 && o->candidate_selected == 0);
    CHECK(o->candidate_page == 1 && o->candidate_grid == 0 && o->hanja_context == 0);
    CHECK(o->candidate_notes != NULL && strcmp(o->candidate_notes[2], "나라 국") == 0);
    o = cssgsg_engine_commit(h);
    CHECK(strcmp(o->commit, "大韓民國") == 0 && o->learning_changed == 1);
    const char *tsv = cssgsg_engine_hanja_learning_save(h);
    CHECK(strstr(tsv, "대한민국\t大韓民國\t1\t") != NULL);
    CHECK(cssgsg_engine_hanja_learning_load(e, tsv) == 1);
    cssgsg_engine_free(h);

    cssgsg_engine_free(e);
    printf(fail ? "FFI SMOKE: FAIL\n" : "FFI SMOKE: OK\n");
    return fail;
}

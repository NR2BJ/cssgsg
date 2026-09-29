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
    CHECK(cssgsg_engine_mode(e) == CSSGSG_MODE_KO); /* 한국어로 시작한다 */
    const CssgsgOutput *o = cssgsg_engine_set_mode(e, CSSGSG_MODE_EN);
    CHECK(o->mode == CSSGSG_MODE_EN);
    o = cssgsg_engine_set_mode(e, CSSGSG_MODE_KO);
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
    CHECK(m.hud == 1 && m.hud_at_mouse == 0 && m.candidate_font_size == 14);
    m = cssgsg_engine_mac_settings(NULL);
    CHECK(m.hud == 1 && m.hud_at_mouse == 0 && m.candidate_font_size == 14);
    CssgsgEngine *c = cssgsg_engine_new("[mac]\nhud = false\nhud_position = \"mouse\"\ncandidate_font_size = 18");
    CHECK(c != NULL);
    m = cssgsg_engine_mac_settings(c);
    CHECK(m.hud == 0 && m.hud_at_mouse == 1 && m.candidate_font_size == 18);
    cssgsg_engine_free(c);
    CHECK(cssgsg_engine_new("[mac]\ncandidate_font_size = 40") == NULL);
    /* 0.5.x의 줄바꿈 대기는 읽고 버린다(0.6.0부터 기다리지 않는다) */
    c = cssgsg_engine_new("[mac]\nshift_enter_delay_ms = 25\nnewline_replay_ms = 150\nnewline_delay_offset_ms = 10");
    CHECK(c != NULL);
    cssgsg_engine_free(c);

    /* 빠른 탭 전환 보정: 오른쪽 Shift를 누른 채 친 글자는 잡아 두고 타이머를 청한다(timer_ms). */
    CssgsgEngine *b = cssgsg_engine_new("tap_buffering = true");
    CHECK(b != NULL);
    cssgsg_engine_set_mode(b, CSSGSG_MODE_EN);
    CssgsgKeyEvent rs = { cssgsg_key_from_mac_keycode(0x3C), 1, 0, CSSGSG_MOD_SHIFT_R, 10.0 };
    CssgsgKeyEvent rs_j = { cssgsg_key_from_mac_keycode(0x26), 1, 0, CSSGSG_MOD_SHIFT_R, 10.05 };
    cssgsg_engine_handle_key(b, &rs, NULL);
    o = cssgsg_engine_handle_key(b, &rs_j, NULL);
    CHECK(o->consumed == 1 && o->commit[0] == '\0' && o->timer_ms > 0 && o->timer_ms <= 100);
    /* 시간이 다 되면(수식키를 떼지 않았다) 누른 그대로 친다: Shift+j → H */
    o = cssgsg_engine_timer(b, 10.2);
    CHECK(o->consumed == 1 && strcmp(o->commit, "H") == 0 && o->timer_ms == 0);
    o = cssgsg_engine_timer(b, 10.3);
    CHECK(o->consumed == 0 && o->commit[0] == '\0' && o->timer_ms == 0);
    CssgsgKeyEvent rs_up = { cssgsg_key_from_mac_keycode(0x3C), 0, 0, 0, 10.4 };
    cssgsg_engine_handle_key(b, &rs_up, NULL);
    /* 잡아 둔 뒤 곧 떼면 탭: 전환하고(영어 → 한국어) 그 글자는 Shift 없이 친다 */
    rs.time = 20.0;
    rs_j.time = 20.05;
    rs_up.time = 20.08;
    cssgsg_engine_handle_key(b, &rs, NULL);
    cssgsg_engine_handle_key(b, &rs_j, NULL);
    o = cssgsg_engine_handle_key(b, &rs_up, NULL);
    CHECK(o->mode == CSSGSG_MODE_KO && o->consumed == 1 && o->preedit_changed == 1 && strlen(o->preedit) > 0);
    /* 사용자 사전 다시 읽기: Mozc가 없으면 할 일이 없다 */
    CHECK(cssgsg_engine_reload_dictionary(b) == 1);
    CHECK(cssgsg_engine_reload_dictionary(NULL) == 0);
    CHECK(cssgsg_engine_timer(NULL, 1.0) == NULL);
    cssgsg_engine_free(b);

    /* 한자: 조합 중인 국 + Option+Enter → 國(조합), 후보 뜻, 확정, 학습 저장/불러오기.
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
    CHECK(o->consumed == 1 && o->preedit_changed == 1 && strcmp(o->preedit, "國") == 0);
    CHECK(o->candidates_changed == 1 && o->candidate_count > 3 && o->candidate_selected == 0);
    CHECK(o->candidate_page == 1 && o->candidate_grid == 0 && o->learning_changed == 0);
    CHECK(o->candidate_notes != NULL && strcmp(o->candidate_notes[0], "나라 국") == 0);
    CHECK(strcmp(o->candidates[1], "局") == 0);
    o = cssgsg_engine_commit(h);
    CHECK(strcmp(o->commit, "國") == 0 && o->learning_changed == 1);
    const char *tsv = cssgsg_engine_hanja_learning_save(h);
    CHECK(strstr(tsv, "국\t國\t1\t") != NULL);
    CHECK(cssgsg_engine_hanja_learning_load(e, tsv) == 1);
    cssgsg_engine_free(h);

    cssgsg_engine_free(e);
    printf(fail ? "FFI SMOKE: FAIL\n" : "FFI SMOKE: OK\n");
    return fail;
}

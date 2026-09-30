/* 설정 라이브러리 헤더(config-ffi/include/cssgsg_config.h)와 러스트 ABI가 맞는지 확인하는 C 스모크 테스트.
 * 실행: bash tools/ffi-smoke/run.sh */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include "cssgsg_config.h"

static int fail = 0;
#define CHECK(cond) do { if (!(cond)) { printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #cond); fail = 1; } } while (0)

int main(void) {
    /* 기본 설정 → JSON → 설정 파일: 모두 주석 */
    const char *json = cssgsg_config_json(NULL);
    CHECK(json != NULL && strstr(json, "\"ko_layout\":\"chamshin-v18\"") != NULL);
    char buf[4096];
    snprintf(buf, sizeof buf, "%s", json);
    const char *toml = cssgsg_config_toml(buf);
    CHECK(toml != NULL && strstr(toml, "# ko_layout = \"chamshin-v18\"") != NULL);
    /* 바꾼 값은 주석 없이 적는다 */
    toml = cssgsg_config_toml("{\"ja\":{\"full_width_space\":true}}");
    CHECK(toml != NULL && strstr(toml, "\nfull_width_space = true\n") != NULL);
    /* 파일 → JSON: 적은 값과 기본값 */
    json = cssgsg_config_json("[mac]\nhud = false\n");
    CHECK(json != NULL && strstr(json, "\"hud\":false") != NULL && strstr(json, "\"candidate_font_size\":14") != NULL
          && strstr(json, "delay") == NULL && strstr(json, "tap_overlap_ms") == NULL
          && strstr(json, "\"newline_insert_wait_ms\":20") != NULL && strstr(json, "\"newline_key_press_wait_ms\":50") != NULL);
    CHECK(cssgsg_config_toml("{\"mac\":{\"newline_key_press_wait_ms\":201}}") == NULL);
    CHECK(strstr(cssgsg_config_error(), "newline_key_press_wait_ms") != NULL);
    /* 오류 */
    CHECK(cssgsg_config_json("tap_threshold_ms = 1") == NULL);
    CHECK(strstr(cssgsg_config_error(), "tap_threshold_ms") != NULL);
    CHECK(cssgsg_config_toml("{\"mac\":{\"candidate_font_size\":40}}") == NULL);
    CHECK(strstr(cssgsg_config_error(), "candidate_font_size") != NULL);
    /* 단축키는 글자열이고, 틀린 것은 까닭과 함께 거부한다 */
    json = cssgsg_config_json(NULL);
    CHECK(json != NULL && strstr(json, "\"toggle_english\":\"tap:shift_right\"") != NULL
          && strstr(json, "\"hanja\":\"alt_left+enter\"") != NULL && strstr(json, "\"candidate_font_size\":14") != NULL);
    toml = cssgsg_config_toml("{\"shortcuts\":{\"toggle_non_english\":\"control_left+shift_right+space\"}}");
    CHECK(toml != NULL && strstr(toml, "\ntoggle_non_english = \"control_left+shift_right+space\"\n") != NULL);
    CHECK(cssgsg_config_toml("{\"shortcuts\":{\"hanja\":\"meta_left+enter\"}}") == NULL); /* ⌘ 조합 */
    CHECK(cssgsg_config_toml("{\"shortcuts\":{\"hanja\":\"a\"}}") == NULL);
    CHECK(strstr(cssgsg_config_error(), "hanja") != NULL);
    CHECK(cssgsg_config_toml("{\"shortcuts\":{\"toggle_non_english\":\"tap:shift_right\"}}") == NULL);
    /* 녹화: 맥 키코드 → 키 이름. 수식키는 NULL */
    CHECK(strcmp(cssgsg_config_key_name(0x24), "enter") == 0);
    CHECK(strcmp(cssgsg_config_key_name(0x00), "a") == 0);
    CHECK(strcmp(cssgsg_config_key_name(0x69), "f13") == 0);
    CHECK(cssgsg_config_key_name(0x38) == NULL);
    /* 사용자 사전: 없는 파일은 빈 사전, 쓰고 다시 읽기 */
    const char *dict = cssgsg_userdict_json("/nonexistent/cssgsg/user_dictionary.db");
    CHECK(dict != NULL && strcmp(dict, "{\"dictionaries\":[]}") == 0);
    char path[] = "/tmp/cssgsg-config-smoke-XXXXXX";
    int fd = mkstemp(path);
    CHECK(fd >= 0);
    close(fd);
    CHECK(cssgsg_userdict_save(path, "{\"dictionaries\":[{\"id\":\"\",\"name\":\"User Dictionary\","
                                     "\"entries\":[{\"key\":\"くもつ\",\"value\":\"雲津\"}]}]}") == 1);
    dict = cssgsg_userdict_json(path);
    CHECK(dict != NULL && strstr(dict, "\"key\":\"くもつ\",\"value\":\"雲津\"") != NULL && strstr(dict, "\"pos\":1") != NULL);
    CHECK(cssgsg_userdict_save(path, "{\"dictionaries\":[{\"entries\":[{\"key\":\"\",\"value\":\"x\"}]}]}") == 0);
    unlink(path);
    printf(fail ? "CONFIG FFI SMOKE: FAIL\n" : "CONFIG FFI SMOKE: OK\n");
    return fail;
}

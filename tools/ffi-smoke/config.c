/* 설정 라이브러리 헤더(config-ffi/include/cssgsg_config.h)와 러스트 ABI가 맞는지 확인하는 C 스모크 테스트.
 * 실행: bash tools/ffi-smoke/run.sh */
#include <stdio.h>
#include <string.h>
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
    CHECK(json != NULL && strstr(json, "\"hud\":false") != NULL && strstr(json, "\"newline_replay_ms\":120") != NULL);
    /* 오류 */
    CHECK(cssgsg_config_json("tap_threshold_ms = 1") == NULL);
    CHECK(strstr(cssgsg_config_error(), "tap_threshold_ms") != NULL);
    CHECK(cssgsg_config_toml("{\"mac\":{\"newline_replay_ms\":5}}") == NULL);
    CHECK(strstr(cssgsg_config_error(), "newline_replay_ms") != NULL);
    printf(fail ? "CONFIG FFI SMOKE: FAIL\n" : "CONFIG FFI SMOKE: OK\n");
    return fail;
}

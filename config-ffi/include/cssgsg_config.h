/*
 * cssgsg 설정 앱용 C ABI: 설정 파일(TOML) ↔ JSON. 구현: config-ffi/src/lib.rs (손으로 맞춰 둔다).
 *
 * 설정 앱은 입력기 코어(Mozc가 묶인 libcssgsg_core.a) 대신 이것(libcssgsg_config.a)만 링크한다.
 * - 문자열은 UTF-8, NUL로 끝난다.
 * - 돌려주는 문자열은 같은 스레드에서 이 라이브러리를 다시 부르기 전까지만 유효하다.
 */
#ifndef CSSGSG_CONFIG_H
#define CSSGSG_CONFIG_H

#ifdef __cplusplus
extern "C" {
#endif

/* 설정 파일 내용(TOML)을 JSON으로. 적지 않은 설정은 기본값을 채운 전체. NULL이면 기본 설정.
 * 설정 오류면 NULL(cssgsg_config_error). */
const char *cssgsg_config_json(const char *toml);
/* JSON을 설정 파일 내용(설명이 달린 TOML, 기본값과 같은 설정은 주석)으로. 값도 검사한다. 오류면 NULL. */
const char *cssgsg_config_toml(const char *json);
/* 이 스레드의 마지막 오류(없으면 ""). */
const char *cssgsg_config_error(void);

#ifdef __cplusplus
}
#endif

#endif /* CSSGSG_CONFIG_H */

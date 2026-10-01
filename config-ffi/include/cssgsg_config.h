/*
 * cssgsg 설정 앱용 C ABI: 설정 파일(TOML) ↔ JSON, Mozc 사용자 사전 ↔ JSON. 구현: config-ffi/src/lib.rs (손으로 맞춰 둔다).
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
/* Mozc 사용자 사전 파일(user_dictionary.db)을 JSON으로:
 * {"dictionaries":[{"id":"…","name":"…","entries":[{"key","value","comment","pos","locale"}]}]}.
 * 파일이 없으면 빈 목록. 오류면 NULL. */
const char *cssgsg_userdict_json(const char *path);
/* JSON({"dictionaries":[…]})을 사용자 사전 파일에 쓴다(같은 id 사전의 모르는 필드는 지킨다). 성공 1, 오류 0. */
unsigned char cssgsg_userdict_save(const char *path, const char *json);

/* 맥 키코드의 설정 파일 키 이름("enter", "a", "f13" …). 수식키나 모르는 키면 NULL. 단축키 녹화에 쓴다. */
const char *cssgsg_config_key_name(unsigned short mac_keycode);

/* 윈도우 설정 앱(cssgsg_config.dll, C#이 부른다). 위의 둘과 같고, 기본값이 윈도우 기본값이다
 * (적지 않은 한자 단축키는 오른쪽 Control 탭, 그 값과 같으면 주석). */
const char *cssgsg_config_json_windows(const char *toml);
const char *cssgsg_config_toml_windows(const char *json);
/* 윈도우 스캔 코드(extended: 확장 키 비트 0/1)의 키 이름. 수식키는 "mod:shift_left" 꼴, 모르는 키는 NULL. */
const char *cssgsg_config_key_name_windows(unsigned short scan, unsigned char extended);

/* 이 스레드의 마지막 오류(없으면 ""). */
const char *cssgsg_config_error(void);

#ifdef __cplusplus
}
#endif

#endif /* CSSGSG_CONFIG_H */

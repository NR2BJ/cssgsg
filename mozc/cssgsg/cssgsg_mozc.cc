// cssgsg용 Mozc C API 구현. 설명은 cssgsg_mozc.h. ios/ios_engine.cc(프로세스 안에서 쓰는 선례)를 따른다.
#include "cssgsg/cssgsg_mozc.h"

#include <cstdint>
#include <memory>
#include <string>
#include <utility>

#include "absl/status/statusor.h"
#include "absl/strings/str_cat.h"
#include "absl/strings/str_format.h"
#include "absl/strings/string_view.h"
#include "absl/synchronization/mutex.h"
#include "base/strings/unicode.h"
#include "base/system_util.h"
#include "config/config_handler.h"
#include "data_manager/data_manager.h"
#include "engine/engine.h"
#include "engine/engine_interface.h"
#include "protocol/candidate_window.pb.h"
#include "protocol/commands.pb.h"
#include "protocol/config.pb.h"
#include "session/session_handler.h"

struct CssgsgMozc {
  std::unique_ptr<mozc::SessionHandler> handler;
  uint64_t session_id = 0;
  // 마지막으로 돌려준 문자열(다음 호출 전까지 유효).
  std::string returned;
  // 마지막 변환 화면(SELECT_ON_PAGE가 후보 id를 찾는 데 쓴다).
  mozc::commands::Output last;
  // 지금 설정(바꿀 때 다른 항목을 잃지 않게 사본을 든다).
  mozc::config::Config config;
  absl::Mutex mu;
};

namespace {

using ::mozc::commands::Command;
using ::mozc::commands::Input;
using ::mozc::commands::KeyEvent;
using ::mozc::commands::Output;
using ::mozc::commands::SessionCommand;

// Request.candidate_page_size 기본값.
constexpr int kPageSize = 9;

bool Eval(CssgsgMozc& m, Command& c) {
  c.mutable_input()->set_id(m.session_id);
  return m.handler->EvalCommand(&c);
}

bool Special(CssgsgMozc& m, KeyEvent::SpecialKey key, bool shift, Output* out) {
  Command c;
  c.mutable_input()->set_type(Input::SEND_KEY);
  KeyEvent* k = c.mutable_input()->mutable_key();
  k->set_special_key(key);
  if (shift) k->add_modifier_keys(KeyEvent::SHIFT);
  if (!Eval(m, c)) return false;
  if (out != nullptr) *out = c.output();
  return true;
}

bool Session(CssgsgMozc& m, SessionCommand::CommandType type, int32_t id, Output* out) {
  Command c;
  c.mutable_input()->set_type(Input::SEND_COMMAND);
  c.mutable_input()->mutable_command()->set_type(type);
  if (id >= 0) c.mutable_input()->mutable_command()->set_id(id);
  if (!Eval(m, c)) return false;
  if (out != nullptr) *out = c.output();
  return true;
}

// 변환·조합을 모두 버린다: REVERT를 조합이 없어질 때까지(변환 → 조합 → 빈 상태).
void Clear(CssgsgMozc& m) {
  for (int i = 0; i < 3; ++i) {
    Output o;
    if (!Session(m, SessionCommand::REVERT, -1, &o)) break;
    if (!o.has_preedit() || o.preedit().segment_size() == 0) break;
  }
  m.last.Clear();
}

void AppendJson(std::string& s, absl::string_view v) {
  s.push_back('"');
  for (const char c : v) {
    const unsigned char u = static_cast<unsigned char>(c);
    switch (c) {
      case '"': s += "\\\""; break;
      case '\\': s += "\\\\"; break;
      case '\n': s += "\\n"; break;
      case '\r': s += "\\r"; break;
      case '\t': s += "\\t"; break;
      default:
        if (u < 0x20) {
          absl::StrAppend(&s, absl::StrFormat("\\u%04x", u));
        } else {
          s.push_back(c);
        }
    }
  }
  s.push_back('"');
}

// 변환 화면 → JSON. 변환 중이 아니면(문절이 없으면) 빈 문자열.
std::string ViewJson(const Output& o) {
  if (!o.has_preedit() || o.preedit().segment_size() == 0) return "";
  std::string s = "{\"segments\":[";
  int focused = 0;
  for (int i = 0; i < o.preedit().segment_size(); ++i) {
    const auto& segment = o.preedit().segment(i);
    if (i > 0) s += ",";
    AppendJson(s, segment.value());
    if (segment.annotation() == mozc::commands::Preedit::Segment::HIGHLIGHT) focused = i;
  }
  absl::StrAppend(&s, "],\"focused\":", focused, ",\"candidates\":[");
  if (o.has_candidate_window() && o.candidate_window().candidate_size() > 0) {
    const auto& w = o.candidate_window();
    for (int i = 0; i < w.candidate_size(); ++i) {
      if (i > 0) s += ",";
      AppendJson(s, w.candidate(i).value());
    }
    const int first = static_cast<int>(w.candidate(0).index());
    s += "],\"selected\":";
    if (w.has_focused_index()) {
      absl::StrAppend(&s, static_cast<int>(w.focused_index()) - first);
    } else {
      s += "null";
    }
    const int pages = (static_cast<int>(w.size()) + kPageSize - 1) / kPageSize;
    absl::StrAppend(&s, ",\"page\":[", first / kPageSize + 1, ",", pages, "]}");
  } else {
    s += "],\"selected\":null,\"page\":null}";
  }
  return s;
}

void ApplyConfig(CssgsgMozc& m) {
  Command c;
  c.mutable_input()->set_type(Input::SET_CONFIG);
  *c.mutable_input()->mutable_config() = m.config;
  m.handler->EvalCommand(&c);
}

const char* Keep(CssgsgMozc& m, std::string s) {
  m.returned = std::move(s);
  return m.returned.c_str();
}

const char* ViewOrNull(CssgsgMozc& m, const Output& o) {
  std::string json = ViewJson(o);
  if (json.empty()) return nullptr;
  m.last = o;
  return Keep(m, std::move(json));
}

}  // namespace

extern "C" {

CssgsgMozc* cssgsg_mozc_new(const char* data_path, const char* profile_dir) {
  if (data_path == nullptr) return nullptr;
  if (profile_dir != nullptr && profile_dir[0] != '\0') {
    mozc::SystemUtil::SetUserProfileDirectory(profile_dir);
  }
  absl::StatusOr<std::unique_ptr<const mozc::DataManager>> data =
      mozc::DataManager::CreateFromFile(data_path);
  if (!data.ok()) return nullptr;
  absl::StatusOr<std::unique_ptr<mozc::Engine>> engine = mozc::Engine::CreateEngine(*std::move(data));
  if (!engine.ok()) return nullptr;

  auto* m = new CssgsgMozc;
  m->handler = std::make_unique<mozc::SessionHandler>(*std::move(engine));
  // MSIME 키맵: Shift+←→가 문절 길이. 입력 중 추천과 실시간 변환은 쓰지 않는다(코어가 읽기를 들고 있다).
  m->config = mozc::config::ConfigHandler::GetCopiedConfig();
  m->config.set_session_keymap(mozc::config::Config::MSIME);
  m->config.set_use_history_suggest(false);
  m->config.set_use_dictionary_suggest(false);
  m->config.set_use_realtime_conversion(false);
  // 음역 후보(ひらがな·カタカナ·半角…)를 "そのほかの文字種" 하위 목록으로 접지 않고 본 목록에 펼친다.
  m->config.set_use_cascading_window(false);
  ApplyConfig(*m);
  {
    Command c;
    c.mutable_input()->set_type(Input::CREATE_SESSION);
    if (!m->handler->EvalCommand(&c)) {
      delete m;
      return nullptr;
    }
    m->session_id = c.output().id();
  }
  Special(*m, KeyEvent::ON, false, nullptr);  // 입력기 켬(PRECOMPOSITION)
  return m;
}

void cssgsg_mozc_free(CssgsgMozc* m) {
  if (m == nullptr) return;
  {
    Command c;
    c.mutable_input()->set_type(Input::DELETE_SESSION);
    Eval(*m, c);
  }
  delete m;
}

const char* cssgsg_mozc_start(CssgsgMozc* m, const char* reading) {
  if (m == nullptr || reading == nullptr || reading[0] == '\0') return nullptr;
  absl::MutexLock lock(&m->mu);
  Clear(*m);
  // 읽기를 한 글자씩 key_string으로 넣는다(key_code 없이). 조합기는 그 글자를 그대로 조합에 넣는다.
  for (const absl::string_view ch : mozc::Utf8AsChars(reading)) {
    Command c;
    c.mutable_input()->set_type(Input::SEND_KEY);
    c.mutable_input()->mutable_key()->set_key_string(std::string(ch));
    if (!Eval(*m, c)) return nullptr;
  }
  Output o;
  if (!Special(*m, KeyEvent::SPACE, false, &o)) return nullptr;
  return ViewOrNull(*m, o);
}

const char* cssgsg_mozc_command(CssgsgMozc* m, int32_t command, int32_t arg) {
  if (m == nullptr) return nullptr;
  absl::MutexLock lock(&m->mu);
  Output o;
  bool ok = false;
  switch (command) {
    case CSSGSG_MOZC_NEXT: ok = Special(*m, KeyEvent::SPACE, false, &o); break;
    case CSSGSG_MOZC_PREV: ok = Special(*m, KeyEvent::UP, false, &o); break;
    case CSSGSG_MOZC_FOCUS_LEFT: ok = Special(*m, KeyEvent::LEFT, false, &o); break;
    case CSSGSG_MOZC_FOCUS_RIGHT: ok = Special(*m, KeyEvent::RIGHT, false, &o); break;
    case CSSGSG_MOZC_SHRINK: ok = Special(*m, KeyEvent::LEFT, true, &o); break;
    case CSSGSG_MOZC_EXPAND: ok = Special(*m, KeyEvent::RIGHT, true, &o); break;
    case CSSGSG_MOZC_NEXT_PAGE: ok = Session(*m, SessionCommand::CONVERT_NEXT_PAGE, -1, &o); break;
    case CSSGSG_MOZC_PREV_PAGE: ok = Session(*m, SessionCommand::CONVERT_PREV_PAGE, -1, &o); break;
    case CSSGSG_MOZC_SELECT_ON_PAGE: {
      if (!m->last.has_candidate_window()) return nullptr;
      const auto& w = m->last.candidate_window();
      if (arg < 0 || arg >= w.candidate_size()) return nullptr;
      ok = Session(*m, SessionCommand::SELECT_CANDIDATE, w.candidate(arg).id(), &o);
      break;
    }
    default:
      return nullptr;
  }
  if (!ok) return nullptr;
  return ViewOrNull(*m, o);
}

const char* cssgsg_mozc_commit(CssgsgMozc* m) {
  if (m == nullptr) return "";
  absl::MutexLock lock(&m->mu);
  Output o;
  const bool ok = Session(*m, SessionCommand::SUBMIT, -1, &o);
  m->last.Clear();
  return Keep(*m, ok && o.has_result() ? o.result().value() : "");
}

void cssgsg_mozc_cancel(CssgsgMozc* m) {
  if (m == nullptr) return;
  absl::MutexLock lock(&m->mu);
  Clear(*m);
}

void cssgsg_mozc_set_learning(CssgsgMozc* m, int32_t enabled) {
  if (m == nullptr) return;
  absl::MutexLock lock(&m->mu);
  m->config.set_history_learning_level(enabled != 0 ? mozc::config::Config::DEFAULT_HISTORY
                                                    : mozc::config::Config::NO_HISTORY);
  ApplyConfig(*m);
}

}  // extern "C"

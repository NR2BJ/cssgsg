// cssgsg Mozc C API 확인용 명령줄 도구.
//   cssgsg_mozc_main <mozc.data> <프로필 폴더> <읽기> [명령…]
// 명령: next prev left right shrink expand pgdn pgup sel:N commit cancel
// 매 단계의 화면 JSON과 걸린 시간(밀리초)을 한 줄씩 찍는다.
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>

#include "cssgsg/cssgsg_mozc.h"

namespace {
double Ms(std::chrono::steady_clock::time_point since) {
  return std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - since).count();
}
void Print(const char* step, const char* json, double ms) {
  std::printf("%s\t%.1fms\t%s\n", step, ms, json != nullptr ? json : "(null)");
}
}  // namespace

int main(int argc, char** argv) {
  if (argc < 4) {
    std::fprintf(stderr, "usage: %s <mozc.data> <profile_dir> <reading> [commands...]\n", argv[0]);
    return 2;
  }
  auto t = std::chrono::steady_clock::now();
  CssgsgMozc* m = cssgsg_mozc_new(argv[1], argv[2]);
  if (m == nullptr) {
    std::fprintf(stderr, "cssgsg_mozc_new failed\n");
    return 1;
  }
  Print("new", "", Ms(t));
  t = std::chrono::steady_clock::now();
  Print("start", cssgsg_mozc_start(m, argv[3]), Ms(t));
  for (int i = 4; i < argc; ++i) {
    const std::string cmd = argv[i];
    t = std::chrono::steady_clock::now();
    if (cmd == "commit") {
      Print("commit", cssgsg_mozc_commit(m), Ms(t));
      continue;
    }
    if (cmd == "cancel") {
      cssgsg_mozc_cancel(m);
      Print("cancel", "", Ms(t));
      continue;
    }
    int code = -1, arg = 0;
    if (cmd == "next") code = CSSGSG_MOZC_NEXT;
    else if (cmd == "prev") code = CSSGSG_MOZC_PREV;
    else if (cmd == "left") code = CSSGSG_MOZC_FOCUS_LEFT;
    else if (cmd == "right") code = CSSGSG_MOZC_FOCUS_RIGHT;
    else if (cmd == "shrink") code = CSSGSG_MOZC_SHRINK;
    else if (cmd == "expand") code = CSSGSG_MOZC_EXPAND;
    else if (cmd == "pgdn") code = CSSGSG_MOZC_NEXT_PAGE;
    else if (cmd == "pgup") code = CSSGSG_MOZC_PREV_PAGE;
    else if (cmd.rfind("sel:", 0) == 0) { code = CSSGSG_MOZC_SELECT_ON_PAGE; arg = std::atoi(cmd.c_str() + 4); }
    else if (cmd.rfind("start:", 0) == 0) { Print("start", cssgsg_mozc_start(m, cmd.c_str() + 6), Ms(t)); continue; }
    if (code < 0) {
      std::fprintf(stderr, "unknown command %s\n", cmd.c_str());
      continue;
    }
    Print(cmd.c_str(), cssgsg_mozc_command(m, code, arg), Ms(t));
  }
  cssgsg_mozc_free(m);
  return 0;
}

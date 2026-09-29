# 맥 설치 뒤 확인 목록

세 배열을 몰라도 확인할 수 있게 "이 키를 누르면 이게 보여야 한다" 꼴로 적었다.
키는 키캡에 적힌 쿼티 글자 그대로 누른다. 메뉴 막대의 cssgsg 아이콘(A / 한 / あ)이 지금 모드다.

"엔진 확인" 칸은 같은 입력을 엔진(러스트 시뮬레이터)으로 친 결과다. `cargo test --test mac_checklist`가
이 칸을 읽어 엔진으로 다시 쳐 보므로, 여기 적힌 기대값은 엔진과 어긋나지 않는다.
사람이 확인하는 것은 엔진이 아니라 **입력기가 앱과 주고받는 부분**(글자 넣기, 밑줄, 단축키, 클릭, 비밀번호 칸)이다.

| # | 모드·장소 | 누를 키 | 보여야 하는 것 | 엔진 확인 |
|---|---|---|---|---|
| 1 | 설치 직후 | 입력 메뉴에서 cssgsg(ㅊ 아이콘) 선택. 없으면 시스템 설정 → 키보드 → 입력 소스 → + → 영어 목록 | 메뉴 막대에 **A** | — |
| 2 | A (텍스트 편집기) | `j l w w i` | hello | `en:jlwwi` → `hello` |
| 3 | A | `Shift+j`, `l w w i` | Hello | `en:Jlwwi` → `Hello` |
| 4 | A | `'` 키 | , (쉼표) | `en:'` → `,` |
| 5 | A | 오른쪽 Shift를 톡 | **한** | `en:{rs}jfs` → `안` |
| 6 | 한 | `j f s m t d h f l e j a` | 안녕하세요 (치는 동안 마지막 글자에 밑줄) | `ko:jfsmtdhfleja` → `안녕하세요` |
| 7 | 한 | `j f s` 후 Backspace 두 번 | 안 → 아 → ㅇ (한 타씩 지워짐) | `ko:jfs{bs}` → `아`, `ko:jfs{bs}{bs}` → `ㅇ` |
| 8 | 한 | 왼쪽 Shift를 톡 | **あ** | `ko:{ls}ckeuwl` → `にほんご` |
| 9 | あ | `c k e u w l` 후 Enter | にほんご (밑줄이 있다가 Enter로 확정) | `ja:ckeuwl{ent}` → `にほんご` |
| 10 | あ | `c k e u w l` 후 Space, Space | 후보창 1.にほんご 2.ニホンゴ → 두 번째 Space에 ニホンゴ | `ja:ckeuwl{sp}{sp}` → `ニホンゴ` |
| 11 | あ | Caps Lock 켜고 `c k e u w l` | ニホンゴ. 오른쪽 Shift를 톡 해서 A로 가면 Caps Lock 불이 저절로 꺼짐 | `ja:{caps}ckeuwl` → `ニホンゴ` |
| 12 | 아무 모드 | 오른쪽 Shift를 1초 꾹 눌렀다 뗌 | 전환 안 됨 | — (탭 시간은 `tools/mac/shell-smoke`가 확인) |
| 13 | 한 | `j f s` 후 ⌘A | "안"이 확정되고 전체 선택 | `ko:jfs{M-a}` → `안` |
| 14 | 한 | `j f` 치고 다른 곳을 클릭 | "아"가 남고, 두 번 찍히지 않음 | `ko:jf{click}` → `아` |
| 15 | 한 (Chrome이나 Codex 입력창) | `j f s` 후 Shift+Enter | "안" 다음에 줄바꿈, 안이 사라지지 않음 | `ko:jfs{S-ent}` → `안\n` |
| 16 | 한 (웹 비밀번호 칸, 눈 아이콘으로 보기) | `j f s` | jfs (쿼티 그대로) | — (비밀번호 칸 판정은 앱에서만) |
| 17 | 메뉴 막대 cssgsg 메뉴 | 메뉴 열기 → 업데이트 확인 | 맨 위에 버전, 다시 열면 "최신 버전 사용 중" | — |

틀린 항목이 있으면 번호를 알려 준다. 재현이 필요하면 개발자 기록을 켠다(키 코드·수식키·시각만 남고 글자 내용은 남지 않는다).

```bash
defaults write com.cssgsg.inputmethod.app developerMode -bool true
```

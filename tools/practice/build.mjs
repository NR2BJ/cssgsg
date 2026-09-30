// 타자 연습 페이지(practice/index.html)를 만든다. 참신세벌식 v18(한국어), Graphite(영어), 新月(일본어)를 한 파일에 담는다.
// 화면과 동작은 practice/template.html, 연습 글은 tools/practice/{ko,en,ja}-{words,lines}.txt.
//
// - 글자(음절·가나·영문자)마다 치는 방법(권장부터)과 한 타씩의 화면은 엔진이 만든다(cssgsg-cli encode·batch·layout-json).
//   페이지는 조합기를 따로 갖지 않고 이 표로 누른 키를 맞춰 본다. 그래서 입력기와 어긋나지 않는다.
// - 단계마다 그 단계까지 익힌 자리만으로 된 글자·낱말·줄을 고른다.
// - 쓰는 낱말·줄·연습 글·예시를 권장 입력으로 엔진에 쳐서 그대로 나오는지 본다. 하나라도 틀리면 실패한다.
//
// 사용
//   node tools/practice/build.mjs           practice/index.html을 쓴다
//   node tools/practice/build.mjs --check   확인하고 practice/index.html이 최신인지 본다(쓰지 않음)
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const CHECK = process.argv.includes("--check");
const env = { ...process.env, PATH: `${process.env.HOME}/.cargo/bin:${process.env.PATH}` };
execFileSync("cargo", ["build", "-q", "-p", "cssgsg-cli"], { cwd: repo, env, stdio: "inherit" });
const CLI = path.join(repo, "build/cargo/debug/cssgsg-cli");
const run = (args, input = "") => execFileSync(CLI, args, { input, encoding: "utf8", maxBuffer: 256 << 20 });
const cliLines = (args, input) => run(args, input).trimEnd().split("\n").filter(Boolean);
const layoutJson = (id) => JSON.parse(run(["layout-json", id]));

const fail = (msg) => {
  console.error(msg);
  process.exit(1);
};
const unique = (xs) => [...new Set(xs)];
const src = (name) => fs.readFileSync(path.join(repo, "tools/practice", name), "utf8");
const readTokens = (name) => src(name).split("\n").map((l) => l.replace(/#.*/, "")).flatMap((l) => l.split(/\s+/)).filter(Boolean);
const readLines = (name) => src(name).split("\n").map((l) => l.trim()).filter((l) => l && !l.startsWith("#"));

// ---------------------------------------------------------------- 키와 엔진

// 키열 표기(cssgsg_core::sim과 같음): 쿼티 자리 글자 하나가 한 타, 대문자·Shift 기호는 Shift를 누른 타, 공백은 스페이스.
const SHIFTED = { "`": "~", 1: "!", 2: "@", 3: "#", 4: "$", 5: "%", 6: "^", 7: "&", 8: "*", 9: "(", 0: ")", "-": "_", "=": "+",
  "[": "{", "]": "}", "\\": "|", ";": ":", "'": '"', ",": "<", ".": ">", "/": "?" };
const BASE_OF = Object.fromEntries(Object.entries(SHIFTED).map(([b, s]) => [s, b]));
const shiftedOf = (key) => (/[a-z]/.test(key) ? key.toUpperCase() : SHIFTED[key]);
const baseKey = (c) => (/[A-Z]/.test(c) ? c.toLowerCase() : BASE_OF[c] ?? c);
const LEFT = new Set([..."`12345qwertasdfgzxcvb"]);
// 아직 안 익힌 문장부호를 뺀 줄(페이지도 같은 방법으로 뺀다). lower면 대문자도 소문자로.
const plainOf = (text, strip, lower = false) => {
  const t = [...text].filter((c) => !strip.includes(c)).join("").replace(/ +/g, " ").trim();
  return lower ? t.toLowerCase() : t;
};
// 키열 문법에서 {는 특수 키 이름을 여므로 Shift+[·Shift+]는 {S-[}·{S-]}로 쓴다.
const simKey = (c) => ({ " ": "{sp}", "{": "{S-[}", "}": "{S-]}" })[c] ?? c;

function batch(mode, keyLists, layout) {
  if (!keyLists.length) return [];
  const args = ["batch", "--mode", mode, ...(layout ? ["--ko-layout", layout] : [])];
  return cliLines(args, keyLists.map((k) => [...k].map(simKey).join("")).join("\n") + "\n").map((l) => JSON.parse(l));
}
// 키열마다 앞부분(한 타, 두 타, …)을 쳐서 한 타씩의 화면.
function screensOf(mode, keyLists, layout) {
  const prefixes = keyLists.flatMap((keys) => [...keys].map((_, i) => [...keys].slice(0, i + 1).join("")));
  const outs = batch(mode, prefixes, layout);
  let i = 0;
  return keyLists.map((keys) => {
    const s = outs.slice(i, i + [...keys].length);
    i += [...keys].length;
    return s;
  });
}
// 글을 권장 입력으로 쳐서 그대로 나오는지. toKeys(글) → 키열.
function verify(label, mode, texts, toKeys, layout) {
  const outs = batch(mode, texts.map(toKeys), layout);
  const bad = texts.map((t, i) => [t, outs[i]]).filter(([t, o]) => o !== t);
  if (bad.length) fail(`${label}: 권장 입력으로 친 결과가 다르다 ${bad.length}개:\n  ${bad.slice(0, 20).map(([t, o]) => `${t} → ${toKeys(t)} → 엔진 ${JSON.stringify(o)}`).join("\n  ")}`);
  return texts.length;
}

// ---------------------------------------------------------------- 한국어: 참신세벌식 v18

const CHO = [..."ㄱㄲㄴㄷㄸㄹㅁㅂㅃㅅㅆㅇㅈㅉㅊㅋㅌㅍㅎ"];
const JUNG = [..."ㅏㅐㅑㅒㅓㅔㅕㅖㅗㅘㅙㅚㅛㅜㅝㅞㅟㅠㅡㅢㅣ"];
const JONG = [..."ㄱㄲㄳㄴㄵㄶㄷㄹㄺㄻㄼㄽㄾㄿㅀㅁㅂㅄㅅㅆㅇㅈㅊㅋㅌㅍㅎ"];
const isSyllable = (c) => c >= "가" && c <= "힣";
function decompose(c) {
  const i = c.codePointAt(0) - 0xac00;
  const t = i % 28;
  return { cho: CHO[Math.floor(i / 588)], jung: JUNG[Math.floor((i % 588) / 28)], jong: t ? JONG[t - 1] : null };
}
const compose = (cho, jung, jong = null) =>
  String.fromCodePoint(0xac00 + (CHO.indexOf(cho) * 21 + JUNG.indexOf(jung)) * 28 + (jong ? JONG.indexOf(jong) + 1 : 0));

const KO_LAYOUT = "chamshin-v18";
const KO_PUNCT = ".,?!";
const KO_SYMS = "/;"; // 쿼티와 자리가 다른 기호(Shift+L, Shift+P)
// 받침 단계에서 실제 음절이 적은 받침을 채울 때 쓰는 초성·모음(고정 순서라 빌드 결과가 늘 같다).
const GEN_CHO = [..."ㅇㄱㄴㄷㅁㅂㅅㅈㅎ"];
const GEN_JUNG = [..."ㅏㅓㅗㅜㅡㅣ"];
// ㅗ·ㅜ·ㅢ 규칙 단계: 같은 키열에서 한 키만 다른 짝(왼손 ㅗ·ㅜ + 받침 ↔ 오른손 ㅗ·ㅜ + 모음).
const KO_OO = ["곧", "과", "곤", "괘", "공", "괴", "궂", "궈", "국", "궤", "궁", "귀", "돈", "돼", "동", "되", "둥", "뒤", "응", "의",
  "관", "권", "왕", "원", "광", "괜", "된", "뒷", "월", "왼", "웬", "윗", "활", "확", "훨", "휠"];
const KO_PUNCT_ITEMS = ["다.", "요.", "요?", "까?", "네!", "죠?", "고,", "서,", "며,", "지만,", "니다.", "세요.", "어요?", "아요!", "그래?", "좋아!", "네, 알겠습니다.",
  "예/아니요", "남/여", "위/아래", "첫째; 둘째"];
// 자음·모음만: ❖(Shift+N)로 조합을 끊어야 하는 것과 그냥 치는 것.
const KO_JAMO = ["ㅋㅋ", "ㅋㅋㅋ", "ㅎㅎ", "ㅎㅎㅎ", "ㅇㅋ", "ㄱㄱ", "ㅇㅇ", "ㄴㄴ", "ㅈㅅ", "ㄱㅅ", "ㅊㅋ", "ㅂㅂ", "ㅅㄱ", "ㄷㄷ", "ㅠㅠ", "ㅜㅜ", "ㅎㅇ"];
const KO_JAMO_LINES = ["ㅋㅋㅋ 너무 웃겨", "ㅇㅋ 내일 봐", "ㄱㄱ 지금 가자", "ㅠㅠ 아쉽다", "ㅎㅎ 고마워", "ㅇㅇ 알겠어", "ㅈㅅ 좀 늦었어",
  "ㄱㅅ 잘 먹을게", "ㅊㅋ 생일 축하해", "ㄷㄷ 대단하다", "ㅜㅜ 배고파", "ㅂㅂ 잘 자"];

// 받침 없이 홈열 → 윗열 → 아랫열 → 겹모음 → 된소리, 받침도 같은 순서, 그다음 규칙과 실전(사용자 요청, 2026-09-30).
// cho·jung·jong은 이 단계에서 새로 익히는 낱자. ex는 규칙 설명 아래 보이는 예시.
const KO_LESSONS = [
  { id: "home1", part: "받침 없이", title: "홈열 ① ㅇ ㄱ · ㅏ ㅣ", cho: "ㅇㄱ", jung: "ㅏㅣ", ex: ["아", "가", "이", "기"],
    blurb: "오른손은 초성, 왼손은 모음입니다. 음절은 늘 오른손 초성부터 칩니다. 받침이 없으면 초성 다음 모음 한 번으로 끝납니다. 검지를 F와 J에 올려 두세요." },
  { id: "home2", part: "받침 없이", title: "홈열 ② ㅎ ㅅ ㅂ · ㅐ ㅗ ㅛ", cho: "ㅎㅅㅂ", jung: "ㅐㅗㅛ", ex: ["하", "새", "보", "효"],
    blurb: "홈열의 나머지 자리입니다. ㅗ는 왼손 G입니다. 오른손 O에도 ㅗ가 있지만 겹모음(ㅘ ㅙ ㅚ)에 씁니다." },
  { id: "top-cho", part: "받침 없이", title: "윗열 초성 ㅊ ㅁ ㄷ ㄹ ㅌ", cho: "ㅊㅁㄷㄹㅌ", ex: ["차", "마", "다", "라", "타"],
    blurb: "오른손 윗열의 초성입니다. O 자리는 초성 ㄹ입니다." },
  { id: "top-jung", part: "받침 없이", title: "윗열 모음 ㅠ ㅢ ㅔ ㅜ ㅕ", jung: "ㅠㅢㅔㅜㅕ", ex: ["유", "의", "게", "구", "겨"],
    blurb: "왼손 윗열의 모음입니다. ㅢ는 W 한 번입니다. ㅜ는 왼손 R입니다." },
  { id: "bot-cho", part: "받침 없이", title: "아랫열 초성 ㅋ ㅈ ㄴ ㅍ", cho: "ㅋㅈㄴㅍ", ex: ["카", "자", "나", "파"],
    blurb: "오른손 아랫열의 초성입니다. / 자리가 ㅍ입니다." },
  { id: "bot-jung", part: "받침 없이", title: "아랫열 모음 ㅖ ㅒ ㅡ ㅓ", jung: "ㅖㅒㅡㅓ", ex: ["계", "걔", "그", "거"],
    blurb: "왼손 아랫열의 모음입니다. 이제 왼손 모음 자리를 모두 익혔습니다." },
  { id: "ya", part: "받침 없이", title: "ㅑ는 초성 뒤 B", jung: "ㅑ", ex: ["갸", "야", "캬"],
    blurb: "ㅑ는 초성 바로 뒤에 B를 칩니다. 초성 없이 B를 치면 초성 ㅋ이라서, 캬는 B를 두 번 칩니다." },
  { id: "wa", part: "받침 없이", title: "겹모음 ㅘ ㅙ ㅚ", jung: "ㅘㅙㅚ", ex: ["과", "왜", "괴", "로"],
    blurb: "오른손 ㅗ(O)를 치고 왼손 ㅏ ㅐ ㅣ를 칩니다. O는 초성 바로 뒤에서만 ㅗ가 되고, 그 밖에는 초성 ㄹ입니다. 홑모음 ㅗ는 계속 왼손 G로 칩니다." },
  { id: "wo", part: "받침 없이", title: "겹모음 ㅝ ㅞ ㅟ", jung: "ㅝㅞㅟ", ex: ["궈", "웨", "뒤", "두"],
    blurb: "오른손 ㅜ(.)를 치고 왼손 ㅓ ㅔ ㅣ를 칩니다. .는 초성 바로 뒤에서만 ㅜ가 됩니다. 홑모음 ㅜ는 계속 왼손 R로 칩니다." },
  { id: "double", part: "받침 없이", title: "된소리 ㄲ ㄸ ㅃ ㅆ ㅉ", cho: "ㄲㄸㅃㅆㅉ", ex: ["까", "따", "빠", "싸", "짜"],
    blurb: "ㄲ ㄸ ㅃ ㅆ은 ㅇ(J)을 먼저 치고 홑자음을 칩니다. ㅉ만 ㅈ(N) 다음에 ㅇ(J)입니다. 같은 키를 두 번 쳐도 됩니다." },
  { id: "jong-home", part: "받침", title: "받침 홈열 ㅁ ㄴ ㅇ ㄷ ㅍ", jong: "ㅁㄴㅇㄷㅍ", ex: ["감", "안", "강", "곧", "앞"],
    blurb: "왼손 키는 모음 뒤에 치면 받침이 됩니다(갈마들이). 모음 자리 그대로라서 새로 찾을 키는 없고, 키마다 받침이 무엇인지 익히면 됩니다." },
  { id: "jong-top", part: "받침", title: "받침 윗열 ㅆ ㄹ ㄱ ㅌ ㅊ", jong: "ㅆㄹㄱㅌㅊ", ex: ["있", "말", "각", "밭", "꽃"],
    blurb: "윗열 받침입니다. 받침 ㅆ은 Q 한 번입니다." },
  { id: "jong-bot", part: "받침", title: "받침 아랫열 ㅂ ㅅ ㅎ ㅈ", jong: "ㅂㅅㅎㅈ", ex: ["밥", "옷", "좋", "낮"],
    blurb: "아랫열 받침입니다. 이제 홑받침을 모두 칩니다." },
  { id: "jong-kk", part: "받침", title: "받침 ㄲ ㅋ", jong: "ㄲㅋ", ex: ["밖", "깎", "부엌"],
    blurb: "받침 ㄲ은 ㄱ(E)을 두 번, 받침 ㅋ은 ㄱ(E) 다음 ㅁ(A)입니다. 받침 ㅋ은 Shift+B로도 됩니다." },
  { id: "jong-pair1", part: "받침", title: "겹받침 ㄳ ㄵ ㄶ ㅄ", jong: "ㄳㄵㄶㅄ", ex: ["몫", "앉", "않", "없"],
    blurb: "원작자가 권하는 조합입니다. ㄳ은 ㄱ+ㅆ, ㄵ은 ㄱ+ㄴ, ㄶ은 ㅇ+ㄴ, ㅄ은 ㅅ+ㅂ(거꾸로)입니다. 표준 순서(ㄱ+ㅅ, ㄴ+ㅈ, ㄴ+ㅎ, ㅂ+ㅅ)로 쳐도 됩니다." },
  { id: "jong-pair2", part: "받침", title: "겹받침 ㄺ ㄻ ㄼ ㄽ ㄾ ㄿ ㅀ", jong: "ㄺㄻㄼㄽㄾㄿㅀ", ex: ["읽", "삶", "넓", "곬", "핥", "읊", "싫"],
    blurb: "ㄹ이 든 겹받침입니다. ㄺ은 ㄱ+ㄹ(거꾸로), ㄻ은 ㄹ+ㅁ, ㄼ은 ㄴ+ㅁ, ㄽ은 ㄹ+ㅆ, ㄾ은 ㄹ+ㅌ, ㄿ은 ㅇ+ㅁ, ㅀ은 ㅇ+ㄹ입니다. 표준 순서로 쳐도 됩니다." },
  { id: "oo", part: "규칙 다지기", title: "왼손 ㅗ·ㅜ와 오른손 ㅗ·ㅜ", drill: KO_OO, ex: ["곧", "과", "궁", "귀", "응", "의"],
    rule: (s) => { const d = decompose(s); return "ㅘㅙㅚㅝㅞㅟㅢ".includes(d.jung) || ("ㅗㅜㅡ".includes(d.jung) && d.jong !== null); },
    chips: [["ㅗ", "받침 앞", "g"], ["ㅗ", "겹모음", "o"], ["ㅜ", "받침 앞", "r"], ["ㅜ", "겹모음", "."], ["ㅢ", "모음", "w"]],
    blurb: "ㅗ·ㅜ 다음에 받침이 오면 왼손 ㅗ(G)·ㅜ(R), 겹모음이 되면 오른손 ㅗ(O)·ㅜ(.)입니다. 왼손 ㅗ·ㅜ 다음의 왼손 키는 받침이 되기 때문입니다. 같은 까닭으로 ㅢ는 W이고, ㅡ(C) 다음 ㅣ(D)는 받침 ㅇ이 됩니다." },
  { id: "punct", part: "규칙 다지기", title: "문장부호 . , ? !", punct: true, drill: KO_PUNCT_ITEMS, ex: ["다.", "요?", "네!", "고,"], modes: ["drill", "lines"],
    chips: [[".", "마침표", "."], [",", "쉼표", ","], ["?", "물음표", "?"], ["!", "느낌표", "!"], ["/", "빗금", "L"], [";", "쌍반점", "P"]],
    blurb: ". 자리는 초성 바로 뒤가 아니면 마침표입니다. 쉼표는 , 자리, 물음표는 Shift+/, 느낌표는 Shift+1입니다. 빗금 /은 Shift+L, 쌍반점 ;은 Shift+P로 쿼티와 자리가 다릅니다." },
  { id: "jamo", part: "규칙 다지기", title: "자음·모음만 ❖", stop: true, drill: KO_JAMO, ex: ["ㅋㅋ", "ㅇㅋ", "ㄱㄱ", "ㅎㅎ", "ㅠㅠ"],
    chips: [["❖", "조합 끊기", "N"]], modes: ["drill", "lines"],
    blurb: "자음만 칠 때 뒤 키가 앞 자음과 합쳐지면 사이에 ❖(Shift+N)를 넣습니다. ㅋ 다음 B는 ㅑ, ㄱ 다음 K는 ㄲ이 되기 때문입니다. 합쳐지지 않는 ㅎㅎ, ㅠㅠ는 그냥 칩니다." },
  { id: "words", part: "실전", title: "낱말", modes: ["words", "lines"], blurb: "익힌 자리를 모두 써서 낱말을 칩니다." },
  { id: "lines", part: "실전", title: "짧은 문장", modes: ["lines"], blurb: "문장을 칩니다. 쉼표·마침표·물음표도 함께 칩니다." },
];

function buildKo() {
  const layout = layoutJson(KO_LAYOUT);
  const get = (key, shift) => layout.keys.find((k) => k.key === key && k.shift === shift);

  // 배열도: 키마다 큰 글자(main), 오른쪽 아래(sub), 왼쪽 위(shift), 오른쪽 위(corner). id는 단계에서 켜고 끄는 이름.
  const board = {};
  for (const key of unique(layout.keys.map((k) => k.key))) {
    const b = get(key, false), s = get(key, true);
    const spec = {};
    if (b.passthrough != null) spec.main = { t: b.passthrough, c: "plain", id: `pass:${b.passthrough}` };
    else if (b.cho && b.jung) Object.assign(spec, { main: { t: b.cho, c: "cho", id: `cho:${b.cho}` }, sub: { t: b.jung, c: "jung", id: `rjung:${b.jung}` } });
    else if (b.sym && b.jung) Object.assign(spec, { main: { t: b.sym, c: "sym", id: `sym:${b.sym}` }, sub: { t: b.jung, c: "jung", id: `rjung:${b.jung}` } });
    else if (b.jung && b.jong) Object.assign(spec, { main: { t: b.jung, c: "jung", id: `jung:${b.jung}` }, sub: { t: b.jong, c: "jong", id: `jong:${b.jong}` } });
    else if (b.cho) spec.main = { t: b.cho, c: "cho", id: `cho:${b.cho}` };
    else if (b.jung) spec.main = { t: b.jung, c: "jung", id: `jung:${b.jung}` };
    else if (b.sym) spec.main = { t: b.sym, c: "sym", id: `sym:${b.sym}` };
    if (s.passthrough != null) spec.shift = { t: s.passthrough, c: "plain", id: `pass:${s.passthrough}` };
    else if (s.action === "stop") spec.shift = { t: "❖", c: "sym", id: "stop" };
    else if (s.sym) spec.shift = { t: s.sym, c: "sym", id: `sym:${s.sym}` };
    if (s.jong) spec.corner = { t: s.jong, c: "jong", id: `alt:${s.jong}` };
    board[key] = spec;
  }
  const choKey = {}, jungKey = {};
  for (const k of layout.keys) {
    if (k.shift) continue;
    if (k.cho) choKey[k.cho] ??= k.key;
    if (k.jung && !k.cho && !k.sym) jungKey[k.jung] ??= k.key;
  }

  // 연습 글
  const words = unique(readTokens("ko-words.txt"));
  for (const w of words) if (![...w].every(isSyllable)) fail(`ko-words.txt: 한글 음절이 아닌 글자: ${w}`);
  const lines = readLines("ko-lines.txt");
  for (const l of lines) if (![...l].every((c) => isSyllable(c) || ` ${KO_PUNCT}`.includes(c))) fail(`ko-lines.txt: 쓸 수 없는 글자: ${l}`);
  const allLines = [...lines, ...KO_JAMO_LINES];
  const realSyllables = unique([...words.join(""), ...lines.join("")].filter(isSyllable)).sort();

  // 음절마다 치는 방법(엔진). 초성 × 모음 전부, 받침 단계의 채움 음절, 연습 글과 예시의 음절, 새 낱자를 보일 음절.
  const need = new Set(realSyllables);
  for (const c of CHO) for (const v of JUNG) need.add(compose(c, v));
  for (const t of JONG) for (const v of GEN_JUNG) for (const c of GEN_CHO) need.add(compose(c, v, t));
  for (const t of JONG) need.add(compose("ㅇ", "ㅏ", t));
  for (const L of KO_LESSONS) for (const x of [...(L.drill ?? []), ...(L.ex ?? [])]) for (const c of x) if (isSyllable(c)) need.add(c);
  for (const x of KO_JAMO_LINES) for (const c of x) if (isSyllable(c)) need.add(c);
  const units = {};
  for (const line of cliLines(["encode", "--ko-layout", KO_LAYOUT], [...need].join("\n") + "\n")) {
    const { s, ways } = JSON.parse(line);
    if (!ways.length) fail(`${s}: 치는 방법이 없다`);
    units[s] = ways.map((w) => [w.keys, w.screens]);
  }
  for (const s of need) if (!units[s]) fail(`${s}: 엔진이 방법을 주지 않았다`);
  for (const p of KO_PUNCT) units[p] = [[p, [p]]];
  for (const c of KO_SYMS) {
    const k = layout.keys.find((x) => x.shift && x.sym === c) ?? fail(`${KO_LAYOUT}에 Shift 기호 ${c}가 없다`);
    units[c] = [[shiftedOf(k.key), [c]]];
  }

  // 자음·모음만 쓰는 연습: 낱자마다 그 키, 사이마다 ❖(N)를 넣거나 말거나. 엔진이 그대로 내는 것만, 짧은 것부터.
  for (const item of unique([...KO_JAMO, ...KO_JAMO_LINES.map((l) => l.split(" ")[0])])) {
    const keys = [...item].map((j) => choKey[j] ?? jungKey[j] ?? fail(`${item}: ${j}의 키가 없다`));
    const cands = [];
    for (let mask = 0; mask < 1 << (keys.length - 1); mask++) {
      cands.push(keys.map((k, i) => (i && mask & (1 << (i - 1)) ? "N" : "") + k).join(""));
    }
    const outs = batch("ko", cands, KO_LAYOUT);
    const ok = cands.filter((_, i) => outs[i] === item).sort((a, b) => a.length - b.length);
    if (!ok.length) fail(`${item}: 칠 방법이 없다`);
    const scr = screensOf("ko", ok, KO_LAYOUT);
    units[item] = ok.map((k, i) => [k, scr[i]]);
  }

  // 글 → 글자 단위. 자음·모음 연습(ㅋㅋ 등)은 한 덩어리.
  const tokens = (text) => {
    const out = [];
    for (let i = 0; i < text.length; ) {
      const t = [3, 2].map((n) => text.slice(i, i + n)).find((x) => x.length > 1 && units[x] && !isSyllable(x[0])) ?? text[i];
      out.push(t);
      i += t.length;
    }
    return out;
  };
  const toKeys = (text) => tokens(text).map((t) => (t === " " ? " " : units[t]?.[0][0] ?? fail(`${text}: ${t}를 칠 수 없다`))).join("");

  // 새 낱자를 치는 키: 초성은 ㅏ 앞, 모음은 ㅇ 뒤, 받침은 ㅇㅏ 뒤에 붙여 친 음절에서 그 낱자의 키만 떼어 낸다.
  const aKeys = units["아"][0][0];
  const keysOf = (part, j) => {
    const s = part === "cho" ? compose(j, "ㅏ") : part === "jung" ? compose("ㅇ", j) : compose("ㅇ", "ㅏ", j);
    const k = units[s]?.[0][0] ?? fail(`${s}: 방법이 없다`);
    return part === "cho" ? k.slice(0, k.length - 1) : part === "jung" ? k.slice(aKeys.length - 1) : k.slice(aKeys.length);
  };

  // 단계
  const ROLE = { cho: "초성", jung: "모음", jong: "받침" };
  const SINGLE_JONG = new Set([..."ㄱㄴㄷㄹㅁㅂㅅㅆㅇㅈㅊㅌㅍㅎ"]);
  let A = { cho: new Set(), jung: new Set(), jong: new Set(), punct: false, stop: false };
  const lessons = [];
  const drillSeen = new Set();
  for (const L of KO_LESSONS) {
    A = {
      cho: new Set([...A.cho, ...(L.cho ?? "")]),
      jung: new Set([...A.jung, ...(L.jung ?? "")]),
      jong: new Set([...A.jong, ...(L.jong ?? "")]),
      punct: A.punct || !!L.punct,
      stop: A.stop || !!L.stop,
    };
    const fits = (s) => {
      const d = decompose(s);
      return A.cho.has(d.cho) && A.jung.has(d.jung) && (d.jong === null || A.jong.has(d.jong));
    };
    const fresh = (s) => {
      const d = decompose(s);
      return !!(L.cho?.includes(d.cho) || L.jung?.includes(d.jung) || (d.jong && L.jong?.includes(d.jong)) || L.rule?.(s));
    };
    const textOk = (t) => [...t].every((c) => c === " " || (isSyllable(c) ? fits(c) : (KO_PUNCT + KO_SYMS).includes(c) && A.punct));

    // 자리 익히기: 받침 없는 단계는 초성 × 모음 전부, 받침 단계는 그 받침이 든 실제 음절(모자라면 채움 음절)과 복습용 음절.
    let drill;
    if (L.drill) {
      drill = { items: L.drill, fresh: L.drill };
    } else if (!A.jong.size) {
      const all = [...A.cho].flatMap((c) => [...A.jung].map((v) => compose(c, v)));
      drill = { items: all, fresh: all.filter(fresh) };
    } else if (L.jong) {
      const picked = [];
      for (const t of L.jong) {
        const real = realSyllables.filter((s) => fits(s) && decompose(s).jong === t);
        const gen = GEN_JUNG.flatMap((v) => GEN_CHO.map((c) => compose(c, v, t))).filter((s) => !real.includes(s));
        picked.push(...real, ...gen.slice(0, Math.max(0, 8 - real.length)));
      }
      const review = realSyllables.filter((s) => fits(s) && !fresh(s));
      drill = { items: unique([...picked, ...review]), fresh: unique(picked) };
    } else {
      const all = realSyllables.filter(fits);
      drill = { items: all, fresh: all.filter(fresh) };
    }
    for (const x of drill.items) drillSeen.add(x);

    const wordIdx = words.map((w, i) => [w, i]).filter(([w]) => [...w].every(fits));
    const lineIdx = L.id === "jamo"
      ? KO_JAMO_LINES.map((_, i) => lines.length + i)
      : lines.map((l, i) => [l, i]).filter(([l]) => textOk(l)).map(([, i]) => i);
    // 문장부호를 아직 안 익혔으면 빼고 쓸 수 있는 줄
    const strip = A.punct ? "" : KO_PUNCT;
    const plainIdx = !strip || L.id === "jamo" ? [] : lines.map((l, i) => [l, i]).filter(([l]) => !textOk(l) && textOk(plainOf(l, strip))).map(([, i]) => i);

    // 배열도에서 켤 자리
    const labels = [
      ...[...A.cho].filter((c) => choKey[c]).map((c) => `cho:${c}`),
      ...[...A.jung].filter((v) => jungKey[v]).map((v) => `jung:${v}`),
      ...(["ㅘ", "ㅙ", "ㅚ"].some((v) => A.jung.has(v)) ? ["rjung:ㅗ"] : []),
      ...(["ㅝ", "ㅞ", "ㅟ"].some((v) => A.jung.has(v)) ? ["rjung:ㅜ"] : []),
      ...(A.jung.has("ㅑ") ? ["rjung:ㅑ"] : []),
      ...[...A.jong].filter((t) => SINGLE_JONG.has(t)).map((t) => `jong:${t}`),
      ...(A.punct ? ["sym:.", "pass:,", "pass:?", "pass:!", ...[...KO_SYMS].map((c) => `sym:${c}`)] : []),
      ...(A.stop ? ["stop"] : []),
    ];
    // 새로 익히는 자리(칩과 배열도의 노란 키)
    const chips = L.chips
      ? L.chips.map(([t, role, keys]) => ({ t, role, keys }))
      : ["cho", "jung", "jong"].flatMap((part) => [...(L[part] ?? "")].map((j) => ({ t: j, role: ROLE[part], keys: keysOf(part, j) })));
    lessons.push({
      id: L.id, part: L.part, title: L.title, blurb: L.blurb, ex: L.ex ?? [], chips,
      freshKeys: unique(chips.flatMap((c) => [...c.keys].map(baseKey))),
      labels, modes: L.modes ?? ["drill", "words", "lines"],
      drill,
      words: wordIdx.map(([, i]) => i),
      wordsFresh: wordIdx.filter(([w]) => [...w].some(fresh)).map(([, i]) => i),
      lines: lineIdx,
      linesPlain: plainIdx,
      strip,
    });
  }

  // 엔진으로 확인: 낱말·줄·연습 글·예시를 권장 입력으로. 문장부호를 뺀 줄도.
  const texts = unique([...words, ...allLines, ...KO_LESSONS.flatMap((L) => [...(L.drill ?? []), ...(L.ex ?? [])]),
    ...lines.map((l) => plainOf(l, KO_PUNCT))]);
  const checked = verify("한국어", "ko", texts, toKeys, KO_LAYOUT);

  // 페이지가 쓰는 글자만 남긴다.
  const used = new Set([...drillSeen, ...texts].flatMap(tokens));
  const keep = Object.fromEntries(Object.entries(units).filter(([u]) => used.has(u)));
  return {
    lang: {
      id: "ko", name: "한국어", layout: "참신세벌식 v18", htmlLang: "ko", space: true,
      intro: "오른손은 초성, 왼손은 모음과 받침입니다. 받침 없는 글자로 초성·모음 자리를 먼저 익히고, 받침과 규칙을 차례로 더합니다.",
      board, units: keep, words, lines: allLines, lessons,
    },
    checked,
  };
}

// ---------------------------------------------------------------- 영어: Graphite

const EN_PUNCT = ",.?!'\"-;:/";
const EN_STRIP = ",.?!\";:";
const EN_LESSONS = [
  { id: "home1", part: "홈열", title: "홈열 ① t s h a", chars: "tsha", ex: ["hat", "that"],
    blurb: "Graphite 홈열의 검지와 중지입니다. 왼손 중지(D)가 t, 검지(F)가 s, 오른손 검지(J)가 h, 중지(K)가 a입니다." },
  { id: "home2", part: "홈열", title: "홈열 ② n r e i", chars: "nrei", ex: ["the", "rain"],
    blurb: "약지와 새끼손가락입니다. 왼손 A n, S r, 오른손 L e, ; i. 영어에서 가장 많이 쓰는 글자가 홈열에 모여 있습니다." },
  { id: "home3", part: "홈열", title: "홈열 ③ g y ,", chars: "gy,", ex: ["yes,", "night"], punct: ",",
    blurb: "검지를 안쪽으로 뻗어 G g, H y를 칩니다. 쉼표는 오른손 새끼손가락 ' 자리입니다." },
  { id: "top1", part: "윗열", title: "윗열 ① d w f o", chars: "dwfo", ex: ["word", "food"],
    blurb: "윗열의 검지와 중지입니다. E d, R w, U f, I o." },
  { id: "top2", part: "윗열", title: "윗열 ② b l u j", chars: "bluj", ex: ["blue", "just"],
    blurb: "윗열의 약지와 새끼손가락입니다. Q b, W l, O u, P j." },
  { id: "top3", part: "윗열", title: "윗열 ③ z '", chars: "z'", ex: ["it's", "size"],
    extra: ["it's", "that's", "isn't", "he's", "she's", "don't", "won't", "didn't", "let's", "there's", "we'll", "you'd", "they'd", "i'd", "doesn't", "wouldn't"],
    blurb: "검지를 안쪽으로 뻗어 T z, Y '(아포스트로피)를 칩니다." },
  { id: "bot1", part: "아랫열", title: "아랫열 ① m c p .", chars: "mcp.", ex: ["come.", "map"], punct: ".",
    blurb: "아랫열의 검지와 중지입니다. C m, V c, M p. 마침표는 , 자리입니다." },
  { id: "bot2", part: "아랫열", title: "아랫열 ② q x v k - /", chars: "qxvk-/", ex: ["quick", "well-known", "yes/no"],
    extra: ["well-known", "x-ray", "and/or", "yes/no", "one-way", "up-to-date", "self-made", "on/off", "t-shirt", "twenty-six"],
    blurb: "나머지 글자입니다. Z q, X x, B v, N k. 하이픈은 . 자리, /는 / 자리입니다." },
  { id: "caps", part: "Shift", title: "대문자", caps: true, ex: ["The", "London"],
    blurb: "대문자는 반대쪽 손의 Shift를 누른 채 칩니다. 오른손 글자는 왼쪽 Shift, 왼손 글자는 오른쪽 Shift입니다." },
  { id: "sym", part: "Shift", title: "기호 ? ! \" ; :", chars: "?!\";:", ex: ["why?", "\"hi\""], modes: ["drill", "lines"],
    extra: ["why?", "yes!", "\"hi\"", "\"no\"", "wait;", "note:", "ok?", "wow!", "really?", "stop!", "\"ok\"", "time:", "so;"],
    blurb: "물음표는 Shift+', 큰따옴표는 Shift+., 세미콜론은 [ 자리, 콜론은 Shift+[, 느낌표는 Shift+1입니다." },
  { id: "words", part: "실전", title: "낱말", modes: ["words", "lines"], blurb: "익힌 자리를 모두 써서 낱말을 칩니다." },
  { id: "lines", part: "실전", title: "짧은 문장", modes: ["lines"], blurb: "문장을 칩니다. 대문자와 문장부호도 함께 칩니다." },
];

function buildEn() {
  const layout = layoutJson("graphite");
  const keyOf = { " ": " " };
  for (const k of layout.keys) {
    if (k.base) keyOf[k.base] ??= k.key;
    if (k.shift) keyOf[k.shift] ??= shiftedOf(k.key);
  }
  const board = {};
  for (const k of layout.keys) {
    const letter = /[a-z]/.test(k.base);
    const spec = { main: { t: k.base, c: letter ? "latin" : "sym", id: `en:${k.base}` } };
    if (!letter) spec.shift = { t: k.shift, c: "sym", id: `en:${k.shift}` };
    board[k.key] = spec;
  }

  const words = unique(readTokens("en-words.txt"));
  for (const w of words) if (!/^[a-z]+$/.test(w)) fail(`en-words.txt: 소문자가 아닌 글자: ${w}`);
  const lines = readLines("en-lines.txt");
  for (const l of lines) if (![...l].every((c) => /[A-Za-z ]/.test(c) || EN_PUNCT.includes(c))) fail(`en-lines.txt: 쓸 수 없는 글자: ${l}`);

  const units = {};
  for (const c of unique([..."abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ", ...EN_PUNCT])) {
    if (!keyOf[c]) fail(`Graphite에 ${c}가 없다`);
    units[c] = [[keyOf[c], [c]]];
  }
  const toKeys = (t) => [...t].map((c) => (c === " " ? " " : units[c][0][0])).join("");

  // 낱말에서 글자 잇기 빈도(연습 낱말 만들기용). ^는 낱말 처음, $는 끝.
  const bigrams = {};
  for (const w of words) {
    const seq = ["^", ...w, "$"];
    for (let i = 0; i + 1 < seq.length; i++) (bigrams[seq[i]] ??= {})[seq[i + 1]] = (bigrams[seq[i]]?.[seq[i + 1]] ?? 0) + 1;
  }

  const A = new Set();
  let caps = false;
  const lessons = [];
  for (const L of EN_LESSONS) {
    for (const c of L.chars ?? "") A.add(c);
    caps ||= !!L.caps;
    const ok = (c) => c === " " || A.has(c) || (caps && /[A-Z]/.test(c) && A.has(c.toLowerCase()));
    const letters = [...A].filter((c) => /[a-z]/.test(c));
    const freshLetters = [...(L.chars ?? "")].filter((c) => /[a-z]/.test(c));
    const extra = (L.extra ?? []).filter((x) => [...x].every(ok));
    const wordIdx = words.map((w, i) => [w, i]).filter(([w]) => [...w].every(ok));
    const lineIdx = lines.map((l, i) => [l, i]).filter(([l]) => [...l].every(ok)).map(([, i]) => i);
    // 대문자·문장부호를 아직 안 익혔으면 소문자로 바꾸고 문장 부호를 빼고 쓸 수 있는 줄
    const strip = [...EN_STRIP].filter((c) => !A.has(c)).join("");
    const plainIdx = lines.map((l, i) => [l, i])
      .filter(([l]) => ![...l].every(ok) && [...plainOf(l, strip, !caps)].every(ok)).map(([, i]) => i);
    const chips = L.caps
      ? [{ t: "A–Z", role: "대문자", keys: "" }]
      : [...(L.chars ?? "")].map((c) => ({ t: c, role: /[a-z]/.test(c) ? "" : "기호", keys: units[c][0][0] }));
    lessons.push({
      id: L.id, part: L.part, title: L.title, blurb: L.blurb, ex: L.ex ?? [], chips,
      freshKeys: L.caps ? ["lshift", "rshift"] : unique(chips.flatMap((c) => [...c.keys].map(baseKey))),
      labels: [...A].map((c) => `en:${c}`),
      modes: L.modes ?? ["drill", "words", "lines"],
      drill: {
        gen: { units: letters, fresh: freshLetters, min: 3, max: 6, caps: !!L.caps, punct: L.punct ? [...L.punct] : [] },
        ...(extra.length ? { items: extra, fresh: extra, mix: 0.45 } : {}),
      },
      capWords: !!L.caps,
      words: wordIdx.map(([, i]) => i),
      wordsFresh: wordIdx.filter(([w]) => [...w].some((c) => freshLetters.includes(c))).map(([, i]) => i),
      lines: lineIdx,
      linesPlain: plainIdx,
      strip,
      lower: !caps,
    });
  }
  const texts = unique([...words, ...lines, ...lines.map((l) => plainOf(l, EN_STRIP, true)), ...EN_LESSONS.flatMap((L) => [...(L.extra ?? []), ...(L.ex ?? [])]),
    ...words.map((w) => w[0].toUpperCase() + w.slice(1))]);
  const checked = verify("영어", "en", texts, toKeys);
  return {
    lang: {
      id: "en", name: "English", layout: "Graphite", htmlLang: "en", space: true,
      intro: "Graphite는 가장 많이 쓰는 글자를 홈열에 모았습니다. 손가락별로 두세 글자씩 더해 가며 익힙니다. 키캡의 쿼티 글자와 다르니 배열도를 보며 칩니다.",
      board, units, words, lines, bigrams, lessons,
    },
    checked,
  };
}

// ---------------------------------------------------------------- 일본어: 新月

const JA_SYMBOLS = "、。「」・？！ー";
const JA_STRIP = "、。「」・？！";
const JA_SHORT = ["きゃ", "きゅ", "きょ", "しゃ", "しゅ", "しょ", "ちゃ", "ちゅ", "ちょ", "にゃ", "にゅ", "にょ", "ひゃ", "ひゅ", "ひょ",
  "みゃ", "みゅ", "みょ", "りゃ", "りゅ", "りょ", "ぎゃ", "ぎゅ", "ぎょ", "じゃ", "じゅ", "じょ", "びゃ", "びゅ", "びょ", "ぴゃ", "ぴゅ", "ぴょ",
  "ふぁ", "ふぃ", "ふぇ", "ふぉ", "てぃ", "でぃ", "うぃ", "うぇ", "うぉ", "しぇ", "じぇ", "ちぇ", "こー", "すー", "らー", "けーき", "きょう", "しゅう", "ちょう"];
const JA_SYM_ITEMS = ["「はい」", "「いいえ」", "え？", "なに？", "ほんと！", "すごい！", "あか・あお", "「おはよう」", "いく？", "よし！", "やま・かわ"];

const JA_LESSONS = [
  { id: "home", part: "한 번에 치는 가나", title: "홈열 は か と た く う き", kana: "はかとたくうき", ex: ["うた", "とき"],
    blurb: "新月은 로마자를 거치지 않고 가나를 바로 칩니다. 홈열에서 한 번에 치는 가나부터 익힙니다. D와 K는 앞치기 키(★ ☆), L은 ゛ 키라서 뒤에서 익힙니다. 변환(Space)은 연습하지 않고 가나만 칩니다." },
  { id: "top-r", part: "한 번에 치는 가나", title: "오른손 윗열 つ ん い の り", kana: "つんいのり", ex: ["いと", "のり"],
    blurb: "오른손 윗열입니다. Y つ, U ん, I い, O の, P り. い·の·ん은 아주 자주 나옵니다." },
  { id: "top-l", part: "한 번에 치는 가나", title: "왼손 윗열 そ こ し て ょ", kana: "そこしてょ", ex: ["こし", "しょ"],
    blurb: "왼손 윗열입니다. Q そ, W こ, E し, R て, T ょ(작은 よ). 작은 ょ는 앞 가나에 붙여 しょ처럼 씁니다." },
  { id: "bot-l", part: "한 번에 치는 가나", title: "왼손 아랫열 す け に な さ", kana: "すけになさ", ex: ["なす", "さけ"],
    blurb: "왼손 아랫열입니다. Z す, X け, C に, V な, B さ." },
  { id: "bot-r", part: "한 번에 치는 가나", title: "오른손 아랫열 っ る 、 。", kana: "っる、。", ex: ["きって", "くる。"], punct: "、。",
    blurb: "오른손 아랫열입니다. N っ(작은 つ), M る, 쉼표 、는 , 자리, 마침표 。는 . 자리입니다." },
  { id: "daku", part: "゛ 뒤치기", title: "탁음·반탁음 ゛", daku: true, ex: ["が", "ど", "ぱ"],
    blurb: "가나 뒤에 ゛(L)를 치면 탁음(が), 두 번 치면 반탁음(ぱ)입니다. 오른손 약지로 칩니다." },
  { id: "star-home", part: "☆ 다음 왼손", title: "☆ 홈열 あ を ら れ よ", kana: "あをられよ", ex: ["あ", "を", "ら"],
    blurb: "☆(K)를 치고 왼손 키를 치면 주황 글자가 나옵니다. 오른손 중지로 ☆, 이어서 왼손입니다. 양손이 번갈아 움직입니다." },
  { id: "star-top", part: "☆ 다음 왼손", title: "☆ 윗열 ゆ ひ ほ ふ め", kana: "ゆひほふめ", ex: ["ゆめ", "ひと", "ぶ"],
    blurb: "☆ 다음 왼손 윗열입니다. ひ ふ ほ 뒤에 ゛를 치면 び ぶ ぼ, 두 번이면 ぴ ぷ ぽ입니다." },
  { id: "star-bot", part: "☆ 다음 왼손", title: "☆ 아랫열 ね へ せ ゅ ゃ", kana: "ねへせゅゃ", ex: ["ねこ", "きゃ"],
    blurb: "☆ 다음 왼손 아랫열입니다. ゅ ゃ는 작은 글자라 앞 가나에 붙여 きゃ처럼 씁니다." },
  { id: "black-home", part: "★ 다음 오른손", title: "★ 홈열 ま お も わ ち", kana: "まおもわち", ex: ["まち", "おもい"],
    blurb: "★(D)를 치고 오른손 키를 치면 남색 글자가 나옵니다. 왼손 중지로 ★, 이어서 오른손입니다." },
  { id: "black-rest", part: "★ 다음 오른손", title: "★ 윗열·아랫열 ぬ え み や む ろ", kana: "ぬえみやむろ", ex: ["いぬ", "みみ", "やま"],
    blurb: "★ 다음 오른손 윗열(Y ぬ, U え, I み, O や)과 아랫열(N む, M ろ)입니다. 이제 가나를 모두 익혔습니다." },
  { id: "small", part: "작은 글자·줄임", title: "요음·작은 글자·ー", kana: "ー", drill: JA_SHORT, ex: ["きょう", "ふぁ", "こー"],
    blurb: "작은 ゃ ゅ ょ를 앞 가나에 붙이면 요음입니다. あ い う え お 뒤에 ゛를 치면 작은 ぁ ぃ ぅ ぇ ぉ가 됩니다(う는 두 번). 장음 ー는 - 자리입니다." },
  { id: "short", part: "작은 글자·줄임", title: "3타 줄임 ☆ ゛", sc: true, ex: ["じゃ", "ぎょ", "ぴょ"],
    blurb: "자주 쓰는 요음은 ☆(K), ゛(L), 키 하나를 차례로 쳐서 한 번에 입력합니다. 하나씩 쳐도 됩니다. 이 단계부터는 줄임을 먼저 보여 줍니다." },
  { id: "sym", part: "작은 글자·줄임", title: "기호 「 」 ・ ？ ！", kana: "「」・？！", drill: JA_SYM_ITEMS, ex: ["「はい」", "え？"], modes: ["drill", "lines"],
    blurb: "「 」는 [ ] 자리, ・는 / 자리, ？는 Shift+/, ！는 Shift+1입니다." },
  { id: "words", part: "실전", title: "낱말", modes: ["words", "lines"], blurb: "익힌 자리를 모두 써서 낱말을 칩니다." },
  { id: "lines", part: "실전", title: "짧은 문장", modes: ["lines"], blurb: "문장을 칩니다. 한자가 섞인 글을 보고 읽는 대로 가나를 칩니다." },
];

function buildJa() {
  const sg = layoutJson("shingetsu");
  const byBase = {}, byStar = {}, byBlack = {};
  for (const k of sg.keys) {
    if (k.base) (byBase[k.base] ??= []).push(k.key);
    if (k.star) (byStar[k.star] ??= []).push(k.key);
    if (k.black) (byBlack[k.black] ??= []).push(k.key);
  }
  const from = Object.fromEntries(sg.postfix.map((p) => [p.to, p.from]));
  const sameHand = (a, b) => LEFT.has(a) === LEFT.has(b);

  // 가나 하나를 치는 방법들: 한 번, ☆+왼손, ★+오른손, 앞 가나 + ゛. labels는 타마다 쓰는 자리(단계에서 켜는 이름).
  function waysOf(x) {
    const out = [];
    for (const key of byBase[x] ?? []) out.push({ keys: key, labels: [`kana:${x}`] });
    for (const key of byStar[x] ?? []) out.push({ keys: sg.star_key + key, labels: ["pre:☆", `kana:${x}`] });
    for (const key of byBlack[x] ?? []) out.push({ keys: sg.black_key + key, labels: ["pre:★", `kana:${x}`] });
    if (from[x]) for (const w of waysOf(from[x])) out.push({ keys: w.keys + sg.daku_key, labels: [...w.labels, "daku"] });
    if (x === "？") out.push({ keys: "?", labels: ["kana:？"] });
    if (x === "！") out.push({ keys: "!", labels: ["kana:！"] });
    // 짧은 것부터, 같으면 앞치기와 가나를 양손으로 번갈아 치는 것부터(☆D·★K가 ★★·☆☆보다 앞)
    const cost = (w) => w.keys.length * 2 + (w.keys.length === 2 && sameHand(w.keys[0], w.keys[1]) ? 1 : 0);
    return out.sort((a, b) => cost(a) - cost(b));
  }
  const pairs = new Map(sg.star_daku.filter((p) => p.out.length === 2).map((p) => [p.out, sg.star_key + sg.daku_key + p.key]));

  const words = unique(readTokens("ja-words.txt")).map((t) => (t.includes("|") ? t.split("|") : [t, t]));
  const lines = readLines("ja-lines.txt").map((l) => {
    const [shown, read] = l.split("|");
    if (!read) fail(`ja-lines.txt: 읽기가 없다: ${l}`);
    return [shown, read];
  });
  const kanaOk = (c) => /[ぁ-ゔ]/.test(c) || JA_SYMBOLS.includes(c);
  for (const [s, r] of [...words, ...lines]) if (![...r].every(kanaOk)) fail(`${s}: 읽기에 쓸 수 없는 글자: ${r}`);

  // 글 → 글자 단위. 줄임을 익힌 뒤에는 줄임이 있는 두 글자(じゃ 등)를 한 덩어리로.
  const tokenize = (text, sc) => {
    const out = [];
    for (let i = 0; i < text.length; ) {
      const two = text.slice(i, i + 2);
      if (sc && pairs.has(two)) {
        out.push(two);
        i += 2;
      } else {
        out.push(text[i]);
        i++;
      }
    }
    return out;
  };
  const allTexts = [...words.map((w) => w[1]), ...lines.map((l) => l[1]), ...JA_SHORT, ...JA_SYM_ITEMS, ...JA_LESSONS.flatMap((L) => L.ex ?? [])];
  const singles = unique([...allTexts.join("")]);
  const units = {}, labelsOf = {};
  const add = (u, ways) => {
    if (!ways.length) fail(`${u}: 新月으로 칠 방법이 없다`);
    const scr = screensOf("ja", ways.map((w) => w.keys));
    const bad = ways.map((w, i) => [w, scr[i].at(-1)]).filter(([, s]) => s !== u);
    if (bad.length) fail(`${u}: ${bad.map(([w, s]) => `${w.keys} → ${JSON.stringify(s)}`).join(", ")}`);
    units[u] = ways.map((w, i) => [w.keys, scr[i]]);
    labelsOf[u] = ways[0].labels;
  };
  for (const c of singles) add(c, waysOf(c).slice(0, 3));
  for (const [out, keys] of pairs) {
    const [a, b] = [...out];
    const comp = waysOf(a).slice(0, 2).flatMap((x) => waysOf(b).slice(0, 1).map((y) => ({ keys: x.keys + y.keys, labels: [...x.labels, ...y.labels] })));
    add(out, [{ keys, labels: ["pre:☆", "daku", "sc"] }, ...comp]);
  }
  const toKeys = (sc) => (t) => tokenize(t, sc).map((u) => units[u][0][0]).join("");

  const board = {};
  for (const k of sg.keys) {
    const spec = {};
    if (k.key === sg.star_key) Object.assign(spec, { main: { t: "☆", c: "star", id: "pre:☆" }, corner: { t: k.black, c: "black", id: `kana:${k.black}` } });
    else if (k.key === sg.black_key) Object.assign(spec, { main: { t: "★", c: "black", id: "pre:★" }, corner: { t: k.star, c: "star", id: `kana:${k.star}` } });
    else if (k.key === sg.daku_key) Object.assign(spec, { main: { t: "゛", c: "daku", id: "daku" }, corner: { t: k.black, c: "black", id: `kana:${k.black}` } });
    else {
      spec.main = k.base ? { t: k.base, c: "kana", id: `kana:${k.base}` } : { t: k.key, c: "plain", id: null };
      if (k.star) spec.corner = { t: k.star, c: "star", id: `kana:${k.star}` };
      if (k.black) spec.corner = { t: k.black, c: "black", id: `kana:${k.black}` };
    }
    if (k.key === "/") spec.shift = { t: "？", c: "sym", id: "kana:？" };
    if (k.key === "1") spec.shift = { t: "！", c: "sym", id: "kana:！" };
    board[k.key] = spec;
  }

  // 가나 잇기 빈도(연습 낱말 만들기용)
  const drillUnits = singles.filter((c) => !JA_SYMBOLS.includes(c));
  const bigrams = {};
  for (const [, r] of words) {
    const seq = ["^", ...tokenize(r, false), "$"];
    for (let i = 0; i + 1 < seq.length; i++) (bigrams[seq[i]] ??= {})[seq[i + 1]] = (bigrams[seq[i]]?.[seq[i + 1]] ?? 0) + 1;
  }

  const allowed = new Set();
  let sc = false;
  const lessons = [];
  let prevUnits = new Set();
  for (const L of JA_LESSONS) {
    const freshLabels = [...(L.kana ?? "")].map((k) => `kana:${k}`);
    if (L.daku) freshLabels.push("daku");
    if (L.id === "star-home") freshLabels.push("pre:☆");
    if (L.id === "black-home") freshLabels.push("pre:★");
    if (L.sc) freshLabels.push("sc");
    for (const l of freshLabels) allowed.add(l);
    sc ||= !!L.sc;
    const unitOk = (u) => labelsOf[u]?.every((l) => allowed.has(l));
    const textOk = (t) => tokenize(t, sc).every(unitOk);
    const nowUnits = new Set(drillUnits.filter(unitOk));
    const freshUnits = [...nowUnits].filter((u) => !prevUnits.has(u));
    prevUnits = nowUnits;
    const isFreshText = (t) => tokenize(t, sc).some((u) => labelsOf[u]?.some((l) => freshLabels.includes(l)));
    const wordIdx = words.map((w, i) => [w, i]).filter(([w]) => textOk(w[1]));
    const lineIdx = lines.map((l, i) => [l, i]).filter(([l]) => textOk(l[1])).map(([, i]) => i);
    // 구두점·기호를 아직 안 익혔으면 빼고 쓸 수 있는 줄
    const strip = [...JA_STRIP].filter((c) => !allowed.has(`kana:${c}`)).join("");
    const plainIdx = lines.map((l, i) => [l, i]).filter(([l]) => !textOk(l[1]) && textOk(plainOf(l[1], strip))).map(([, i]) => i);

    // 새 자리 칩: 새 가나(탁음 단계는 대표 몇 개, 줄임 단계는 줄임 전부)
    let chips;
    if (L.daku) chips = ["が", "ざ", "だ", "ば", "ぱ", "ぐ"].map((t) => ({ t, role: t === "ぱ" ? "반탁음" : "탁음", keys: units[t][0][0] }));
    else if (L.sc) chips = [...pairs.keys()].map((t) => ({ t, role: "", keys: units[t][0][0] }));
    else chips = [...(L.kana ?? "")].map((t) => ({ t, role: "", keys: (units[t] ?? [[waysOf(t)[0].keys]])[0][0] }));
    const freshKeys = unique(chips.flatMap((c) => {
      if (c.t.length === 2) return [sg.star_key, sg.daku_key, c.keys.at(-1)];
      const w = waysOf(c.t)[0];
      return [...w.keys].filter((_, i) => freshLabels.includes(w.labels[i])).map(baseKey);
    }));

    let drill;
    if (L.drill) {
      for (const x of L.drill) if (!textOk(x)) fail(`${L.title}: 연습 글 ${x}에 아직 안 익힌 자리가 있다`);
      drill = { items: L.drill, fresh: L.drill };
    } else if (L.sc) drill = { items: [...pairs.keys()], fresh: [...pairs.keys()] };
    else drill = { gen: { units: [...nowUnits], fresh: freshUnits, min: 2, max: 4, punct: L.punct ? [...L.punct] : [] } };
    lessons.push({
      id: L.id, part: L.part, title: L.title, blurb: L.blurb, ex: L.ex ?? [], chips, freshKeys,
      labels: [...allowed], modes: L.modes ?? ["drill", "words", "lines"], sc, drill,
      words: wordIdx.map(([, i]) => i),
      wordsFresh: wordIdx.filter(([w]) => isFreshText(w[1])).map(([, i]) => i),
      lines: lineIdx,
      linesPlain: plainIdx,
      strip,
    });
  }

  // 엔진으로 확인: 읽기·연습 글·예시를 줄임 없이, 또 줄임으로. 구두점을 뺀 줄도.
  const texts = unique([...allTexts, ...lines.map((l) => plainOf(l[1], JA_STRIP))]);
  const checked = verify("일본어", "ja", texts, toKeys(false)) + verify("일본어(줄임)", "ja", texts, toKeys(true));
  return {
    lang: {
      id: "ja", name: "日本語", layout: "新月", htmlLang: "ja", space: false,
      intro: "新月은 가나를 바로 칩니다. 한 번에 치는 가나 → ゛ 뒤치기 → ☆·★ 다음에 치는 가나 → 작은 글자와 줄임 순서로 익힙니다. 가나 사이에 공백은 치지 않습니다.",
      board, units, words, lines, bigrams, pairs: [...pairs.keys()], lessons,
    },
    checked,
  };
}

// ---------------------------------------------------------------- 쓰기

const ko = buildKo(), en = buildEn(), ja = buildJa();
const DATA = { langs: [ko.lang, en.lang, ja.lang] };
const template = fs.readFileSync(path.join(repo, "practice/template.html"), "utf8");
if (!template.includes("/*DATA*/null")) fail("practice/template.html에 /*DATA*/null 자리가 없다");
const json = JSON.stringify(DATA).replace(/</g, "\\u003c");
const NOTE = "<!-- 생성 파일: tools/practice/build.mjs가 practice/template.html과 엔진 데이터로 만든다. 직접 고치지 않는다. -->";
const html = template.replace("/*DATA*/null", () => json).replace("<!--generated-->", NOTE);
const outPath = path.join(repo, "practice/index.html");
const summary = DATA.langs.map((l) => `${l.name} 단계 ${l.lessons.length}·글자 ${Object.keys(l.units).length}·낱말 ${l.words.length}·줄 ${l.lines.length}`).join(" / ");
const checked = ko.checked + en.checked + ja.checked;
if (CHECK) {
  const current = fs.existsSync(outPath) ? fs.readFileSync(outPath, "utf8") : "";
  if (current !== html) fail("practice/index.html이 최신이 아니다: node tools/practice/build.mjs");
  console.log(`엔진으로 ${checked}개 확인, practice/index.html 최신 (${summary})`);
} else {
  fs.writeFileSync(outPath, html);
  console.log(`practice/index.html ${(html.length / 1024).toFixed(0)}KB, 엔진으로 ${checked}개 확인\n${summary}`);
  for (const l of DATA.langs) {
    console.log(`  ${l.name}: ${l.lessons.map((L) => `${L.id}(자리 ${L.drill.items?.length ?? "생성"}·낱말 ${L.words.length}/${L.wordsFresh.length}·줄 ${L.lines.length}+${L.linesPlain.length})`).join(" ")}`);
  }
}

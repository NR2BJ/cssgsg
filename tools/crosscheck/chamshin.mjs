// 참신세벌식 배열 데이터 교차 검증.
// 우리 layouts/ko/*.toml(엔진이 읽은 결과)을 독립 출처 둘과 키마다 대조한다.
//   - 타닥 문서의 배열도 데이터(KangWH/tadak-docs): 기본형 + D
//   - 오이의 배열표(Sinseiki/ohi, GPL이라 받기만 한다): 기본형
// 사용: node tools/crosscheck/chamshin.mjs   (의존성 없음, Node 18+)
import { execFileSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const TADAK = "https://raw.githubusercontent.com/KangWH/tadak-docs/8d57bf2e2a04ef1f60892aae46449d3a2c9e9850/app/docs/layouts/threeSetLayouts.ts";
const OHI = "https://raw.githubusercontent.com/Sinseiki/ohi/123e0fad8e9a5a42e816d9bd3a9d039e5851d7c2/additional_layouts.js";

// 출처 사이의 알려진 차이: "출처 키 shift" → 이유
const KNOWN = {
  "오이 o true": "Shift+O 기호: 오이 —, 타닥 ―. 타닥을 따른다(설정으로 바꿀 수 있게 할 예정)",
  "오이 h true": "Shift+H 기호: 오이 ⨉, 타닥 ×. 타닥을 따른다",
  "오이 m true": "Shift+M 아래아: 출처는 모음(옛한글 조합), 우리는 아직 기호 ㆍ로만 낸다",
  "타닥 m true": "Shift+M 아래아: 출처는 모음(옛한글 조합), 우리는 아직 기호 ㆍ로만 낸다",
  "타닥 ` false": "타닥은 ` 자리에 ₩를 표기한다(한국 키보드 관례). 원작자: 쿼티와 다른 기호 자리는 / ; 두 곳뿐",
};

const SHIFTED = { "!": "1", "@": "2", "#": "3", $: "4", "%": "5", "^": "6", "&": "7", "*": "8", "(": "9", ")": "0",
  _: "-", "+": "=", "{": "[", "}": "]", "|": "\\", ":": ";", '"': "'", "~": "`", "<": ",", ">": ".", "?": "/" };
const CHO = "ㄱㄲㄴㄷㄸㄹㅁㅂㅃㅅㅆㅇㅈㅉㅊㅋㅌㅍㅎ";
const JUNG = "ㅏㅐㅑㅒㅓㅔㅕㅖㅗㅘㅙㅚㅛㅜㅝㅞㅟㅠㅡㅢㅣ";
const JONG = "ㄱㄲㄳㄴㄵㄶㄷㄹㄺㄻㄼㄽㄾㄿㅀㅁㅂㅄㅅㅆㅇㅈㅊㅋㅌㅍㅎ";

const fmt = (r) => Object.entries(r).filter(([, v]) => v != null).map(([k, v]) => `${k}=${v}`).sort().join(" ") || "(없음)";

function ours(id) {
  const env = { ...process.env, PATH: `${process.env.HOME}/.cargo/bin:${process.env.PATH}` };
  execFileSync("cargo", ["build", "-q", "-p", "cssgsg-cli"], { cwd: repo, env, stdio: "inherit" });
  const out = execFileSync(path.join(repo, "build/cargo/debug/cssgsg-cli"), ["layout-json", id], { encoding: "utf8" });
  const map = new Map();
  for (const k of JSON.parse(out).keys) {
    const r = k.passthrough != null
      ? { sym: k.passthrough }
      : { cho: k.cho, jung: k.jung, jong: k.jong, sym: k.sym, action: k.action };
    map.set(`${k.key} ${k.shift}`, r);
  }
  return map;
}

function unquote(s) {
  return s.replace(/\\(.)/g, "$1");
}

function tadak(src, name) {
  const start = src.indexOf(`export const ${name}:`);
  const end = src.indexOf("export const", start + 10);
  const block = src.slice(start, end < 0 ? undefined : end);
  const map = new Map();
  const keyRe = /\{keyCode: '((?:\\.|[^'])*)', bottomLabels: \[(.*?)\], topLabels: \[(.*?)\]\}/g;
  const labelRe = /\{label: '((?:\\.|[^'])*)', labelType: '(\w+)'\}/g;
  const roles = (labels) => {
    const r = {};
    for (const [, label, type] of labels.matchAll(labelRe)) {
      const v = unquote(label);
      if (type === "leading") r.cho = v;
      else if (type === "vowel") r.jung = v;
      else if (type === "trailing") r.jong = v;
      else if (v === "❖") r.action = "stop";
      else r.sym = v;
    }
    return r;
  };
  for (const [, key, bottom, top] of block.matchAll(keyRe)) {
    const k = unquote(key);
    map.set(`${k} false`, roles(bottom));
    map.set(`${k} true`, roles(top));
  }
  return map;
}

function ohi(src) {
  const start = src.indexOf("K3_Sin3_Cham_layout = [");
  const end = src.indexOf("];", start);
  const pairs = [...src.slice(start, end).matchAll(/\[0x([0-9A-Fa-f]+),0x([0-9A-Fa-f]+)\]/g)].map((m) => [parseInt(m[1], 16), parseInt(m[2], 16)]);
  if (pairs.length !== 94) throw new Error(`오이 배열표 칸 수 ${pairs.length} (94여야 함)`);
  const map = new Map();
  pairs.forEach(([a, b], i) => {
    const ch = String.fromCharCode(0x21 + i);
    const shift = ch !== ch.toLowerCase() || ch in SHIFTED;
    const base = SHIFTED[ch] ?? ch.toLowerCase();
    const r = {};
    for (const c of [a, b]) {
      if (c === 0) continue;
      if (c >= 0x1100 && c <= 0x1112) r.cho = CHO[c - 0x1100];
      else if (c >= 0x1161 && c <= 0x1175) r.jung = JUNG[c - 0x1161];
      else if (c === 0x119e) r.jung = "ㆍ";
      else if (c >= 0x11a8 && c <= 0x11c2) r.jong = JONG[c - 0x11a8];
      else if (c === 0x1b) r.action = "stop";
      else r.sym = String.fromCodePoint(c);
    }
    map.set(`${base} ${shift}`, r);
  });
  return map;
}

function compare(label, ref, mine) {
  let same = 0;
  const known = [], unexpected = [];
  for (const [k, r] of ref) {
    const m = mine.get(k);
    if (!m) { unexpected.push(`${k}: 우리 데이터에 없는 키`); continue; }
    if (fmt(r) === fmt(m)) { same++; continue; }
    const line = `${k.replace(" true", " (Shift)").replace(" false", "")}: ${label} [${fmt(r)}] / 우리 [${fmt(m)}]`;
    (KNOWN[`${label} ${k}`] ? known : unexpected).push(line + (KNOWN[`${label} ${k}`] ? `  ← ${KNOWN[`${label} ${k}`]}` : ""));
  }
  return { same, total: ref.size, known, unexpected };
}

const [tadakSrc, ohiSrc] = await Promise.all([TADAK, OHI].map((u) => fetch(u).then((r) => { if (!r.ok) throw new Error(`${u}: ${r.status}`); return r.text(); })));
const checks = [
  ["chamshin-v18", "타닥", tadak(tadakSrc, "chamShinThreeSetLayout")],
  ["chamshin-v18", "오이", ohi(ohiSrc)],
  ["chamshin-d-v19", "타닥", tadak(tadakSrc, "chamShinThreeSetDLayout")],
];
let bad = 0;
for (const [id, label, ref] of checks) {
  const r = compare(label, ref, ours(id));
  console.log(`\n${id} vs ${label}: ${r.same}/${r.total} 키 일치, 알려진 차이 ${r.known.length}, 예상 밖 ${r.unexpected.length}`);
  for (const l of r.known) console.log(`  (알려짐) ${l}`);
  for (const l of r.unexpected) console.log(`  !! ${l}`);
  bad += r.unexpected.length;
}
process.exit(bad ? 1 : 0);

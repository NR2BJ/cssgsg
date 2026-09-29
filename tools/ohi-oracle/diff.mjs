// 오이(최신 조합표로 고친 사본)와 cssgsg-cli의 참신세벌식 결과를 무작위 키열로 비교한다.
// 사용: node diff.mjs [--count N] [--seed S] [--maxlen L] [--show K]
import { execFileSync, spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { JSDOM, VirtualConsole } from "jsdom";

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, "../..");
const work = path.join(repo, "build/ohi-oracle");
const OHI_REPO = "https://github.com/Sinseiki/ohi.git";
const OHI_COMMIT = "123e0fad8e9a5a42e816d9bd3a9d039e5851d7c2"; // 2026-09-05

const args = Object.fromEntries(
  process.argv.slice(2).reduce((acc, a, i, all) => (a.startsWith("--") ? [...acc, [a.slice(2), all[i + 1]]] : acc), []),
);
const COUNT = Number(args.count ?? 5000);
const SEED = Number(args.seed ?? 1);
const MAXLEN = Number(args.maxlen ?? 10);
const SHOW = Number(args.show ?? 15);

// ---------------------------------------------------------------- 오이 준비

function prepareOhi() {
  const dir = path.join(work, "ohi");
  if (!fs.existsSync(path.join(dir, ".git"))) {
    fs.mkdirSync(work, { recursive: true });
    execFileSync("git", ["clone", "-q", OHI_REPO, dir], { stdio: "inherit" });
  }
  execFileSync("git", ["-C", dir, "checkout", "-q", OHI_COMMIT], { stdio: "inherit" });
  const patched = path.join(work, "ohi-latest");
  fs.rmSync(patched, { recursive: true, force: true });
  fs.cpSync(dir, patched, { recursive: true, filter: (p) => !p.includes(`${path.sep}.git`) });
  const file = path.join(patched, "additional_layouts.js");
  let src = fs.readFileSync(file, "utf8");
  const edits = [
    ["[0x11001109,0x110D], /* choseong gieun + sieus = ssang_jieuj */", "[0x110C110B,0x110D], /* ㅈ+ㅇ=ㅉ (최신) */"],
    ["[0x11A811B7,0x11A9], /* jongseong gieug + mieum = ssang_gieug */", "[0x11A811B7,0x11BF], /* ㄱ+ㅁ=ㅋ (최신) */"],
    ["[0x11A811B8,0x11BF], /* jongseong gieug + bieub = kieuk */", "/* ㄱ+ㅂ=ㅋ 삭제 (최신) */"],
    ["[0x11BC11B7,0x11B6], /* jongseong ieung + mieum = lieul-hieuh */", "[0x11BC11B7,0x11B5], /* ㅇ+ㅁ=ㄿ (최신) */"],
  ];
  // 같은 줄이 다른 배열의 표에도 있으므로 참신세벌식 표 구간 안에서만 고친다.
  const start = src.indexOf("K3_Sin3_Cham_combination_table =");
  const addStart = src.indexOf("K3_Sin3_Cham_additional_combination_table", start);
  const end = src.indexOf("];", addStart);
  if (start < 0 || addStart < 0 || end < 0) throw new Error("오이에서 참신세벌식 조합표를 찾을 수 없음");
  let region = src.slice(start, end);
  for (const [from, to] of edits) {
    const n = region.split(from).length - 1;
    if (n !== 1) throw new Error(`참신세벌식 조합표에서 ${n}번 나옴(1번이어야 함): ${from}`);
    region = region.replace(from, to);
  }
  src = src.slice(0, start) + region + src.slice(end);
  fs.writeFileSync(file, src);
  return patched;
}

async function loadOhi(dir) {
  const html = fs.readFileSync(path.join(dir, "index.html"), "utf8").replace(/Ko_type = '[^']*'/, "Ko_type = 'Sin3-Cham'");
  const dom = new JSDOM(html, {
    runScripts: "dangerously",
    resources: "usable",
    url: "file://" + path.join(dir, "index.html"),
    virtualConsole: new VirtualConsole(),
    pretendToBeVisual: true,
  });
  await new Promise((r) => dom.window.addEventListener("load", r));
  return dom;
}

// 브라우저 keydown의 keyCode(VK)
const VK = { ".": 190, ",": 188, "/": 191, ";": 186, "'": 222, "-": 189, "=": 187, "[": 219, "]": 221, "\\": 220, "`": 192 };
const SHIFTED = { ">": ".", "<": ",", "?": "/", ":": ";", '"': "'", _: "-", "+": "=", "{": "[", "}": "]", "|": "\\", "~": "`",
  "!": "1", "@": "2", "#": "3", $: "4", "%": "5", "^": "6", "&": "7", "*": "8", "(": "9", ")": "0" };

function ohiType(dom, seq) {
  const w = dom.window;
  const ta = w.document.getElementById("inputText");
  w.eval("ohiQ=[0,0,0,0,0,0,0,0,0]; prev_ohiQ=[]; backup_ohiQ=[]; initialize_NFD_stack(); esc_ext_state(); prev_cursor_position=-1;");
  ta.value = "";
  ta.focus();
  ta.selectionStart = ta.selectionEnd = 0;
  for (const ch of seq) {
    const base = SHIFTED[ch] ?? ch.toLowerCase();
    const shift = ch !== ch.toLowerCase() || ch in SHIFTED;
    const vk = VK[base] ?? base.toUpperCase().charCodeAt(0);
    for (const type of ["keydown", "keypress", "keyup"]) {
      const ev = new w.Event(type, { bubbles: true, cancelable: true });
      const code = type === "keypress" ? ch.charCodeAt(0) : vk;
      for (const [k, v] of Object.entries({ which: code, keyCode: code, charCode: type === "keypress" ? code : 0, shiftKey: shift, ctrlKey: false, altKey: false, metaKey: false })) {
        Object.defineProperty(ev, k, { value: v });
      }
      ta.dispatchEvent(ev);
    }
  }
  return ta.value;
}

// ---------------------------------------------------------------- 키열 생성

function rng(seed) {
  let s = seed >>> 0 || 1;
  return () => ((s = (s * 1664525 + 1013904223) >>> 0) / 2 ** 32);
}

// 한글 키 위주 + 쿼티 그대로 나가는 키 몇 개 + Shift 기호/❖/받침 ㅋ. Shift+M(아래아)은 옛한글이라 뺀다.
const LEFT = "qwertasdfgzxcv";
const RIGHT = "yuiophjkl;bnm./";
const OTHER = ",1QPLBNJK?";
function makeSeqs(n) {
  const r = rng(SEED);
  const pick = (s) => s[Math.floor(r() * s.length)];
  const out = new Set();
  while (out.size < n) {
    const len = 1 + Math.floor(r() * MAXLEN);
    let s = "";
    for (let i = 0; i < len; i++) {
      const x = r();
      s += x < 0.5 ? pick(LEFT) : x < 0.93 ? pick(RIGHT) : pick(OTHER);
    }
    out.add(s);
  }
  return [...out];
}

// ---------------------------------------------------------------- 알려진 차이

// 오이와 일부러 다른 동작. [이름, 판별 함수(keys, ohi, ours)]
//
// 1) 겹받침 뒤 왼손 키: 오이는 겹받침(두 받침을 합친 것) 뒤에 오는 왼손 키를 소리 없이 버린다
//    (예: ytesqraq → 오이 "쳕", 뒤 네 타가 사라짐). 우리는 홑받침 뒤와 같은 규칙으로
//    새 받침 낱자·모음을 만든다(→ "쳕ㅆㅜㅛㅠ"). 키 입력을 버리지 않는 쪽을 택했다.
//    판별: 우리 결과에서 "받침 있는 음절 바로 뒤에 끼어든 낱자(호환 자모)"를 빼면 오이 결과가 된다.
const isJamo = (c) => c >= "\u3131" && c <= "\u318E";
const hasJong = (c) => c >= "\uAC00" && c <= "\uD7A3" && (c.charCodeAt(0) - 0xac00) % 28 !== 0;
function onlyExtraJamoAfterJong(_keys, ohi, ours) {
  // 받침 있는 음절 바로 뒤의 낱자 연속 구간을 빼 보는 모든 경우를 시도한다(삽입 3곳까지).
  let frontier = [ours];
  const seen = new Set(frontier);
  for (let depth = 0; depth < 3 && frontier.length; depth++) {
    const next = [];
    for (const str of frontier) {
      const cs = [...str];
      for (let i = 1; i < cs.length; i++) {
        if (!hasJong(cs[i - 1])) continue;
        for (let j = i; j < cs.length && isJamo(cs[j]); j++) {
          const cand = [...cs.slice(0, i), ...cs.slice(j + 1)].join("");
          if (cand === ohi) return true;
          if (!seen.has(cand)) {
            seen.add(cand);
            next.push(cand);
          }
        }
      }
    }
    frontier = next;
  }
  return false;
}
const KNOWN_DIFFERENCES = [["겹받침 뒤 왼손 키(오이는 버림)", onlyExtraJamoAfterJong]];

// ---------------------------------------------------------------- 실행

const seqs = makeSeqs(COUNT);
console.log(`키열 ${seqs.length}개 (seed ${SEED}, 최대 길이 ${MAXLEN})`);

execFileSync("cargo", ["build", "-q", "-p", "cssgsg-cli"], {
  cwd: repo,
  stdio: "inherit",
  env: { ...process.env, PATH: `${process.env.HOME}/.cargo/bin:${process.env.PATH}` },
});
const cli = path.join(repo, "build/cargo/debug/cssgsg-cli");
const res = spawnSync(cli, ["batch", "--mode", "ko"], { input: seqs.join("\n") + "\n", maxBuffer: 1 << 28 });
if (res.status !== 0) throw new Error("cssgsg-cli 실패: " + res.stderr);
const ours = res.stdout.toString("utf8").trimEnd().split("\n").map((l) => JSON.parse(l));

const dir = prepareOhi();
let dom = await loadOhi(dir);
const ohi = [];
for (let i = 0; i < seqs.length; i++) {
  if (i > 0 && i % 2000 === 0) {
    // 오래 쓰면 느려지므로 가끔 새 DOM으로 바꾼다.
    dom.window.close();
    dom = await loadOhi(dir);
  }
  ohi.push(ohiType(dom, seqs[i]));
}

// 상태 누수 확인: 표본을 새 DOM에서 다시 쳐서 같은지 본다.
const fresh = await loadOhi(dir);
const leak = seqs.slice(0, 200).filter((s, i) => ohiType(fresh, s) !== ohi[i]);
if (leak.length) console.log(`경고: 오이 상태 누수 의심 ${leak.length}건 (예: ${leak.slice(0, 3).join(", ")})`);

const counts = {};
const unexpected = [];
seqs.forEach((s, i) => {
  if (ohi[i] === ours[i]) return;
  const known = KNOWN_DIFFERENCES.find(([, f]) => f(s, ohi[i], ours[i]));
  if (known) counts[known[0]] = (counts[known[0]] ?? 0) + 1;
  else unexpected.push([s, ohi[i], ours[i]]);
});
const same = seqs.length - unexpected.length - Object.values(counts).reduce((a, b) => a + b, 0);
console.log(`일치 ${same}/${seqs.length}`);
for (const [name, n] of Object.entries(counts)) console.log(`알려진 차이 "${name}": ${n}`);
console.log(`예상 밖 차이: ${unexpected.length}`);
for (const [s, o, u] of unexpected.slice(0, SHOW)) console.log(`  ${JSON.stringify(s)}  오이 ${JSON.stringify(o)}  우리 ${JSON.stringify(u)}`);
process.exit(unexpected.length ? 1 : 0);

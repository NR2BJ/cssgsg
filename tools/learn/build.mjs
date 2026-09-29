// 학습 페이지(learn/index.html)를 만든다. 원본은 learn/template.html.
//
// - 배열도(SVG)와 新月 표는 엔진이 읽는 배열 데이터(cssgsg-cli layout-json)로 그린다. 그래서 엔진과 어긋나지 않는다.
// - 예시(<x-ex>)는 전부 엔진으로 쳐서 확인하고, 한 타씩 화면이 어떻게 바뀌는지 붙인다. 하나라도 틀리면 실패한다.
//
// 사용
//   node tools/learn/build.mjs           learn/index.html(앱·브라우저용 전체 문서)과 build/learn/artifact.html(조각)을 쓴다
//   node tools/learn/build.mjs --check   예시를 확인하고 learn/index.html이 최신인지 본다(쓰지 않음)
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const CHECK = process.argv.includes("--check");
const env = { ...process.env, PATH: `${process.env.HOME}/.cargo/bin:${process.env.PATH}` };
execFileSync("cargo", ["build", "-q", "-p", "cssgsg-cli"], { cwd: repo, env, stdio: "inherit" });
const CLI = path.join(repo, "build/cargo/debug/cssgsg-cli");
const layoutJson = (id) => JSON.parse(execFileSync(CLI, ["layout-json", id], { encoding: "utf8" }));

const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
const unesc = (s) => s.replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&");

// ---------------------------------------------------------------- 키보드 그림

const P = 60; // 한 칸
// 줄마다 [종류, 폭(칸), 이름]. 종류 "k"는 글자 키(쿼티 글자), "m"은 수식키·특수 키.
const ROWS = [
  [..."`1234567890-="].map((c) => ["k", 1, c]).concat([["m", 2, "delete"]]),
  [["m", 1.5, "tab"], ...[..."qwertyuiop[]"].map((c) => ["k", 1, c]), ["k", 1.5, "\\"]],
  [["m", 1.75, "caps"], ...[..."asdfghjkl;'"].map((c) => ["k", 1, c]), ["m", 2.25, "return"]],
  [["m", 2.25, "shift", "lshift"], ...[..."zxcvbnm,./"].map((c) => ["k", 1, c]), ["m", 2.75, "shift", "rshift"]],
];
const SHIFTED = { "`": "~", 1: "!", 2: "@", 3: "#", 4: "$", 5: "%", 6: "^", 7: "&", 8: "*", 9: "(", 0: ")", "-": "_", "=": "+",
  "[": "{", "]": "}", "\\": "|", ";": ":", "'": '"', ",": "<", ".": ">", "/": "?" };
const capLabel = (c) => (/[a-z]/.test(c) ? c.toUpperCase() : c);

function keyboard({ id, label, legends, hot = {}, divider = null }) {
  const parts = [];
  let y = 0;
  for (const row of ROWS) {
    let x = 0;
    for (const [kind, w, name, tag] of row) {
      const W = w * P;
      const cx = x + W / 2;
      const hotKey = hot[tag ?? name];
      if (kind === "m") {
        parts.push(`<rect class="k-box k-mod${hotKey ? " k-hot" : ""}" x="${x + 2}" y="${y + 2}" width="${W - 4}" height="${P - 4}" rx="8"/>`);
        if (hotKey) {
          parts.push(`<text class="k-hotlabel" x="${cx}" y="${y + 25}" text-anchor="middle">${esc(hotKey[0])}</text>`);
          parts.push(`<text class="k-hotsub" x="${cx}" y="${y + 44}" text-anchor="middle">${esc(hotKey[1])}</text>`);
        } else {
          parts.push(`<text class="k-modlabel" x="${x + 9}" y="${y + P - 11}">${esc(name)}</text>`);
        }
      } else {
        const lg = legends(name) ?? {};
        parts.push(`<rect class="k-box${lg.fill ? ` k-fill-${lg.fill}` : ""}" x="${x + 2}" y="${y + 2}" width="${W - 4}" height="${P - 4}" rx="8"><title>${esc(capLabel(name))} 자리${lg.title ? `: ${esc(lg.title)}` : ""}</title></rect>`);
        if (lg.shift) parts.push(`<text class="k-shift" x="${x + 9}" y="${y + 20}">${lg.shift}</text>`);
        if (lg.corner) parts.push(`<text class="k-corner ${lg.cornerClass ?? ""}" x="${x + W - 8}" y="${y + 20}" text-anchor="end">${esc(lg.corner)}</text>`);
        if (lg.main) parts.push(`<text class="k-main ${lg.mainClass ?? ""}" x="${cx}" y="${y + 41}" text-anchor="middle">${esc(lg.main)}</text>`);
        if (lg.sub) parts.push(`<text class="k-sub ${lg.subClass ?? ""}" x="${x + W - 8}" y="${y + P - 10}" text-anchor="end">${esc(lg.sub)}</text>`);
        parts.push(`<text class="k-cap" x="${x + 8}" y="${y + P - 10}">${esc(capLabel(name))}</text>`);
      }
      x += W;
    }
    y += P;
  }
  if (divider) {
    // 왼손/오른손 경계(줄마다 x 위치)
    const pts = divider.flatMap((dx, r) => [`${dx},${r * P + 3}`, `${dx},${(r + 1) * P - 3}`]).join(" ");
    parts.push(`<polyline class="k-divider" points="${pts}"/>`);
  }
  return `<div class="board-scroll"><svg class="board-svg" viewBox="-2 -2 904 244" role="img" aria-label="${esc(label)}" id="svg-${id}">${parts.join("")}</svg></div>`;
}

function chamshinBoard(id) {
  const l = layoutJson(id);
  const get = (key, shift) => l.keys.find((k) => k.key === key && k.shift === shift);
  return keyboard({
    id,
    label: `${l.name} 배열도`,
    legends: (key) => {
      const b = get(key, false), s = get(key, true);
      const lg = {};
      const role = (r) => (r.cho ? ["cho", r.cho] : r.jung ? ["jung", r.jung] : r.jong ? ["jong", r.jong] : null);
      if (b.passthrough != null) {
        lg.main = b.passthrough;
        lg.mainClass = "is-plain";
      } else if (b.cho && b.jung) {
        Object.assign(lg, { main: b.cho, mainClass: "is-cho", sub: b.jung, subClass: "is-jung", title: `초성 ${b.cho}, 초성 바로 뒤에서는 모음 ${b.jung}` });
      } else if (b.sym && (b.jung || b.jong)) {
        const [cls, ch] = role({ jung: b.jung, jong: b.jong });
        Object.assign(lg, { main: b.sym, mainClass: "is-sym", sub: ch, subClass: `is-${cls}`, title: cls === "jung" ? `초성 바로 뒤에서는 모음 ${ch}, 그 밖에는 ${b.sym}` : `초성+모음 뒤에서는 받침 ${ch}, 그 밖에는 ${b.sym}` });
      } else if (b.jung && b.jong) {
        Object.assign(lg, { main: b.jung, mainClass: "is-jung", sub: b.jong, subClass: "is-jong", title: `모음 ${b.jung}, 모음 뒤에서는 받침 ${b.jong}` });
      } else if (b.cho) {
        Object.assign(lg, { main: b.cho, mainClass: "is-cho", title: `초성 ${b.cho}` });
      } else if (b.jung) {
        Object.assign(lg, { main: b.jung, mainClass: "is-jung", title: `모음 ${b.jung}` });
      }
      if (s.passthrough != null) {
        lg.shift = `<tspan class="is-plain">${esc(s.passthrough)}</tspan>`;
      } else {
        const sym = s.action === "stop" ? "❖" : s.sym ?? "";
        lg.shift = `<tspan class="is-sym">${esc(sym)}</tspan>${s.jong ? `<tspan class="is-jong" dx="3">${esc(s.jong)}</tspan>` : ""}`;
        lg.title = `${lg.title ?? ""}${lg.title ? " · " : ""}Shift: ${s.action === "stop" ? "음절 조합 끊기 ❖" : sym}${s.jong ? ` (초성+모음 뒤에서는 받침 ${s.jong})` : ""}`;
      }
      return lg;
    },
  });
}

function graphiteBoard() {
  const l = layoutJson("graphite");
  return keyboard({
    id: "graphite",
    label: "Graphite 배열도",
    legends: (key) => {
      const k = l.keys.find((x) => x.key === key);
      const same = k.base === key;
      const shiftSame = k.shift === (SHIFTED[key] ?? key.toUpperCase());
      return {
        main: k.base,
        mainClass: same ? "is-plain" : "is-latin",
        shift: `<tspan class="${shiftSame ? "is-plain" : "is-sym"}">${esc(k.shift)}</tspan>`,
        title: same ? `쿼티와 같다 (${k.base})` : `${k.base}, Shift ${k.shift}`,
      };
    },
  });
}

function shingetsuBoard(l) {
  return keyboard({
    id: "shingetsu",
    label: "新月配列 배열도",
    divider: [360, 390, 405, 435],
    legends: (key) => {
      const k = l.keys.find((x) => x.key === key);
      const lg = {};
      if (key === l.star_key) Object.assign(lg, { main: "☆", mainClass: "is-star", fill: "star", title: "☆ 앞치기: 다음 왼손 키가 ☆면(주황) 가나" });
      else if (key === l.black_key) Object.assign(lg, { main: "★", mainClass: "is-black", fill: "black", title: "★ 앞치기: 다음 오른손 키가 ★면(남색) 가나" });
      else if (key === l.daku_key) Object.assign(lg, { main: "゛", mainClass: "is-daku", fill: "daku", title: "゛ 뒤치기: 앞 가나를 탁음·반탁음·작은 글자로" });
      else if (k.base) Object.assign(lg, { main: k.base, mainClass: "is-kana", title: `${k.base}` });
      else Object.assign(lg, { main: key, mainClass: "is-plain", title: "가나 배열 밖(쿼티 기호)" });
      if (k.star) Object.assign(lg, { corner: k.star, cornerClass: "is-star", title: `${lg.title} · ☆ 다음 ${k.star}` });
      if (k.black) Object.assign(lg, { corner: k.black, cornerClass: "is-black", title: `${lg.title} · ★ 다음 ${k.black}` });
      return lg;
    },
  });
}

function switchingBoard() {
  return keyboard({
    id: "switching",
    label: "전환 키",
    hot: { lshift: ["L⇧ 탭", "한 ↔ 일"], rshift: ["R⇧ 탭", "영 ↔ 이전 언어"], caps: ["Caps Lock", "일본어: 가타카나"] },
    legends: (key) => ({ main: capLabel(key), mainClass: "is-plain" }),
  });
}

// ---------------------------------------------------------------- 新月 표

function postfixTable(l) {
  const map = Object.fromEntries(l.postfix.map((p) => [p.from, p.to]));
  const chain = (k) => {
    const out = [k];
    while (map[out.at(-1)] && out.length < 4) out.push(map[out.at(-1)]);
    return out;
  };
  const rows = [
    ["か행", "かきくけこ"], ["さ행", "さしすせそ"], ["た행", "たちつてと"], ["は행", "はひふへほ"],
    ["あ행", "あいうえお"], ["や행·わ", "やゆよわ"],
  ];
  const body = rows
    .map(([name, kana]) => {
      const cells = [...kana].map((k) => {
        const c = chain(k);
        return `<td>${c.length > 1 ? c.map((x, i) => (i ? `<span class="arrow" aria-hidden="true">→</span>` : "") + `<span lang="ja">${x}</span>`).join("") : `<span lang="ja">${k}</span>`}</td>`;
      });
      return `<tr><th scope="row">${name}</th>${cells.join("")}${"<td></td>".repeat(5 - cells.length)}</tr>`;
    })
    .join("");
  return `<div class="table-scroll"><table class="grid-table"><caption>゛를 칠 때마다 오른쪽으로 바뀐다</caption><tbody>${body}</tbody></table></div>`;
}

function starDakuTable(l) {
  const items = [...l.star_daku].sort((a, b) => a.out.localeCompare(b.out, "ja"));
  return `<ul class="chips">${items
    .map((it) => `<li><span class="chip-out" lang="ja">${esc(it.out)}</span><span class="chip-keys"><kbd>K</kbd><kbd>L</kbd><kbd>${esc(capLabel(it.key))}</kbd></span></li>`)
    .join("")}</ul>`;
}

// ---------------------------------------------------------------- 예시 확인

const TOKEN = /\{[^}]*\}|[\s\S]/g;
const SPECIAL = { sp: "Space", bs: "⌫", ent: "⏎", esc: "Esc", tab: "Tab", left: "←", right: "→", up: "↑", down: "↓",
  rs: "R⇧ 탭", ls: "L⇧ 탭", caps: "Caps", click: "클릭" };
const BASE_OF = Object.fromEntries(Object.entries(SHIFTED).map(([b, s]) => [s, b]));
function keycap(tok) {
  if (tok.startsWith("{")) {
    const name = tok.slice(1, -1);
    const m = name.match(/^([SMCA])-(.+)$/);
    if (m) return ({ S: "⇧", M: "⌘", C: "⌃", A: "⌥" })[m[1]] + (SPECIAL[m[2]] ?? capLabel(m[2]));
    return SPECIAL[name] ?? name;
  }
  if (tok === " ") return "Space";
  if (/[A-Z]/.test(tok)) return `⇧${tok}`;
  if (BASE_OF[tok]) return `⇧${BASE_OF[tok]}`;
  return capLabel(tok);
}

function runBatch(mode, layout, lines) {
  const args = ["batch", "--mode", mode, ...(layout ? ["--ko-layout", layout] : [])];
  const out = execFileSync(CLI, args, { input: lines.join("\n") + "\n", encoding: "utf8" });
  return out.trimEnd().split("\n").map((l) => JSON.parse(l));
}

function renderExamples(html) {
  const re = /<x-ex\s+([^>]*)>([\s\S]*?)<\/x-ex>/g;
  const found = [...html.matchAll(re)].map((m) => {
    const attrs = Object.fromEntries([...m[1].matchAll(/(\w+)="([^"]*)"/g)].map((a) => [a[1], unesc(a[2])]));
    return { raw: m[0], mode: attrs.m, keys: attrs.k, layout: attrs.l ?? null, steps: attrs.steps !== "0", want: unesc(m[2]) };
  });
  // (모드, 배열)별로 모아 한 번에 친다: 예시마다 모든 앞부분(한 타, 두 타, …)
  const groups = new Map();
  for (const ex of found) {
    ex.tokens = ex.keys.match(TOKEN);
    const g = `${ex.mode}|${ex.layout ?? ""}`;
    if (!groups.has(g)) groups.set(g, []);
    groups.get(g).push(ex);
  }
  const failures = [];
  for (const [g, list] of groups) {
    const [mode, layout] = g.split("|");
    const lines = list.flatMap((ex) => ex.tokens.map((_, i) => ex.tokens.slice(0, i + 1).join("")));
    const outs = runBatch(mode, layout || null, lines);
    let i = 0;
    for (const ex of list) {
      ex.screens = outs.slice(i, i + ex.tokens.length);
      i += ex.tokens.length;
      if (ex.screens.at(-1) !== ex.want) failures.push(`[${mode}${layout ? " " + layout : ""}] ${ex.keys} → 엔진 ${JSON.stringify(ex.screens.at(-1))}, 페이지 ${JSON.stringify(ex.want)}`);
    }
  }
  if (failures.length) {
    console.error(`예시 ${failures.length}개가 엔진과 다르다:\n  ${failures.join("\n  ")}`);
    process.exit(1);
  }
  let out = html;
  for (const ex of found) {
    const lang = { ko: "ko", ja: "ja", en: "en" }[ex.mode];
    const showSteps = ex.steps && ex.tokens.length <= 7;
    const keys = ex.tokens
      .map((t, i) => {
        const cap = `<kbd>${esc(keycap(t))}</kbd>`;
        return showSteps ? `<span class="ex-step">${cap}<span class="ex-st" lang="${lang}">${esc(ex.screens[i]) || "&#8203;"}</span></span>` : cap;
      })
      .join("");
    const label = `${ex.want}: ${ex.tokens.map(keycap).join(" ")}`;
    out = out.replace(ex.raw, `<span class="ex" role="group" aria-label="${esc(label)}"><span class="ex-out" lang="${lang}">${esc(ex.want)}</span><span class="ex-keys${showSteps ? " has-steps" : ""}">${keys}</span></span>`);
  }
  return { html: out, count: found.length };
}

// ---------------------------------------------------------------- 조립

const template = fs.readFileSync(path.join(repo, "learn/template.html"), "utf8");
const sg = layoutJson("shingetsu");
let page = template
  .replace("<!--kbd:chamshin-v18-->", chamshinBoard("chamshin-v18"))
  .replace("<!--kbd:chamshin-d-v19-->", chamshinBoard("chamshin-d-v19"))
  .replace("<!--kbd:graphite-->", graphiteBoard())
  .replace("<!--kbd:shingetsu-->", shingetsuBoard(sg))
  .replace("<!--kbd:switching-->", switchingBoard())
  .replace("<!--table:postfix-->", postfixTable(sg))
  .replace("<!--table:stardaku-->", starDakuTable(sg));
const leftover = page.match(/<!--(kbd|table):[^>]*-->/);
if (leftover) throw new Error(`채우지 못한 자리: ${leftover[0]}`);
const { html: fragment, count } = renderExamples(page);

const NOTE = "<!-- 생성 파일: tools/learn/build.mjs가 learn/template.html과 배열 데이터로 만든다. 직접 고치지 않는다. -->";
const head = fragment.match(/^\s*(<title>[\s\S]*?<\/title>)\s*(<style>[\s\S]*?<\/style>)/);
if (!head) throw new Error("template.html은 <title>과 <style>로 시작해야 한다");
const body = fragment.slice(head[0].length).trim();
const full = `<!doctype html>\n${NOTE}\n<html lang="ko">\n<head>\n<meta charset="utf-8">\n<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">\n${head[1]}\n${head[2]}\n</head>\n<body>\n${body}\n</body>\n</html>\n`;

const target = path.join(repo, "learn/index.html");
if (CHECK) {
  const current = fs.existsSync(target) ? fs.readFileSync(target, "utf8") : "";
  if (current !== full) {
    console.error("learn/index.html이 최신이 아니다. node tools/learn/build.mjs로 다시 만든다.");
    process.exit(1);
  }
  console.log(`예시 ${count}개 확인, learn/index.html 최신`);
} else {
  fs.writeFileSync(target, full);
  fs.mkdirSync(path.join(repo, "build/learn"), { recursive: true });
  fs.writeFileSync(path.join(repo, "build/learn/artifact.html"), `${fragment.trim()}\n`);
  console.log(`예시 ${count}개 확인 → learn/index.html, build/learn/artifact.html`);
}

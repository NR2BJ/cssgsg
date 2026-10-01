// 셸 키 코드 표 교차 검증.
// 코어의 맥 keyCode 표(Key::from_mac_keycode)와 윈도우 스캔 코드 표(Key::from_windows_scancode)를
// Chromium의 DOM 코드 표(USB HID ↔ 윈도우 스캔 코드 ↔ 맥 keyCode, BSD 라이선스, 받기만 한다)와 대조한다.
// 사용: node tools/crosscheck/keycodes.mjs   (의존성 없음, Node 18+)
import { execFileSync } from "node:child_process";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const CHROMIUM =
  "https://raw.githubusercontent.com/chromium/chromium/863ae6f44a6d93d0aaa3674b0554f4a9298d5fd1/ui/events/keycodes/dom/dom_code_data.inc";

// 출처 사이의 알려진 차이: "표 코드" → 이유
const KNOWN = {
  "mac 0x3f": "Fn(Globe): Chromium은 맥 키코드를 잇지 않는다. 코어는 HID 키보드 페이지 밖의 값 0x0100을 따로 잡아 쓴다",
  "mac 0x72":
    "kVK_Help: 코어는 Apple 이름대로 Help, Chromium은 PC 키보드의 Insert 자리로 잇는다(맥은 Insert 키를 이 코드로 준다). 입력기는 두 키 모두 쓰지 않는다",
};

const hex = (n, w = 2) => "0x" + n.toString(16).padStart(w, "0");

function ours() {
  const exe = process.platform === "win32" ? ".exe" : "";
  const env = { ...process.env, PATH: `${path.join(os.homedir(), ".cargo", "bin")}${path.delimiter}${process.env.PATH}` };
  execFileSync("cargo", ["build", "-q", "-p", "cssgsg-cli"], { cwd: repo, env, stdio: "inherit" });
  const out = execFileSync(path.join(repo, `build/cargo/debug/cssgsg-cli${exe}`), ["keycodes"], { encoding: "utf8" });
  return JSON.parse(out);
}

async function chromium() {
  const res = await fetch(CHROMIUM);
  if (!res.ok) throw new Error(`Chromium 표를 받지 못했다: ${res.status}`);
  const rows = [];
  const re = /DOM_CODE\((0x[0-9a-f]+), 0x[0-9a-f]+, 0x[0-9a-f]+, (0x[0-9a-f]+), (0x[0-9a-f]+), "(\w+)"/g;
  for (const [, usb, win, mac, code] of (await res.text()).matchAll(re)) {
    rows.push({ usb: Number(usb), win: Number(win), mac: Number(mac), code });
  }
  if (rows.length < 100) throw new Error(`Chromium 표 형식이 바뀐 것 같다(${rows.length}줄)`);
  return rows;
}

const [table, rows] = await Promise.all([Promise.resolve(ours()), chromium()]);
const problems = [];
const known = [];
const name = (hid) => rows.find((r) => r.usb === 0x070000 + hid)?.code ?? `HID ${hex(hid)}`;

// 1. 우리 표의 항목마다 Chromium도 같은 키라고 하는지
for (const [side, field, width] of [["mac", "mac", 2], ["windows", "win", 4]]) {
  for (const [code, hid] of table[side]) {
    const id = `${side === "mac" ? "mac" : "win"} ${hex(code, width === 4 && code > 0xff ? 4 : 2)}`;
    const hits = rows.filter((r) => r[field] === code && !(side === "mac" && code === 0xffff));
    const want = 0x070000 + hid;
    if (KNOWN[id]) {
      known.push(`${id}: ${KNOWN[id]}`);
    } else if (hits.length === 0) {
      problems.push(`${id} → ${name(hid)}: Chromium에 이 코드가 없다`);
    } else if (!hits.some((r) => r.usb === want)) {
      problems.push(`${id} → 우리 ${name(hid)}, Chromium ${hits.map((r) => r.code).join("/")}`);
    }
  }
}

// 2. 거꾸로: 우리가 아는 물리 키를 Chromium은 다른 코드로도 내는지(빠진 별칭)
const missing = [];
for (const [side, field] of [["mac", "mac"], ["windows", "win"]]) {
  const have = new Map(table[side].map(([code, hid]) => [code, hid]));
  const hids = new Set(table[side].map(([, hid]) => hid));
  for (const r of rows) {
    const hid = r.usb - 0x070000;
    const code = r[field];
    if (hid < 0 || hid > 0xff || !hids.has(hid)) continue;
    // 없음 표시: 윈도우는 0, 맥은 0xffff(맥 0은 A 키다)
    if (side === "windows" ? code === 0 : code === 0xffff) continue;
    if (have.get(code) !== hid) missing.push(`${side} ${hex(code, code > 0xff ? 4 : 2)} → ${r.code}`);
  }
}

console.log(`맥 ${table.mac.length}개, 윈도우 ${table.windows.length}개를 Chromium ${rows.length}줄과 대조했다.`);
for (const k of known) console.log(`  알려진 차이 — ${k}`);
for (const m of missing) console.log(`  빠진 별칭 — ${m}`);
if (problems.length) {
  console.log(`예상 밖 차이 ${problems.length}개:`);
  for (const p of problems) console.log(`  ${p}`);
  // process.exit()는 윈도우에서 fetch가 남긴 핸들을 닫는 중에 libuv 단언으로 죽는다. 종료 코드만 정한다.
  process.exitCode = 1;
} else {
  console.log("예상 밖 차이 0");
}

// 셸 스모크 테스트용 무작위 키열. cssgsg-cli batch와 shell-smoke에 같은 것을 넣는다.
// node corpus.mjs <en|ko|ja> <줄 수> [시드]   (같은 인자면 같은 결과)
const [mode = "ko", count = "3000", seed = "1"] = process.argv.slice(2);
let x = (Number(seed) * 2654435761 + { en: 1, ko: 2, ja: 3 }[mode]) >>> 0 || 1;
const rnd = () => {
  x ^= x << 13;
  x >>>= 0;
  x ^= x >>> 17;
  x ^= x << 5;
  x >>>= 0;
  return x / 2 ** 32;
};
const pick = (list) => list[Math.floor(rnd() * list.length)];

// 글자판(세 배열이 쓰는 키), 대문자, 숫자·기호, 특수 키. { }는 키열 문법 기호라 {S-[} {S-]}로 친다.
const LETTERS = [..."abcdefghijklmnopqrstuvwxyz;',./[]"];
const UPPER = [..."ABCDEFGHIJKLMNOPQRSTUVWXYZ"];
const OTHER = [..."0123456789-=`\\!@#$%^&*()_+~|:\"<>?"];
const SPECIAL = [
  "{sp}", "{sp}", "{bs}", "{bs}", "{ent}", "{esc}", "{tab}", "{rs}", "{ls}", "{caps}", "{click}",
  "{left}", "{del}", "{S-ent}", "{S-sp}", "{S-[}", "{S-]}", "{M-a}", "{C-c}", "{A-a}", "{A-ent}",
];

const lines = [];
for (let i = 0; i < Number(count); i++) {
  const length = 1 + Math.floor(rnd() * 16);
  let s = "";
  for (let j = 0; j < length; j++) {
    const r = rnd();
    s += r < 0.55 ? pick(LETTERS) : r < 0.63 ? pick(UPPER) : r < 0.73 ? pick(OTHER) : pick(SPECIAL);
  }
  lines.push(s);
}
process.stdout.write(lines.join("\n") + "\n");

// Reads raw rgb24 frames (width x height) and prints the run-length sequence of left|right colour states.
const fs = require('fs');
const [file, W, H] = [process.argv[2], +process.argv[3], +process.argv[4]];
const buf = fs.readFileSync(file), size = W * H * 3, n = Math.floor(buf.length / size);
const cls = (r, g, b) => (r > 140 && g < 90 && b < 90) ? 'R' : (b > 140 && r < 90 && g < 90) ? 'B' : (g > 110 && r < 90 && b < 90) ? 'G' : (r < 60 && g < 60 && b < 60) ? 'K' : (r > 200 && g > 200 && b > 200) ? 'W' : '?';
function region(f, x0, x1) {
  const c = {};
  for (let y = 2; y < H - 2; y++) for (let x = x0; x < x1; x++) { const i = f * size + (y * W + x) * 3; const k = cls(buf[i], buf[i + 1], buf[i + 2]); c[k] = (c[k] || 0) + 1; }
  const tot = Object.values(c).reduce((a, b) => a + b, 0);
  const top = Object.entries(c).sort((a, b) => b[1] - a[1])[0];
  return top[1] / tot > 0.85 ? top[0] : 'mixed';
}
const states = [];
for (let f = 0; f < n; f++) states.push(region(f, 1, W / 2 - 2) + '|' + region(f, W / 2 + 2, W - 1));
const runs = [];
for (const s of states) { if (runs.length && runs[runs.length - 1][0] === s) runs[runs.length - 1][1]++; else runs.push([s, 1]); }
console.log(`${n} frames (~${(n / 60).toFixed(1)} s at 60 fps). left|right state runs:`);
console.log(runs.map(([s, c]) => `${s} x${c}`).join('  ->  '));
const expected = new Set(['R|B', 'R|R', 'B|B', 'G|G']);
const odd = runs.filter(([s]) => !expected.has(s));
console.log(odd.length ? `FLASH CANDIDATES: ${odd.map(([s, c]) => s + ' x' + c).join(', ')}` : 'no unexpected frames: every frame was split (R|B), only A (R|R) or only B (B|B) or only C (G|G)');

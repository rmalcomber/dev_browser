// Reads raw rgb24 frames and prints the run-length sequence of region colour states, flagging frames that are not an expected state.
// Promoted from the M0 spike's flash-analyze.js (which hard-coded a left|right split of two panes). This is the S8 regression check's engine
// (planning/security-acceptance-tests.md: switching workspace never shows a frame of the old one).
//
//   node tools/analyze-frames.js <raw file> <width> <height> [--regions 0.02-0.12,0.3-0.95] [--expect K|R,K|B] [--fps 60]
//
// Regions are fractions of the width (each region is classified over the full height, minus a 2px margin). A region's class is
// R, G, B, W (white), K (dark) or ? (anything else), or 'mixed' when no class covers 85% of it. Exit code 1 if an unexpected state appears.
const fs = require('fs');
const argv = process.argv.slice(2);
const opt = (name, dflt) => { const i = argv.indexOf('--' + name); return i >= 0 ? argv[i + 1] : dflt; };
const [file, W, H] = [argv[0], +argv[1], +argv[2]];
if (!file || !W || !H) { console.error('usage: node analyze-frames.js <raw> <width> <height> [--regions a-b,c-d] [--expect X|Y,...] [--fps N]'); process.exit(2); }
const regions = opt('regions', '0.02-0.12,0.3-0.95').split(',').map((r) => r.split('-').map(Number));
const expect = opt('expect', '') ? new Set(opt('expect', '').split(',')) : null;
const fps = +opt('fps', 60);
const buf = fs.readFileSync(file), size = W * H * 3, n = Math.floor(buf.length / size);
const cls = (r, g, b) => (r > 140 && g < 90 && b < 90) ? 'R' : (b > 140 && r < 90 && g < 90) ? 'B' : (g > 110 && r < 90 && b < 90) ? 'G'
  : (r < 60 && g < 60 && b < 60) ? 'K' : (r > 200 && g > 200 && b > 200) ? 'W' : '?';
function region(f, a, b) {
  const x0 = Math.max(0, Math.floor(a * W)), x1 = Math.min(W, Math.ceil(b * W)), c = {};
  for (let y = 2; y < H - 2; y++) for (let x = x0; x < x1; x++) { const i = f * size + (y * W + x) * 3; const k = cls(buf[i], buf[i + 1], buf[i + 2]); c[k] = (c[k] || 0) + 1; }
  const tot = Object.values(c).reduce((p, q) => p + q, 0) || 1;
  const top = Object.entries(c).sort((p, q) => q[1] - p[1])[0];
  return top && top[1] / tot > 0.85 ? top[0] : 'mixed';
}
const runs = [];
for (let f = 0; f < n; f++) {
  const s = regions.map(([a, b]) => region(f, a, b)).join('|');
  if (runs.length && runs[runs.length - 1][0] === s) runs[runs.length - 1][1]++; else runs.push([s, 1]);
}
console.log(`${n} frames (~${(n / fps).toFixed(1)} s at ${fps} fps). region state runs:`);
console.log(runs.map(([s, c]) => `${s} x${c}`).join('  ->  '));
if (!expect) process.exit(0);
const odd = runs.filter(([s]) => !expect.has(s));
if (odd.length) { console.log(`UNEXPECTED FRAMES: ${odd.map(([s, c]) => s + ' x' + c).join(', ')}`); process.exit(1); }
console.log(`no unexpected frames: every frame was one of ${[...expect].join(', ')}`);

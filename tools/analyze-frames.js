// Reads raw rgb24 frames and prints the run-length sequence of region colour states, flagging frames that are not an expected state.
// Promoted from the M0 spike's flash-analyze.js (which hard-coded a left|right split of two panes). This is the S8 regression check's engine
// (planning/security-acceptance-tests.md: switching workspace never shows a frame of the old one).
//
//   node tools/analyze-frames.js <raw file> <width> <height> [--regions 0.02-0.12,0.3-0.95] [--expect K|R,K|B] [--fps 60]
//                                [--cycle K|R,K|B,K|G [--gap K|K] [--min-states N]]
//
// --cycle is the S8 check ("switching never shows a frame of the previous workspace"): the states must go round the listed cycle in order,
// each change moving to the NEXT state, never back to the one just left. Frames in the --gap state (a dark neutral gap) are allowed
// between them and are not counted as a state. Any other state, or a change that is not the next one in the cycle, is a violation.
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
const cycle = opt('cycle', '') ? opt('cycle', '').split(',') : null;
if (cycle) {
  const gap = opt('gap', '');
  const gapRuns = runs.filter(([s]) => s === gap);
  const maxGap = gapRuns.reduce((m, [, c]) => Math.max(m, c), 0);
  // drop gap runs, then merge neighbours that are the same state again
  const states = [];
  for (const [s, c] of runs) {
    if (s === gap) continue;
    if (states.length && states[states.length - 1][0] === s) states[states.length - 1][1] += c;
    else states.push([s, c]);
  }
  const bad = [];
  states.forEach(([s], i) => {
    if (!cycle.includes(s)) bad.push(`unexpected state ${s} x${states[i][1]}`);
  });
  for (let i = 1; i < states.length; i++) {
    const a = cycle.indexOf(states[i - 1][0]), b = cycle.indexOf(states[i][0]);
    if (a >= 0 && b >= 0 && b !== (a + 1) % cycle.length) bad.push(`${states[i - 1][0]} -> ${states[i][0]} is not the next state in the cycle (a frame of the previous one came back, or one was skipped)`);
  }
  const minStates = +opt('min-states', 0);
  if (states.length < minStates) bad.push(`only ${states.length} state(s) seen, at least ${minStates} expected: the switches did not show`);
  console.log(`cycle ${cycle.join(' -> ')}: ${states.length} states seen, ${gapRuns.length} gap run(s), longest gap ${maxGap} frame(s) (${(maxGap / fps * 1000).toFixed(0)} ms)`);
  if (bad.length) { console.log('VIOLATIONS: ' + bad.join('; ')); process.exit(1); }
  console.log('no violations: every change went to the next state in the cycle');
  process.exit(0);
}
if (!expect) process.exit(0);
const odd = runs.filter(([s]) => !expect.has(s));
if (odd.length) { console.log(`UNEXPECTED FRAMES: ${odd.map(([s, c]) => s + ' x' + c).join(', ')}`); process.exit(1); }
console.log(`no unexpected frames: every frame was one of ${[...expect].join(', ')}`);

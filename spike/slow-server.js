// Test page server for the destroy checks. Usage: node slow-server.js <port> [delayMs] <logfile>
// /page (default)  answers after delayMs with a solid green page            /busy      green page whose script spins forever
// /sse             page holding an endless event stream open                /ws        page holding a WebSocket open
// /download        page that starts a download that never finishes          /alert     page that opens alert() and sets onbeforeunload
// /popup           page that calls window.open on load                       /geo /notify /clipboard /camera  pages that ask for a permission
// Every request, abort and close is logged so a run can say whether the connection really went away.
const http = require('http'), crypto = require('crypto'), fs = require('fs');
const [port, delay, log] = [+process.argv[2], +(process.argv[3] || 3000), process.argv[4]];
const out = (m) => fs.appendFileSync(log, `${Date.now()} ${m}\n`);
const green = (extra = '', body = '<b>C</b>') => `<!doctype html><body style="background:#0a0;color:#fff;font:200px sans-serif">${body}<script>${extra}</script>`;
const pages = {
  '/busy': green('setTimeout(function(){ document.title = "busy"; while (true) {} }, 400)'),
  '/sse': green('var es = new EventSource("/events"); es.onmessage = function(){}', '<b>S</b>'),
  '/ws': green('var ws = new WebSocket("ws://" + location.host + "/socket"); ws.onmessage = function(){}', '<b>W</b>'),
  '/download': green('setTimeout(function(){ var a = document.createElement("a"); a.href = "/file"; a.download = "big.bin"; document.body.appendChild(a); a.click(); }, 300)', '<b>D</b>'),
  '/alert': green('window.onbeforeunload = function(){ return "stay"; }; setTimeout(function(){ alert("hello from workspace C"); }, 300)', '<b>!</b>'),
  '/popup': green('setTimeout(function(){ window.open("/popped", "_blank"); }, 300)', '<b>P</b>'),
  '/geo': green('setTimeout(function(){ navigator.geolocation.getCurrentPosition(function(){}, function(){}); }, 300)', '<b>G</b>'),
  '/notify': green('setTimeout(function(){ Notification.requestPermission(); }, 300)', '<b>N</b>'),
  '/clipboard': green('setTimeout(function(){ navigator.clipboard.readText().catch(function(){}); }, 300)', '<b>K</b>'),
  '/camera': green('setTimeout(function(){ navigator.mediaDevices.getUserMedia({ video: true, audio: true }).catch(function(){}); }, 300)', '<b>M</b>'),
  '/livecam': green(`var q = new URLSearchParams(location.search), who = q.get('who') || 'C', both = q.get('kinds') === 'both';
function rep(m){ fetch('/report?who=' + who + '&kind=live&msg=' + encodeURIComponent(m)).catch(function(){}); }
var c = both ? { video: true, audio: true } : { video: true }, retried = false;
var evade = q.get('evade') === '1';
if (q.get('tamper') === '1') { MediaStreamTrack.prototype.stop = function(){}; }
function gum(cc){ return evade ? MediaDevices.prototype.getUserMedia.call(navigator.mediaDevices, cc) : navigator.mediaDevices.getUserMedia(cc); }
function start(label){ return gum(c).then(function(st){
  rep(label + 'stream granted: ' + st.getTracks().map(function(t){ return t.kind; }).join('+'));
  var use = q.get('clone') === '1' ? st.clone() : st;
  var v = document.createElement('video'); v.muted = true; v.srcObject = use; v.play(); var n = 0;
  function f(){ n++; v.requestVideoFrameCallback(f); } v.requestVideoFrameCallback(f);
  setInterval(function(){
    var parts = use.getTracks().map(function(t){ return t.kind + '=' + t.readyState; }).join(' ');
    rep(label + parts + ' frames=' + n);
    if (!retried && use.getTracks().every(function(t){ return t.readyState === 'ended'; })) { retried = true; setTimeout(function(){ start('RETRY '); }, 1500); }
  }, 1000);
}, function(e){ rep(label + 'denied ' + e.name); }); }
start('');`, '<b>L</b>'),
  '/liveframe': '<!doctype html><body style="background:#0a0;margin:0"><iframe src="/livecam?who=Cframe" allow="camera; microphone" style="width:600px;height:300px;border:0"></iframe>',
  '/liveframe-x': green("var f = document.createElement('iframe'); f.src = 'http://localhost:' + location.port + '/livecam?who=Cx'; f.allow = 'camera; microphone'; f.style.cssText = 'width:600px;height:300px;border:0'; document.body.appendChild(f);", ''),
  '/liveframe-s': green("var f = document.createElement('iframe'); f.src = 'http://127.0.0.1:' + (+location.port + 1) + '/livecam?who=Cs'; f.allow = 'camera; microphone'; f.style.cssText = 'width:600px;height:300px;border:0'; document.body.appendChild(f);", ''),
  '/popped': green('', '<b>popup</b>'),
};
// /permcheck?kind=geo|notify|clipboard|camera&mode=request|query&who=C : reports the permission state before, asks for it
// (unless mode=query), and reports the outcome and the state after to /report, which the server logs as "report ...".
const permPage = green(`
var q = new URLSearchParams(location.search), kind = q.get('kind'), mode = q.get('mode') || 'request', who = q.get('who') || 'C';
function rep(m) { fetch('/report?who=' + who + '&kind=' + kind + '&msg=' + encodeURIComponent(m)).catch(function(){}); }
var names = { geo: 'geolocation', notify: 'notifications', clipboard: 'clipboard-read', camera: 'camera', mic: 'microphone', cammic: 'camera' };
function state() { return navigator.permissions.query({ name: names[kind] }).then(function(s){ return s.state; }, function(e){ return 'query-failed:' + e.name; }); }
function ask() {
  if (kind === 'geo') return new Promise(function(r){ navigator.geolocation.getCurrentPosition(function(){ r('position'); }, function(e){ r('error code ' + e.code); }); });
  if (kind === 'notify') return Notification.requestPermission();
  if (kind === 'clipboard') return navigator.clipboard.readText().then(function(){ return 'read ok'; }, function(e){ return e.name; });
  var c = kind === 'mic' ? { audio: true } : kind === 'cammic' ? { video: true, audio: true } : { video: true };
  return navigator.mediaDevices.getUserMedia(c).then(function(st){ var n = st.getTracks().map(function(t){ return t.kind + ' [' + t.label + ']'; }).join(' + '); st.getTracks().forEach(function(t){ t.stop(); }); return 'stream(' + n + ')'; }, function(e){ return e.name; });
}
state().then(function(b){ rep('state-before=' + b); if (mode === 'query') return; return ask().then(function(r){ rep('outcome=' + r); return state().then(function(a){ rep('state-after=' + a); }); }); });
`, '<b>?</b>');
const server = http.createServer((req, res) => {
  const id = Math.random().toString(36).slice(2, 6);
  const url = req.url.split('?')[0];
  if (url === '/permcheck') { res.writeHead(200, { 'content-type': 'text/html', 'cache-control': 'no-store' }); return res.end(permPage); }
  if (url === '/report') { const q = new URL(req.url, 'http://x').searchParams; out(`report ${q.get('who')} ${q.get('kind')} ${q.get('msg')}`); res.writeHead(204); return res.end(); }
  out(`request ${id} ${url}`);
  res.on('close', () => out(`closed ${id} ${url}${res.writableFinished ? ' (finished)' : ' (client went away or reset)'}`));
  if (pages[url]) { res.writeHead(200, { 'content-type': 'text/html', 'cache-control': 'no-store' }); return res.end(pages[url]); }
  if (url === '/events') {
    res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-store' });
    const t = setInterval(() => res.write('data: tick\n\n'), 200); res.on('close', () => clearInterval(t)); return;
  }
  if (url === '/file') {
    res.writeHead(200, { 'content-type': 'application/octet-stream', 'content-disposition': 'attachment; filename="big.bin"', 'cache-control': 'no-store' });
    const t = setInterval(() => res.write(Buffer.alloc(16384, 1)), 100); res.on('close', () => clearInterval(t)); return;
  }
  if (url === '/favicon.ico') { res.writeHead(404); return res.end(); }
  setTimeout(() => {
    if (res.destroyed) return;
    res.writeHead(200, { 'content-type': 'text/html', 'cache-control': 'no-store' });
    res.end(green());
    out(`sent ${id}`);
  }, delay);
});
server.on('upgrade', (req, socket) => {
  const key = req.headers['sec-websocket-key'];
  const accept = crypto.createHash('sha1').update(key + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64');
  socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
  out('ws open');
  const t = setInterval(() => { try { socket.write(Buffer.from([0x81, 4, 0x74, 0x69, 0x63, 0x6b])); } catch (e) {} }, 300);
  socket.on('data', () => {});
  socket.on('error', () => {});
  socket.on('close', () => { clearInterval(t); out('ws closed (client went away)'); });
});
server.listen(port, '127.0.0.1', () => out('listening'));
// More origins for the cross-origin iframe checks: port+1 is another origin of the same site; localhost is another site.
const handler = server.listeners('request')[0], upgrader = server.listeners('upgrade')[0];
for (const [p, h] of [[port + 1, '127.0.0.1'], [port, '::1'], [port + 1, '::1']]) {
  const extra = http.createServer(handler); extra.on('upgrade', upgrader); extra.on('error', () => {}); extra.listen(p, h);
}

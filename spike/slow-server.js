// Test page server for the mid-load check: every request is answered after DELAY ms with a solid green page, and the server
// logs whether the client was still there. Usage: node slow-server.js <port> [delayMs] <logfile>
const http = require('http'), fs = require('fs');
const [port, delay, log] = [+process.argv[2], +(process.argv[3] || 3000), process.argv[4]];
const out = (m) => fs.appendFileSync(log, `${Date.now()} ${m}\n`);
http.createServer((req, res) => {
  const id = Math.random().toString(36).slice(2, 6);
  out(`request ${id} ${req.url}`);
  let done = false;
  res.on('close', () => { if (!done) out(`aborted ${id} (client went away before the response)`); });
  setTimeout(() => {
    if (res.destroyed) return;
    done = true;
    res.writeHead(200, { 'content-type': 'text/html', 'cache-control': 'no-store' });
    res.end('<body style="background:#0a0;color:#fff;font:200px sans-serif"><b>C</b>');
    out(`sent ${id}`);
  }, delay);
}).listen(port, '127.0.0.1', () => out('listening'));

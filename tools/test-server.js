// A tiny static server for the pages in tools/test-pages. Usage: node tools/test-server.js <port>
// Serves only files directly inside test-pages (no sub-folders, no path tricks). `?delay=<ms>` delays the response. Prints "listening <port>" when ready.
const http = require('http'), fs = require('fs'), path = require('path');
const dir = path.join(__dirname, 'test-pages');
const port = +process.argv[2] || 0;
const types = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.css': 'text/css', '.png': 'image/png' };
http.createServer((req, res) => {
  const name = decodeURIComponent(req.url.split('?')[0].replace(/^\//, ''));
  if (!/^[A-Za-z0-9_.-]+$/.test(name) || name.startsWith('.')) { res.writeHead(404); return res.end('not found'); }
  fs.readFile(path.join(dir, name), (err, data) => {
    if (err) { res.writeHead(404); return res.end('not found'); }
    // ?delay=<ms> holds the response back (up to 20 s), to test loading and stopping
    const delay = Math.min(+((req.url.split('?')[1] || '').match(/(?:^|&)delay=(\d+)/) || [])[1] || 0, 20000);
    setTimeout(() => {
      res.writeHead(200, { 'Content-Type': types[path.extname(name)] || 'application/octet-stream' });
      res.end(data);
    }, delay);
  });
}).listen(port, '127.0.0.1', function () { console.log('listening ' + this.address().port); });

// An https server with a self-signed certificate, to check that the browser refuses it. Usage: node tools/test-https-server.js <port> <key.pem> <cert.pem>
// Prints "listening <port>" and then "request <n>" for every request that actually reaches it (there must be none).
const https = require('https'), fs = require('fs');
let n = 0;
https.createServer({ key: fs.readFileSync(process.argv[3]), cert: fs.readFileSync(process.argv[4]) }, (req, res) => {
  console.log('request ' + (++n));
  res.writeHead(200, { 'Content-Type': 'text/html' });
  res.end('<title>reached</title>reached');
}).listen(+process.argv[2] || 0, '127.0.0.1', function () { console.log('listening ' + this.address().port); });

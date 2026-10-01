const http = require('http');
const port = parseInt(process.argv[2] || '11880', 10);
const server = http.createServer((req, res) => {
  res.writeHead(200, { 'Content-Type': 'text/plain' });
  res.end('T06B-SMOKE-OK');
});
server.listen(port, '127.0.0.1', () => console.log('listening ' + port));

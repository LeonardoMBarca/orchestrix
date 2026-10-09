import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';

const files = new Map([
  ['/', ['index.html', 'text/html; charset=utf-8']],
  ['/index.html', ['index.html', 'text/html; charset=utf-8']],
  ['/styles.css', ['styles.css', 'text/css; charset=utf-8']],
  ['/app.js', ['app.js', 'text/javascript; charset=utf-8']],
  ['/workspace.js', ['workspace.js', 'text/javascript; charset=utf-8']],
  ['/connections.js', ['connections.js', 'text/javascript; charset=utf-8']],
]);
const server = createServer(async (req, res) => {
  const pathname = new URL(req.url, 'http://127.0.0.1').pathname;
  const file = files.get(pathname);
  if (!file || !['GET', 'HEAD'].includes(req.method)) {
    res.writeHead(404); res.end('Not found'); return;
  }
  try {
    const body = await readFile(fileURLToPath(new URL(file[0], import.meta.url)));
    res.writeHead(200, {'Content-Type':file[1], 'Cache-Control':'no-store', 'X-Content-Type-Options':'nosniff'});
    res.end(req.method === 'HEAD' ? undefined : body);
  } catch {
    res.writeHead(500); res.end('Unable to read prototype asset');
  }
});
server.on('error', error => {console.error(error.message); process.exitCode=1;});
server.listen(Number(process.env.ORCHESTRIX_PROTOTYPE_PORT || 4173), '127.0.0.1', () => {
  console.log(`Orchestrix prototype: http://127.0.0.1:${server.address().port}`);
});

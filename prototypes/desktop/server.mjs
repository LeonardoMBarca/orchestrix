import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {createDiscoveryHandler} from './host-discovery.mjs';

const discoveryHandler=createDiscoveryHandler();

const files = new Map([
  ['/', ['index.html', 'text/html; charset=utf-8']],
  ['/index.html', ['index.html', 'text/html; charset=utf-8']],
  ['/styles.css', ['styles.css', 'text/css; charset=utf-8']],
  ['/identity.css', ['identity.css', 'text/css; charset=utf-8']],
  ['/i18n.js', ['i18n.js', 'text/javascript; charset=utf-8']],
  ['/locales/catalog.js', ['locales/catalog.js', 'text/javascript; charset=utf-8']],
  ['/app.js', ['app.js', 'text/javascript; charset=utf-8']],
  ['/workspace.js', ['workspace.js', 'text/javascript; charset=utf-8']],
  ['/connections.js', ['connections.js', 'text/javascript; charset=utf-8']],
  ['/experience.js', ['experience.js', 'text/javascript; charset=utf-8']],
  ['/experience.css', ['experience.css', 'text/css; charset=utf-8']],
  ['/shell.css', ['shell.css', 'text/css; charset=utf-8']],
  ['/docking.js', ['docking.js', 'text/javascript; charset=utf-8']],
  ['/settings.js', ['settings.js', 'text/javascript; charset=utf-8']],
  ['/settings.css', ['settings.css', 'text/css; charset=utf-8']],
  ['/controls.css', ['controls.css', 'text/css; charset=utf-8']],
  ['/discovery-client.js', ['discovery-client.js', 'text/javascript; charset=utf-8']],
  ['/resource-view.js', ['resource-view.js', 'text/javascript; charset=utf-8']],
  ['/resources.css', ['resources.css', 'text/css; charset=utf-8']],
  ['/connection-diagnostics.js', ['connection-diagnostics.js', 'text/javascript; charset=utf-8']],
  ['/diagnostics-view.js', ['diagnostics-view.js', 'text/javascript; charset=utf-8']],
  ['/diagnostics.css', ['diagnostics.css', 'text/css; charset=utf-8']],
  ['/help.html', ['help.html', 'text/html; charset=utf-8']],
  ['/help.css', ['help.css', 'text/css; charset=utf-8']],
  ['/help.js', ['help.js', 'text/javascript; charset=utf-8']],
  ...['session-tree','workspace-docking','settings-window','account-usage','review-path'].map(name=>[`/assets/help/${name}.svg`,[`assets/help/${name}.svg`,'image/svg+xml']]),
  ['/website.html', ['website.html', 'text/html; charset=utf-8']],
  ['/website.css', ['website.css', 'text/css; charset=utf-8']],
  ['/website.js', ['website.js', 'text/javascript; charset=utf-8']],
  ['/watch.html', ['watch.html', 'text/html; charset=utf-8']],
  ['/watch.js', ['watch.js', 'text/javascript; charset=utf-8']],
  ['/video-config.json', ['video-config.json', 'application/json; charset=utf-8']],
  ['/assets/icons/github-mark-white.svg', ['assets/icons/github-mark-white.svg', 'image/svg+xml']],
  ['/assets/visuals/night-flight.png', ['assets/visuals/night-flight.png', 'image/png']],
  ['/assets/brand/favicon-transparent.png', ['assets/brand/favicon-transparent.png', 'image/png']],
  ...['symbol-indigo','symbol-white','symbol-graphite'].map(name=>[`/assets/brand/transparent/${name}.png`,[`assets/brand/transparent/${name}.png`,'image/png']]),
  ['/assets/brand/transparent/index.html', ['assets/brand/transparent/index.html', 'text/html; charset=utf-8']],
  ...['main-logo-light','main-logo-dark','symbol-light','symbol-dark','app-icon-light','app-icon-dark','orchestration-banner'].map(name=>[`/assets/brand/${name}.png`,[`assets/brand/${name}.png`,'image/png']]),
]);
const server = createServer(async (req, res) => {
  const pathname = new URL(req.url, 'http://127.0.0.1').pathname;
  if(await discoveryHandler(req,res,pathname))return;
  const videoMatch = pathname.match(/^\/assets\/video\/[A-Za-z0-9_-]+\.(mp4|webm|vtt)$/);
  const file = files.get(pathname) || (videoMatch ? [pathname.slice(1), {mp4:'video/mp4', webm:'video/webm', vtt:'text/vtt; charset=utf-8'}[videoMatch[1]]] : null);
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

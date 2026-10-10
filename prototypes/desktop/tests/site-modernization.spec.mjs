import {test, expect} from '@playwright/test';

const site = '/website.html';
const scenario = (page, value) => page.locator(`button[data-scenario="${value}"]`);
const stage = page => page.locator('#product-demo');

test.beforeEach(async ({page}) => {
  await page.goto(site);
});

test('cada cenário muda o pedido e começa uma demonstração própria', async ({page}) => {
  const request = page.locator('#demo-request');
  const requests = new Set();
  for (const choice of ['fix', 'create', 'understand']) {
    await scenario(page, choice).click();
    await expect(scenario(page, choice)).toHaveAttribute('aria-pressed', 'true');
    await expect(stage(page)).toHaveAttribute('data-stage', '0');
    const text = (await request.innerText()).trim();
    expect(text.length).toBeGreaterThan(15);
    requests.add(text);
    for (const other of ['fix', 'create', 'understand'].filter(value => value !== choice)) {
      await expect(scenario(page, other)).toHaveAttribute('aria-pressed', 'false');
    }
    await page.locator('#demo-next').click();
    await expect(stage(page)).toHaveAttribute('data-stage', '1');
  }
  expect(requests.size).toBe(3);
});

test('fluxo manual chega à revisão, volta e reinicia sem executar agentes', async ({page}) => {
  const external = [];
  const origin = new URL(test.info().project.use.baseURL).origin;
  page.on('request', request => {
    if (new URL(request.url()).origin !== origin) external.push(request.url());
  });
  await expect(page.locator('#demo-prev')).toBeDisabled();
  for (let step = 1; step <= 3; step += 1) {
    await page.locator('#demo-next').click();
    await expect(stage(page)).toHaveAttribute('data-stage', String(step));
    await expect(page.locator('#demo-live')).not.toHaveText('');
  }
  await expect(page.locator('#demo-diff')).toBeVisible();
  expect((await page.locator('#demo-diff').innerText()).trim().length).toBeGreaterThan(15);
  await expect(page.locator('#demo-next')).toBeDisabled();
  await page.locator('#demo-prev').click();
  await expect(stage(page)).toHaveAttribute('data-stage', '2');
  await page.locator('#demo-restart').click();
  await expect(stage(page)).toHaveAttribute('data-stage', '0');
  await expect(page.locator('#demo-prev')).toBeDisabled();
  expect(external).toEqual([]);
});

test('controles têm nomes acessíveis e avanço funciona pelo teclado', async ({page}) => {
  for (const selector of ['#demo-next', '#demo-prev', '#demo-restart', '#demo-play']) {
    await expect(page.locator(selector)).toHaveAccessibleName(/\S/);
  }
  await expect(page.locator('#demo-live')).toHaveAttribute('aria-live', 'polite');
  const next = page.locator('#demo-next');
  await next.focus();
  await expect(next).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(stage(page)).toHaveAttribute('data-stage', '1');
  await expect(next).toBeFocused();
  const create = scenario(page, 'create');
  await create.focus();
  await page.keyboard.press('Space');
  await expect(create).toHaveAttribute('aria-pressed', 'true');
  await expect(stage(page)).toHaveAttribute('data-stage', '0');
});

test('a demonstração só avança sozinha quando a pessoa pede e pode ser pausada', async ({page}) => {
  await page.clock.install();
  await page.reload();
  await stage(page).evaluate(element => element.scrollIntoView({behavior: 'instant', block: 'center'}));
  await expect(stage(page)).not.toHaveClass(/is-offscreen/);
  await page.clock.fastForward(15000);
  await expect(stage(page)).toHaveAttribute('data-stage', '0');
  await page.locator('#demo-play').click();
  await page.locator('#demo-play').click();
  await page.clock.fastForward(15000);
  await expect(stage(page)).toHaveAttribute('data-stage', '0');
  await page.locator('#demo-play').click();
  for (let step = 1; step <= 3; step += 1) {
    await page.clock.fastForward(4500);
    await expect(stage(page)).toHaveAttribute('data-stage', String(step));
  }
  await expect(page.locator('#demo-play')).toHaveAttribute('aria-pressed', 'false');
  await page.locator('#demo-restart').click();
  await page.clock.fastForward(15000);
  await expect(stage(page)).toHaveAttribute('data-stage', '0');
});

test('reprodução suspende fora da tela e retoma quando o fluxo volta a aparecer', async ({page}) => {
  await page.clock.install();
  await page.locator('#demo-play').click();
  await expect(stage(page)).not.toHaveClass(/is-offscreen/);
  await page.locator('#download').evaluate(element => element.scrollIntoView({behavior: 'instant', block: 'start'}));
  await expect(stage(page)).toHaveClass(/is-offscreen/);
  await page.clock.fastForward(15000);
  await expect(stage(page)).toHaveAttribute('data-stage', '0');
  await expect(page.locator('#demo-play')).toHaveAttribute('aria-pressed', 'true');
  await stage(page).evaluate(element => element.scrollIntoView({behavior: 'instant', block: 'start'}));
  await expect(stage(page)).not.toHaveClass(/is-offscreen/);
  for (let step = 1; step <= 3; step += 1) {
    await page.clock.fastForward(4500);
    await expect(stage(page)).toHaveAttribute('data-stage', String(step));
  }
  await expect(page.locator('#demo-play')).toHaveAttribute('aria-pressed', 'false');
});

test('Studio revela detalhes da mesma experiência e permite voltar ao chat', async ({page}) => {
  const preview = page.locator('#experience-preview');
  const inspector = page.locator('#studio-inspector');
  const description = page.locator('#experience-description');
  await expect(preview).toHaveAttribute('data-mode', 'chat');
  await expect(inspector).toBeHidden();
  const chatDescription = await description.innerText();
  await page.locator('[data-experience="studio"]').click();
  await expect(preview).toHaveAttribute('data-mode', 'studio');
  await expect(page.locator('[data-experience="studio"]')).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('[data-experience="chat"]')).toHaveAttribute('aria-pressed', 'false');
  await expect(inspector).toBeVisible();
  await expect(description).not.toHaveText(chatDescription);
  await page.locator('[data-experience="chat"]').click();
  await expect(preview).toHaveAttribute('data-mode', 'chat');
  await expect(inspector).toBeHidden();
  await expect(description).toHaveText(chatDescription);
});

test('preferência de movimento reduzido conserva o fluxo manual', async ({page}) => {
  await page.emulateMedia({reducedMotion: 'reduce'});
  await page.reload();
  await expect(page.locator('body')).toHaveAttribute('data-motion', 'reduced');
  await expect(page.locator('#demo-play')).toBeDisabled();
  await expect(page.locator('#motion-toggle')).toHaveCount(0);
  await page.locator('#demo-next').click();
  await expect(stage(page)).toHaveAttribute('data-stage', '1');
  await page.locator('[data-experience="studio"]').click();
  await expect(page.locator('#studio-inspector')).toBeVisible();
});

test('site se adapta a seis larguras e direciona o visitante para os downloads', async ({page}) => {
  const errors = [];
  const failed = [];
  const httpErrors = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('requestfailed', request => failed.push(request.url()));
  page.on('response', response => {
    if (response.status() >= 400) httpErrors.push({url: response.url(), status: response.status()});
  });
  await page.reload();
  for (const width of [320, 390, 720, 1024, 1440, 1920]) {
    await page.setViewportSize({width, height: 900});
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `reflow ${width}px`).toBe(true);
    await expect(page.getByRole('heading', {level: 1})).toBeVisible();
  }
  for (const platform of ['Windows', 'Linux', 'macOS']) {
    await expect(page.getByText(platform, {exact: true})).toBeVisible();
  }
  await expect(page.getByText('Coming soon', {exact: true})).toHaveCount(3);
  await expect(page.locator('a[download]')).toHaveCount(0);
  expect(await page.locator('a[href]').evaluateAll(links => links.map(link => link.getAttribute('href')).filter(href => /\.(?:exe|msi|dmg|deb|rpm|appimage)(?:[?#]|$)/i.test(href)))).toEqual([]);
  await expect(page.locator('a[href="index.html"]')).toHaveCount(0);
  await expect(page.locator('#motion-toggle')).toHaveCount(0);
  await page.getByRole('link', {name: /^Download Orchestrix/}).first().click();
  await expect(page).toHaveURL(/\/website\.html#download$/);
  await expect(page.locator('#download')).toBeInViewport();
  expect(errors).toEqual([]);
  expect(failed).toEqual([]);
  expect(httpErrors).toEqual([]);
});

test('links do GitHub usam a imagem local e os modos ampliam a apresentação da solução', async ({page}) => {
  await expect(page).toHaveTitle('Orchestrix');
  const htmlPages = await Promise.all(['/website.html', '/watch.html', '/index.html'].map(async path => {
    const response = await page.request.get(path);
    return {path, status: response.status(), html: await response.text()};
  }));
  for (const {path, status, html} of htmlPages) {
    expect(status, path).toBe(200);
    expect(html, `${path}: título da aba`).toMatch(/<title>\s*Orchestrix\s*<\/title>/i);
  }
  const favicon = page.locator('link[rel="icon"]');
  await expect(favicon).toHaveAttribute('href', 'assets/brand/transparent/symbol-indigo.png');
  await expect(favicon).toHaveAttribute('type', 'image/png');
  const faviconPixels = await favicon.evaluate(async link => {
    const image = new Image();
    image.src = link.href;
    await image.decode();
    const canvas = document.createElement('canvas');
    canvas.width = image.naturalWidth;
    canvas.height = image.naturalHeight;
    const context = canvas.getContext('2d');
    context.drawImage(image, 0, 0);
    const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
    const alphaAt = (x, y) => pixels[(y * canvas.width + x) * 4 + 3];
    let left = canvas.width, right = -1, top = canvas.height, bottom = -1;
    for (let y = 0; y < canvas.height; y += 1) {
      for (let x = 0; x < canvas.width; x += 1) {
        if (alphaAt(x, y) < 8) continue;
        left = Math.min(left, x);
        right = Math.max(right, x);
        top = Math.min(top, y);
        bottom = Math.max(bottom, y);
      }
    }
    return {
      corners: [[0, 0], [canvas.width - 1, 0], [0, canvas.height - 1], [canvas.width - 1, canvas.height - 1]].map(([x, y]) => alphaAt(x, y)),
      hole: alphaAt(Math.floor(canvas.width * 0.5), Math.floor(canvas.height * 0.32)),
      widthRatio: (right - left + 1) / canvas.width,
      heightRatio: (bottom - top + 1) / canvas.height
    };
  });
  expect(faviconPixels.corners).toEqual([0, 0, 0, 0]);
  expect(faviconPixels.hole, 'abertura interna sem disco opaco').toBeLessThanOrEqual(5);
  expect(faviconPixels.widthRatio, 'símbolo ocupa a largura útil do favicon').toBeGreaterThanOrEqual(0.85);
  expect(faviconPixels.heightRatio, 'símbolo ocupa a altura útil do favicon').toBeGreaterThanOrEqual(0.80);
  for (const selector of ['.site-header .brand-symbol', '.hero-emblem']) {
    const logo = page.locator(selector);
    await expect(logo.locator('img')).toHaveAttribute('src', 'assets/brand/transparent/symbol-indigo.png');
    const appearance = await logo.evaluate(container => {
      const image = container.querySelector('img');
      const outerStyle = getComputedStyle(container);
      const imageStyle = getComputedStyle(image);
      const outerRect = container.getBoundingClientRect();
      const imageRect = image.getBoundingClientRect();
      return {
        background: outerStyle.backgroundColor,
        radius: outerStyle.borderRadius,
        clip: outerStyle.clipPath,
        outerBlend: outerStyle.mixBlendMode,
        imageBlend: imageStyle.mixBlendMode,
        imageInside: imageRect.left >= outerRect.left - 1 && imageRect.top >= outerRect.top - 1 && imageRect.right <= outerRect.right + 1 && imageRect.bottom <= outerRect.bottom + 1
      };
    });
    expect(appearance.background, `${selector}: sem fundo opaco`).toBe('rgba(0, 0, 0, 0)');
    expect(appearance.radius, `${selector}: sem moldura circular`).toBe('0px');
    expect(appearance.clip, `${selector}: sem recorte da imagem`).toBe('none');
    expect(appearance.outerBlend).toBe('normal');
    expect(appearance.imageBlend).toBe('normal');
    expect(appearance.imageInside, `${selector}: imagem cabe sem ampliar e cortar`).toBe(true);
  }
  const githubLinks = page.locator('a[href="https://github.com/LeonardoMBarca/orchestrix"]');
  expect(await githubLinks.count()).toBeGreaterThanOrEqual(3);
  for (const link of await githubLinks.all()) {
    const mark = link.locator('img[src="assets/icons/github-mark-white.svg"]');
    await expect(mark).toHaveCount(1);
    await link.scrollIntoViewIfNeeded();
    await expect.poll(() => mark.evaluate(image => image.complete && image.naturalWidth > 0)).toBe(true);
  }
  await expect(page.locator('#capabilities')).toBeVisible();
  const modes = page.locator('#capabilities article');
  await expect(modes).toHaveCount(6);
  const modeNames = await modes.locator('h3').allTextContents();
  expect(new Set(modeNames.map(name => name.trim())).size).toBe(6);
  await expect(page.locator('#agent-profiles')).toBeVisible();
  await expect(page.locator('body')).not.toContainText(/Papéis ilustrativos|Nenhum agente está sendo executado|O protótipo usa dados simulados|Contas e execuções são simuladas|Para conversar no protótipo/i);
});

async function measureHeroConnections(page) {
  return page.locator('#hero-network').evaluate(svg => {
    return ['request', 'context', 'code', 'review'].map(key => {
      const path = svg.querySelector(`[data-orbit-connector="${key}"]`);
      const dot = svg.querySelector(`[data-orbit-dot="${key}"]`);
      const node = document.querySelector(`[data-orbit-node="${key}"]`);
      const localPoint = path.getPointAtLength(path.getTotalLength());
      const endpoint = new DOMPoint(localPoint.x, localPoint.y).matrixTransform(path.getScreenCTM());
      const dotPoint = new DOMPoint(dot.cx.baseVal.value, dot.cy.baseVal.value).matrixTransform(dot.getScreenCTM());
      const rect = node.getBoundingClientRect();
      const edgeX = key === 'request' || key === 'code' ? rect.right : rect.left;
      const edgeY = rect.top + rect.height / 2;
      return {
        key,
        top: rect.top,
        edgeGap: Math.hypot(endpoint.x - edgeX, endpoint.y - edgeY),
        dotGap: Math.hypot(endpoint.x - dotPoint.x, endpoint.y - dotPoint.y)
      };
    });
  });
}

test('conectores do hero permanecem presos à lateral dos blocos ao mover e redimensionar', async ({page}) => {
  await page.clock.install();
  await page.reload();
  for (const width of [1440, 390]) {
    await page.setViewportSize({width, height: 960});
    await page.locator('.hero-visual').scrollIntoViewIfNeeded();
    await page.clock.runFor(200);
    const before = await measureHeroConnections(page);
    await page.clock.runFor(1900);
    const after = await measureHeroConnections(page);
    for (const connection of [...before, ...after]) {
      expect(connection.edgeGap, `${width}px ${connection.key}: ponta junto da borda`).toBeLessThan(0.8);
      expect(connection.dotGap, `${width}px ${connection.key}: ponto junto da linha`).toBeLessThan(0.8);
    }
    expect(after.some((connection, index) => Math.abs(connection.top - before[index].top) > 0.1), `movimento ${width}px`).toBe(true);
  }
});

test('página do vídeo mantém uma abertura útil antes da publicação do filme', async ({page}) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const configuration = page.waitForResponse(response => new URL(response.url()).pathname === '/video-config.json');
  await page.locator('a[href="watch.html"]').first().click();
  expect((await configuration).status()).toBe(200);
  await expect(page).toHaveURL(/\/watch\.html$/);
  await expect(page.getByRole('heading', {level: 1})).toBeVisible();
  await expect(page.locator('#video-placeholder')).toBeVisible();
  await expect(page.locator('#product-video')).toBeHidden();
  await expect(page.locator('#video-transcript')).toBeHidden();
  await expect(page.locator('#product-video')).not.toHaveAttribute('src', /\S/);
  await expect(page.locator('a[href="website.html#download"]')).toBeVisible();
  expect(errors).toEqual([]);
});

test('configuração do filme ativa player, legendas e transcrição sem reprodução automática', async ({page}) => {
  const mediaRequests = [];
  page.on('request', request => {
    if (request.url().endsWith('/assets/video/example.mp4')) mediaRequests.push(request.url());
  });
  await page.route('**/video-config.json', route => route.fulfill({
    contentType: 'application/json',
    body: JSON.stringify({
      src: 'assets/video/example.mp4',
      poster: 'assets/visuals/night-flight.png',
      captions: 'assets/video/example.vtt',
      transcript: 'Uma conversa reúne contexto, agentes e uma revisão do resultado.'
    })
  }));
  await page.route('**/assets/video/example.vtt', route => route.fulfill({contentType: 'text/vtt', body: 'WEBVTT\n\n00:00.000 --> 00:01.000\nOrchestrix em ação.\n'}));
  await page.goto('/watch.html');
  const video = page.locator('#product-video');
  await expect(video).toBeVisible();
  await expect(page.locator('#video-placeholder')).toBeHidden();
  await expect(video).toHaveAttribute('src', /\/assets\/video\/example\.mp4$/);
  await expect(video).toHaveAttribute('preload', 'none');
  await expect(video).toHaveJSProperty('controls', true);
  await expect(video).toHaveJSProperty('autoplay', false);
  await expect(video).toHaveJSProperty('paused', true);
  await expect(video.locator('track[kind="captions"][srclang="en"]')).toHaveAttribute('src', /\/assets\/video\/example\.vtt$/);
  await expect(video.locator('track[kind="captions"]')).toHaveAttribute('label', 'English');
  await expect(page.locator('#video-transcript')).toBeVisible();
  await page.locator('#video-transcript summary').click();
  await expect(page.locator('#video-transcript p')).toHaveText('Uma conversa reúne contexto, agentes e uma revisão do resultado.');
  expect(mediaRequests).toEqual([]);
});

test('fonte de vídeo inválida conserva a abertura sem criar um player sem conteúdo', async ({page}) => {
  await page.route('**/video-config.json', route => route.fulfill({contentType: 'application/json', body: JSON.stringify({src: 'javascript:void(0)'})}));
  const configuration = page.waitForResponse(response => new URL(response.url()).pathname === '/video-config.json');
  await page.goto('/watch.html');
  await (await configuration).finished();
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  await expect(page.locator('#video-placeholder')).toBeVisible();
  await expect(page.locator('#product-video')).toBeHidden();
  await expect(page.locator('#product-video')).not.toHaveAttribute('src', /\S/);
});

test.describe('conteúdo sem JavaScript', () => {
  test.use({javaScriptEnabled: false});

  test('marca, explicação, plataformas e página de vídeo continuam disponíveis', async ({page}) => {
    await expect(page.getByRole('heading', {level: 1})).toBeVisible();
    await expect(page.getByText('Windows', {exact: true})).toBeVisible();
    await expect(page.getByText('Coming soon', {exact: true})).toHaveCount(3);
    const video = page.locator('a[href="watch.html"]').first();
    await expect(video).toHaveAccessibleName(/\S/);
    await video.click();
    await expect(page).toHaveURL(/\/watch\.html$/);
    await expect(page.getByRole('heading', {level: 1})).toBeVisible();
  });
});

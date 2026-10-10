import {test, expect} from '@playwright/test';

const helpURL = new URL('../help.html', import.meta.url).href;
const siteURL = new URL('../website.html', import.meta.url).href;

test('help works directly from HTML, with complete guides and local illustrations', async ({page}) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(helpURL);
  await expect(page).toHaveTitle('Help — Orchestrix');
  await expect(page.locator('[data-guide]')).toHaveCount(9);
  await expect(page.getByRole('status')).toHaveText('9 guides to help you get started');
  const illustrations = page.locator('.guide figure img');
  await expect(illustrations).toHaveCount(5);
  for (const illustration of await illustrations.all()) {
    await illustration.scrollIntoViewIfNeeded();
    await expect.poll(() => illustration.evaluate(image => image.complete && image.naturalWidth > 0)).toBe(true);
    await expect(illustration).toHaveAttribute('alt', /\S+/);
  }
  await expect(page.locator('.video-guide:visible')).toHaveCount(0);
  await expect(page.locator('video')).toHaveCount(0);
  await expect(page.locator('#accounts')).toContainText('Not available');
  await expect(page.locator('#accounts')).toContainText('Subscription Only');
  expect(errors).toEqual([]);
});

test('help search filters actual content, supports empty recovery, and clears before topic navigation', async ({page}) => {
  await page.goto(helpURL);
  const search = page.getByRole('searchbox', {name:'Search guides'});
  await search.fill('subscription only');
  await expect(page.locator('#accounts')).toBeVisible();
  await expect(page.locator('#layout')).toBeHidden();
  await expect(page.getByRole('status')).toHaveText('1 guide matches your search');
  await search.fill('unfindable-guide-zxcv');
  await expect(page.locator('[data-guide]:visible')).toHaveCount(0);
  await expect(page.locator('#empty-search')).toBeVisible();
  await page.getByRole('button', {name:'Clear', exact:true}).click();
  await expect(search).toBeFocused();
  await expect(page.locator('[data-guide]:visible')).toHaveCount(9);
  await search.fill('subscription only');
  await page.locator('.help-nav').getByRole('link', {name:'Arrange your workspace', exact:true}).click();
  await expect(search).toHaveValue('');
  await expect(page.locator('#layout')).toBeVisible();
  await expect(page).toHaveURL(/#layout$/);
  await expect(page.locator('.help-nav a[href="#layout"]')).toHaveAttribute('aria-current','location');
});

test('help reflows on compact screens without losing search or tutorial text', async ({page}) => {
  for (const width of [390,320]) {
    await page.setViewportSize({width,height:900});
    await page.goto(helpURL);
    await expect(page.getByRole('searchbox')).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    await page.locator('#review').scrollIntoViewIfNeeded();
    await expect(page.locator('#review h2')).toBeVisible();
    await expect(page.locator('#review')).toContainText('Validating a result and applying it are separate decisions.');
  }
});

test('public marketing uses product language while unreleased downloads remain accurate', async ({page}) => {
  await page.goto(siteURL);
  await expect(page.locator('.hero-copy .eyebrow')).not.toContainText(/local|development|demo/i);
  await expect(page.locator('body')).not.toContainText(/local-first|example context|the demo is complete/i);
  await expect(page.locator('.release-state')).toHaveCount(3);
  await expect(page.locator('.release-state').first()).toHaveText('Coming soon');
  await expect(page.getByRole('group', {name:'Choose a request',exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Create a feature',exact:true}).click();
  await expect(page.locator('#demo-stage-copy')).not.toContainText('example');
});

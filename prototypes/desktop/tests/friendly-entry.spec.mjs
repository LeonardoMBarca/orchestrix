import {test,expect} from '@playwright/test';
import {openSettings} from './navigation.mjs';

test.beforeEach(async ({page})=>{await page.goto('/');});

test('entry keeps the brand, starter cards, suggestions and composer in a clear sequence',async ({page})=>{
  await expect(page.getByRole('heading',{level:1})).toHaveText('What shall we create today?');
  await expect(page.locator('.welcome-symbol img:visible')).toBeVisible();
  await expect(page.locator('.quick-start button')).toHaveCount(2);
  await expect(page.locator('.quick-start a[target="_blank"]')).toHaveCount(1);
  const ordered=await page.evaluate(()=>['.welcome-stage','.quick-start','.prompt-suggestions','#chat-form'].map(s=>document.querySelector(s).getBoundingClientRect().top));
  expect(ordered).toEqual([...ordered].sort((a,b)=>a-b));
  await expect(page.locator('.welcome-stage')).not.toContainText(/An idea|simulated|demo/i);
  await expect(page.locator('#chat-project,#chat-session,.project-switch,#interface-language')).toHaveCount(0);
  await expect(page.locator('#studio-navigation')).toHaveJSProperty('open',false);
  await expect(page.locator('.chat-task-card')).toHaveCount(0);
});

test('suggestions prepare a request without sending or starting work',async ({page})=>{
  await page.locator('[data-prompt]').first().click();
  await expect(page.locator('#chat-message')).toHaveValue(/\S/);
  await expect(page.locator('#chat-message')).toBeFocused();
  expect(await page.evaluate(()=>OrchestrixApp.getState().tasks.length)).toBe(0);
  expect(await page.evaluate(()=>OrchestrixApp.getState().chat.messages.length)).toBe(0);
});

test('compact entry fits common laptop viewports without scrolling in all interface languages',async ({page})=>{
  for(const language of ['en','pt-BR','es']){
    await page.evaluate(lang=>OrchestrixI18n.setLanguage(lang),language);
    for(const viewport of [{width:1440,height:900},{width:1280,height:720},{width:1024,height:768}]){
      await page.setViewportSize(viewport);
      const metrics=await page.evaluate(()=>{
        const content=document.querySelector('.main>.content'),send=document.querySelector('#chat-form button[type="submit"]');
        return {scroll:content.scrollHeight,height:content.clientHeight,documentHeight:document.documentElement.scrollHeight,viewport:innerHeight,sendBottom:send.getBoundingClientRect().bottom,contentBottom:content.getBoundingClientRect().bottom};
      });
      expect(metrics.scroll,`${language} ${viewport.width}: content fits`).toBeLessThanOrEqual(metrics.height+1);
      expect(metrics.documentHeight).toBeLessThanOrEqual(metrics.viewport+1);
      expect(metrics.sendBottom).toBeLessThanOrEqual(metrics.contentBottom);
    }
  }
});

test('resizing and docking preserve the composer node, draft, cursor and focus',async ({page})=>{
  const input=page.locator('#chat-message'),draft='Keep <b>this</b> draft / português / español.';
  await input.fill(draft);await input.evaluate(e=>e.setSelectionRange(7,21,'backward'));
  const original=await input.elementHandle();
  for(const viewport of [{width:390,height:844},{width:1440,height:960},{width:320,height:900}]){
    await page.setViewportSize(viewport);
    await expect(input).toBeFocused();await expect(input).toHaveValue(draft);
    expect(await input.evaluate((e,old)=>e===old,original)).toBe(true);
    expect(await input.evaluate(e=>[e.selectionStart,e.selectionEnd,e.selectionDirection])).toEqual([7,21,'backward']);
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  }
  await page.evaluate(()=>OrchestrixDock.move('sessions','right'));
  expect(await input.evaluate((e,old)=>e===old,original)).toBe(true);await expect(input).toHaveValue(draft);
});

for(const language of ['en','pt-BR','es'])test(`${language}: essentials reflow and remain available with large text`,async ({page})=>{
  await openSettings(page);await page.locator('#settings-language').selectOption(language);await page.evaluate(()=>OrchestrixSettings.close());
  for(const width of [320,390,1024,1440]){
    await page.setViewportSize({width,height:900});await page.evaluate(()=>document.documentElement.dataset.textSize='large');
    await expect(page.locator('#chat-message')).toBeVisible();await expect(page.locator('#command-trigger')).toBeVisible();
    await expect(page.locator('#help-link')).toBeVisible();await expect(page.locator('.app-settings-trigger')).toBeVisible();
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
    expect(await page.evaluate(()=>OrchestrixI18n.missing())).toEqual([]);
  }
});

test('help opens in the browser and account registration begins with billing source without provider requests',async ({page})=>{
  await expect(page.locator('#help-link')).toHaveAttribute('href','help.html');await expect(page.locator('#help-link')).toHaveAttribute('target','_blank');
  const external=[];page.on('request',r=>{if(new URL(r.url()).origin!==new URL(page.url()).origin)external.push(r.url());});
  await page.locator('[data-action="connect-account"]').click();
  await expect(page.getByRole('dialog')).toBeVisible();await expect(page.getByRole('dialog').locator('input[type="password"]')).toHaveCount(0);
  await expect(page.getByRole('dialog').locator('input[type="email"]')).toHaveCount(0);
  await page.keyboard.press('Escape');await expect(page.getByRole('dialog')).toBeHidden();expect(external).toEqual([]);
});

test('search has a usable desktop width and opens the command palette by mouse and keyboard',async ({page})=>{
  await page.setViewportSize({width:1440,height:1000});
  expect(await page.locator('#command-trigger').evaluate(e=>e.getBoundingClientRect().width)).toBeGreaterThan(280);
  await page.locator('#command-trigger').click();await expect(page.locator('#command-search')).toBeFocused();
  await page.keyboard.press('Escape');await page.keyboard.press('Control+k');await expect(page.locator('#command-search')).toBeFocused();
});

test('website directs to forthcoming downloads without fake installers or broken assets',async ({page})=>{
  const errors=[],failed=[];page.on('pageerror',e=>errors.push(e.message));page.on('requestfailed',r=>failed.push(r.url()));
  await page.goto('/website.html');
  for(const platform of ['Windows','Linux','macOS'])await expect(page.getByText(platform,{exact:true})).toBeVisible();
  await expect(page.getByText('Coming soon',{exact:true})).toHaveCount(3);await expect(page.locator('a[download]')).toHaveCount(0);
  for(const width of [320,390,720,1024,1440,1920]){
    await page.setViewportSize({width,height:900});expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  }
  await page.getByRole('link',{name:/^Download Orchestrix/}).first().click();await expect(page).toHaveURL(/website\.html#download$/);
  expect(errors).toEqual([]);expect(failed).toEqual([]);
});

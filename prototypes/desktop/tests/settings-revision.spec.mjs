import {test,expect} from '@playwright/test';
import {openSettings} from './navigation.mjs';

const settings=page=>page.locator('#floating-settings');
const close=page=>settings(page).locator('[data-settings-close]').click();
test.beforeEach(async ({page})=>{await page.goto('/');});

test('Settings closes on an outside press without swallowing chat interaction or losing drafts',async ({page})=>{
  const composer=page.locator('#chat-message');
  await composer.fill('A conversation that remains open.');
  await openSettings(page);
  await page.locator('#settings-profile-name').fill('Unsaved profile');
  await expect(settings(page)).toHaveAttribute('aria-modal','false');
  await expect(page.locator('#content')).not.toHaveAttribute('inert','');
  await page.setViewportSize({width:1600,height:1000});
  await settings(page).locator('.settings-drag-handle').focus();
  for(let index=0;index<16;index++)await page.keyboard.press('Shift+ArrowLeft');
  await composer.focus();
  await composer.press('End');
  await page.keyboard.type(' Still writing.');
  await expect(composer).toHaveValue('A conversation that remains open. Still writing.');
  const bounds=await composer.boundingBox();
  await composer.click({position:{x:bounds.width-14,y:12}});
  await expect(settings(page)).toBeHidden();await expect(composer).toBeFocused();
  await expect(composer).toHaveValue('A conversation that remains open. Still writing.');
  await openSettings(page);await expect(page.locator('#settings-profile-name')).toHaveValue('Unsaved profile');
  await settings(page).locator('[data-settings-close]').focus();
  await page.keyboard.press('Escape');
  await expect(settings(page)).toBeHidden();
  await expect(composer).toHaveValue('A conversation that remains open. Still writing.');
});

test('Settings reopens centered at a larger size after being dragged',async ({page})=>{
  await page.setViewportSize({width:1440,height:1000});await openSettings(page);
  const first=await settings(page).boundingBox();
  expect(first.width).toBe(940);expect(first.height).toBe(720);
  expect(first.x).toBe(250);expect(first.y).toBe(140);
  await settings(page).locator('.settings-drag-handle').focus();await page.keyboard.press('Shift+ArrowLeft');await page.keyboard.press('Shift+ArrowDown');
  const moved=await settings(page).boundingBox();expect(moved.x).not.toBe(first.x);expect(moved.y).not.toBe(first.y);
  await close(page);await openSettings(page);expect(await settings(page).boundingBox()).toEqual(first);
});

test('child account dialog clicks do not dismiss Settings',async ({page})=>{
  await openSettings(page,'accounts');await settings(page).locator('[data-settings-add-account]').click();
  await expect(page.locator('#modal')).toBeVisible();
  await page.locator('#modal-title').click();
  await expect(settings(page)).toBeVisible();
  await page.keyboard.press('Escape');await expect(page.locator('#modal')).toBeHidden();await expect(settings(page)).toBeVisible();
});

test('text slider applies intermediate scales live, persists and reflows without losing drafts',async ({page})=>{
  const composer=page.locator('#chat-message');await composer.fill('Keep this draft during scaling.');
  await openSettings(page);await expect(page.locator('html')).toHaveAttribute('data-density','compact');
  const slider=page.locator('#settings-text-size');await expect(slider).toHaveAttribute('type','range');
  await slider.focus();await slider.press('ArrowRight');
  await expect(slider).toHaveValue('105');await expect(page.locator('#settings-text-size-value')).toHaveText('105%');
  expect(await page.evaluate(()=>getComputedStyle(document.documentElement).fontSize)).toBe('14.7px');
  await slider.fill('125');await expect(slider).toHaveAttribute('aria-valuetext','125%');
  expect(await page.evaluate(()=>getComputedStyle(document.documentElement).fontSize)).toBe('17.5px');
  await expect(composer).toHaveValue('Keep this draft during scaling.');
  await close(page);await page.reload();await openSettings(page);await expect(slider).toHaveValue('125');
  for(const scale of [80,150,175,200]){
    await slider.fill(String(scale));await expect(page.locator('html')).toHaveAttribute('data-text-scale',String(scale));
    expect(await page.evaluate(()=>Number.parseFloat(getComputedStyle(document.documentElement).fontSize))).toBeCloseTo(14*scale/100,3);
    for(const viewport of [{width:390,height:844},{width:320,height:720},{width:1440,height:1000}]){
      await page.setViewportSize(viewport);await expect(settings(page).locator('[data-settings-close]')).toBeInViewport();
      expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
    }
  }
  await expect(page.locator('html')).toHaveAttribute('data-text-size','large');
  await settings(page).locator('[data-settings-reset-scale]').click();await expect(slider).toHaveValue('100');
  await expect(page.locator('html')).toHaveAttribute('data-text-size','normal');
});

for(const [legacy,value] of [['large','200'],['normal','100']])test(`legacy text size ${legacy} migrates to the slider`,async ({page})=>{
  await page.evaluate(legacy=>localStorage.setItem('orchestrix-prototype-layout',JSON.stringify({textSize:legacy})),legacy);
  await page.reload();await openSettings(page);await expect(page.locator('#settings-text-size')).toHaveValue(value);
});

test('profile and personal instructions save safely and survive reload',async ({page})=>{
  await openSettings(page);
  const name='Ada <img data-settings-injection src=x onerror="window.injected=true">';
  await page.locator('#settings-profile-name').fill(name);
  await settings(page).getByRole('button',{name:'Save profile',exact:true}).click();
  await settings(page).locator('[data-settings-tab="conversation"]').click();
  const prompt='Prefer concise answers. Preserve literal <b>content</b> & Unicode: português / español.';
  await page.locator('#settings-system-prompt').fill(prompt);
  await settings(page).getByRole('button',{name:'Save instructions',exact:true}).click();
  await expect(page.locator('[data-settings-injection]')).toHaveCount(0);
  expect(await page.evaluate(()=>window.injected)).toBeUndefined();
  expect(await page.evaluate(()=>OrchestrixSettings.getPreferences())).toEqual({profileName:name,systemPrompt:prompt,subscriptionOnly:true});
  expect(await page.evaluate(()=>OrchestrixApp.getSettings().systemPrompt)).toBe(prompt);
  await page.reload();
  await openSettings(page,'conversation');
  await expect(page.locator('#settings-system-prompt')).toHaveValue(prompt);
  await settings(page).locator('[data-settings-tab="general"]').click();
  await expect(page.locator('#settings-profile-name')).toHaveValue(name);
});

test('unsaved personal instructions survive tab changes, language changes and closing',async ({page})=>{
  await openSettings(page,'conversation');
  const prompt='Keep my unsaved instructions and cursor. Não traduzir.';
  await page.locator('#settings-system-prompt').fill(prompt);
  await page.locator('#settings-system-prompt').evaluate(element=>element.setSelectionRange(7,18,'backward'));
  for(const language of ['pt-BR','es','en']){
    await page.evaluate(language=>OrchestrixI18n.setLanguage(language),language);
    await expect(page.locator('#settings-system-prompt')).toHaveValue(prompt);
    await expect(page.locator('#settings-system-prompt')).toBeFocused();
    expect(await page.locator('#settings-system-prompt').evaluate(element=>[element.selectionStart,element.selectionEnd,element.selectionDirection])).toEqual([7,18,'backward']);
  }
  await settings(page).locator('[data-settings-tab="themes"]').click();
  await settings(page).locator('[data-settings-tab="conversation"]').click();
  await expect(page.locator('#settings-system-prompt')).toHaveValue(prompt);
  await page.locator('#settings-system-prompt').press('Escape');
  await openSettings(page,'conversation');
  await expect(page.locator('#settings-system-prompt')).toHaveValue(prompt);
  expect(await page.evaluate(()=>OrchestrixSettings.getPreferences().systemPrompt)).toBe('');
});

test('Settings language and theme controls are centralized and translated in all supported languages',async ({page})=>{
  await expect(page.locator('.sidebar [data-language-picker]')).toHaveCount(0);
  await expect(page.locator('#interface-language')).toHaveCount(0);
  await openSettings(page);
  await expect(settings(page).locator('.floating-settings-tabs button svg')).toHaveCount(6);
  for(const [language,name,title] of [['en','Interface language','Orchestrix settings'],['pt-BR','Idioma da interface','Configurações do Orchestrix'],['es','Idioma de la interfaz','Configuración de Orchestrix']]){
    await page.locator('#settings-language').focus();
    await page.locator('#settings-language').selectOption(language);
    await expect(settings(page)).toHaveAccessibleName(title);
    await expect(page.locator('#settings-language')).toHaveAccessibleName(name);
    await expect(page.locator('#settings-language')).toBeFocused();
    for(const tab of ['accounts','themes','conversation','orchestration','layout','general'])await settings(page).locator(`[data-settings-tab="${tab}"]`).click();
    expect(await page.evaluate(()=>OrchestrixI18n.missing())).toEqual([]);
  }
  await settings(page).locator('[data-settings-tab="themes"]').click();
  await expect(settings(page).getByRole('radio')).toHaveCount(8);
  await settings(page).getByRole('radio',{name:'Medieval',exact:true}).check();
  await expect(page.locator('html')).toHaveAttribute('data-direction','medieval');
  await close(page);await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-direction','medieval');
});

test('panel layout offers only side docking with Sessions on the left by default',async ({page})=>{
  await openSettings(page,'layout');
  await expect(page.locator('#settings-dock-sessions')).toHaveValue('left');
  await expect(page.locator('#settings-dock-navigation')).toHaveValue('right');
  await expect(page.locator('#settings-dock-work')).toHaveValue('right');
  for(const id of ['sessions','navigation','work']){
    await expect(page.locator(`#settings-dock-${id} option`)).toHaveCount(2);
    await expect(page.locator(`#settings-dock-${id} option[value="bottom"]`)).toHaveCount(0);
  }
});

test('unknown and synthetic quotas never render a made-up percentage or credits',async ({page})=>{
  await openSettings(page,'accounts');
  await expect(settings(page)).toContainText('No accounts connected');
  await expect(settings(page).locator('progress')).toHaveCount(0);
  await page.evaluate(()=>OrchestrixSettings.configure({getState:()=>({connections:[
    {id:'unknown',name:'Private subscription',provider:'Provider',mode:'own-plan',lifecycle:'pending'},
    {id:'api',name:'API without telemetry',provider:'Provider',mode:'api'},
    {id:'fixture',name:'Unverified record',provider:'Provider',mode:'own-plan',simulated:true,usage:{observed:true,source:'runtime',usedPercent:63}}
  ]})}));
  await expect(settings(page).locator('.settings-account')).toHaveCount(3);
  await expect(settings(page).locator('progress')).toHaveCount(0);
  await expect(settings(page)).not.toContainText('63%');
  await expect(settings(page)).not.toContainText('$');
  await expect(settings(page)).toContainText('Not reported');
  await expect(settings(page)).toContainText('Unavailable');
});

test('trusted provider telemetry shows only reported usage, reset and API credit',async ({page})=>{
  await openSettings(page,'accounts');
  await page.evaluate(()=>OrchestrixSettings.configure({getState:()=>({connections:[
    {id:'observed',name:'Observed subscription',provider:'Provider',mode:'own-plan',lifecycle:'active',authorizationValid:true,usage:{observed:true,source:'provider',usedPercent:32.5,resetAt:'2026-10-17T12:00:00Z'}},
    {id:'observed-api',name:'Observed API',provider:'Provider',mode:'api',usage:{observed:true,source:'runtime',credit:{remaining:12.25,spent:7.75,currency:'USD'}}},
    {id:'invalid',name:'Invalid metadata',provider:'Provider',mode:'api',usage:{observed:true,source:'provider',usedPercent:140,resetAt:'invalid',credit:{remaining:-1,currency:'USD'}}}
  ]})}));
  await expect(settings(page).locator('progress')).toHaveCount(1);
  await expect(settings(page).locator('progress')).toHaveAttribute('value','32.5');
  await expect(settings(page)).toContainText('32.5%');
  await expect(settings(page)).toContainText('$12.25');
  await expect(settings(page)).toContainText('$7.75');
  await expect(settings(page)).not.toContainText('140%');
  await expect(settings(page)).not.toContainText('-$1');
});

test('Settings drag and viewport reflow keep a reachable close control and no horizontal overflow',async ({page})=>{
  await page.setViewportSize({width:1440,height:1000});
  await openSettings(page);
  const handle=settings(page).locator('.settings-drag-handle');
  const before=await settings(page).boundingBox();
  const target=await handle.boundingBox();
  await page.mouse.move(target.x+35,target.y+20);await page.mouse.down();
  await page.mouse.move(target.x-120,target.y+100,{steps:8});await page.mouse.up();
  const after=await settings(page).boundingBox();
  expect(after.x).toBeLessThan(before.x);expect(after.y).toBeGreaterThan(before.y);
  for(const viewport of [{width:390,height:844},{width:320,height:720},{width:1280,height:800}]){
    await page.setViewportSize(viewport);
    await expect(settings(page).locator('[data-settings-close]')).toBeInViewport();
    const bounds=await settings(page).boundingBox();
    expect(bounds.x).toBeGreaterThanOrEqual(7);expect(bounds.y).toBeGreaterThanOrEqual(7);
    expect(bounds.x+bounds.width).toBeLessThanOrEqual(viewport.width-7);
    expect(bounds.y+bounds.height).toBeLessThanOrEqual(viewport.height-7);
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  }
});

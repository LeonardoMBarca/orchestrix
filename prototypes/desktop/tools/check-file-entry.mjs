import assert from 'node:assert/strict';
import {mkdir,writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {chromium,expect} from '@playwright/test';
const entry=new URL('../index.html',import.meta.url).href;
const report={entry,verifiedAt:new Date().toISOString(),layouts:[],journey:[],languages:[]};
const browser=await chromium.launch({channel:'chrome',headless:true});const contexts=[],failures=[],externalRequests=[];
async function open(viewport,language='en'){
  const context=await browser.newContext({viewport});contexts.push(context);
  await context.addInitScript(language=>{try{if(!localStorage.getItem('orchestrix-prototype-language'))localStorage.setItem('orchestrix-prototype-language',language);}catch{}},language);
  const page=await context.newPage();page.on('pageerror',e=>failures.push(e.message));page.on('requestfailed',r=>failures.push(r.url()+': '+r.failure()?.errorText));
  page.on('request',r=>{if(/^https?:/.test(r.url()))externalRequests.push(r.url());});await page.goto(entry);
  await expect(page.locator('html')).toHaveAttribute('lang',language);await expect(page.locator('#chat-message')).toBeVisible();return page;
}
try{
  await mkdir(new URL('../artifacts/',import.meta.url),{recursive:true});
  for(const viewport of [{width:1440,height:900},{width:1280,height:720},{width:1024,height:768},{width:390,height:844},{width:320,height:900}])for(const language of viewport.width===1440?['en']:['en','pt-BR','es']){
    const page=await open(viewport,language);
    const metrics=await page.evaluate(()=>({width:innerWidth,height:innerHeight,scrollWidth:document.documentElement.scrollWidth,language:document.documentElement.lang,missing:OrchestrixI18n.missing(),contentHeight:document.querySelector('.main>.content').clientHeight,contentScrollHeight:document.querySelector('.main>.content').scrollHeight,brokenImages:[...document.images].filter(i=>!i.complete||!i.naturalWidth).map(i=>i.getAttribute('src'))}));
    assert.ok(metrics.scrollWidth<=viewport.width);assert.deepEqual(metrics.missing,[]);assert.deepEqual(metrics.brokenImages,[]);if(viewport.width>800)assert.ok(metrics.contentScrollHeight<=metrics.contentHeight+1,`${language} ${viewport.width}: initial screen needs no scrolling`);report.layouts.push(metrics);
    if((viewport.width===1440&&language==='en')||(viewport.width===390&&language==='en')||(viewport.width===320&&language==='es')){
      await page.screenshot({path:fileURLToPath(new URL(`../artifacts/d1-compact-entry-${viewport.width}-${language}.png`,import.meta.url)),fullPage:true});
    }
    await page.context().close();
  }
  const page=await open({width:1440,height:900});const input=page.locator('#chat-message');
  const first=await page.evaluate(()=>OrchestrixApp.getState().chat.id);await input.fill('Keep my independent draft.');
  await page.locator('.new-session-trigger').click();const second=await page.evaluate(()=>OrchestrixApp.getState().chat.id);assert.notEqual(second,first);
  await input.fill('Plan a scientific investigation.');await page.evaluate(()=>OrchestrixApp.openProject());await page.locator('#entry-name').fill('Scientific notes');await page.locator('#entry-path').fill('C:\\Work\\Papers');
  await page.getByRole('button',{name:'Create project and session',exact:true}).click();
  await page.evaluate(()=>OrchestrixDock.activate('sessions'));await page.locator(`[data-session-id="${second}"]`).click();await expect(input).toHaveValue('Plan a scientific investigation.');
  await page.locator('[data-action="session-attach"]').click();await page.locator('[data-attach-project]').first().click();assert.equal(await page.evaluate(()=>OrchestrixApp.getState().chat.id),second);
  await page.evaluate(()=>{OrchestrixDock.move('navigation','left');OrchestrixDock.activate('sessions');});await page.screenshot({path:fileURLToPath(new URL('../artifacts/d1-compact-tabs-left.png',import.meta.url)),fullPage:true});
  await page.locator('#sessions-panel .dock-panel-handle').click();await expect(page.locator('#dock-position-menu')).toBeVisible();await page.screenshot({path:fileURLToPath(new URL('../artifacts/d1-compact-side-chooser.png',import.meta.url)),fullPage:true});await page.keyboard.press('Escape');await page.evaluate(()=>OrchestrixDock.move('sessions','right'));
  await page.screenshot({path:fileURLToPath(new URL('../artifacts/d1-compact-sessions-right.png',import.meta.url)),fullPage:true});
  await page.evaluate(()=>OrchestrixSettings.open('general'));
  await page.locator('#settings-profile-name').fill('Night pilot');await page.locator('[data-settings-form="profile"]').getByRole('button',{name:'Save profile',exact:true}).click();
  await expect(page.locator('#floating-settings')).toBeVisible();
  await page.screenshot({path:fileURLToPath(new URL('../artifacts/d1-settings-centered-file.png',import.meta.url)),fullPage:true});
  await page.locator('.statusbar').click({position:{x:200,y:14}});await expect(page.locator('#floating-settings')).toBeHidden();await input.fill('My conversation draft survives closing Settings.');
  await page.evaluate(()=>OrchestrixSettings.open('accounts'));await expect(page.locator('.settings-account-list')).toHaveCount(0);await expect(page.locator('progress')).toHaveCount(0);
  await page.screenshot({path:fileURLToPath(new URL('../artifacts/d1-settings-api-empty.png',import.meta.url)),fullPage:true});
  await page.evaluate(()=>OrchestrixSettings.open('general'));await page.locator('#settings-text-size').fill('125');await page.locator('#settings-text-size').dispatchEvent('input');await page.evaluate(()=>{OrchestrixSettings.close();OrchestrixApp.persistSessions();});await page.reload();await expect(input).toHaveValue('My conversation draft survives closing Settings.');assert.equal(await page.evaluate(()=>OrchestrixApp.getUI().textScale),125);
  assert.equal(await page.evaluate(()=>OrchestrixDock.getLayout().positions.sessions),'right');
  report.journey.push('independent-session','isolated-drafts','create-project','attach-existing-project-preserves-session','dock-left-tabs','dock-right','side-choice-blur','centered-settings-outside-dismiss','text-scale-persistence','unknown-usage-not-fabricated','reload-persistence');
  for(const language of ['pt-BR','es','en']){
    await page.evaluate(()=>OrchestrixSettings.open('general'));await page.locator('#settings-language').selectOption(language);await page.evaluate(()=>OrchestrixSettings.close());await page.reload();
    await expect(page.locator('html')).toHaveAttribute('lang',language);assert.deepEqual(await page.evaluate(()=>OrchestrixI18n.missing()),[]);report.languages.push({language,persistedAfterReload:true});
  }
  const help=await page.context().newPage();await help.goto(new URL('../help.html',import.meta.url).href);await expect(help.locator('main')).toBeVisible();
  for(const img of await help.locator('img').all()){await img.scrollIntoViewIfNeeded();await expect(img).toHaveJSProperty('complete',true);assert.ok(await img.evaluate(i=>i.naturalWidth>0));}report.journey.push('external-file-help-with-images');
  assert.deepEqual(failures,[]);assert.deepEqual(externalRequests,[]);report.errors=failures;report.externalRequests=externalRequests;report.result='passed';
  await writeFile(new URL('../artifacts/direct-file-qa.json',import.meta.url),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report,null,2));
}finally{await Promise.allSettled(contexts.map(c=>c.close()));await browser.close();}

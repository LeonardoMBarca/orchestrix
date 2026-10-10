import {test,expect} from '@playwright/test';
import {openSettings,usePortuguese} from './navigation.mjs';

async function assertSelectStyles(page,selector) {
  await page.locator(selector).evaluateAll(async elements=>{
    await Promise.all(elements.flatMap(element=>element.getAnimations().map(animation=>animation.finished.catch(()=>{}))));
  });
  const values=await page.locator(selector).evaluateAll(elements=>elements.map(element=>{
    const style=getComputedStyle(element),root=getComputedStyle(document.documentElement);
    const option=element.querySelector('option'),menu=option&&getComputedStyle(option);
    const probe=document.createElement('span');document.body.append(probe);
    const token=name=>{probe.style.color=`var(${name})`;return getComputedStyle(probe).color;};
    const result={id:element.id,tag:element.tagName,appearance:style.appearance,
      width:element.getBoundingClientRect().width,height:element.getBoundingClientRect().height,
      paddingEnd:parseFloat(style.paddingInlineEnd),fontSize:parseFloat(style.fontSize),
      image:style.backgroundImage,color:style.color,background:style.backgroundColor,
      border:style.borderColor,scheme:style.colorScheme,rootScheme:root.colorScheme,
      focused:element.matches(':focus-visible'),hovered:element.matches(':hover'),
      text:token('--text'),bg:token('--bg'),surface:token('--surface'),accent:token('--accent'),panel:token('--panel'),borderToken:token('--border-control'),
      optionColor:menu?.color,optionBackground:menu?.backgroundColor};
    probe.remove();return result;
  }));
  expect(values.length).toBeGreaterThan(0);
  for(const value of values) {
    expect(value.tag,value.id).toBe('SELECT');
    expect(value.appearance,value.id).toBe('none');
    expect(value.height,value.id).toBeGreaterThanOrEqual(44);
    expect(value.width,value.id).toBeGreaterThan(80);
    expect(value.paddingEnd,value.id).toBeCloseTo(value.fontSize*3,1);
    expect(value.image,value.id).toContain('linear-gradient');
    expect(value.color,value.id).toBe(value.text);
    expect(value.background,value.id).toBe(value.hovered?value.surface:value.bg);
    expect(value.border,value.id).toBe(value.focused||value.hovered?value.accent:value.borderToken);
    expect(value.scheme,value.id).toBe(value.rootScheme);
    expect(value.optionColor,value.id).toBe(value.text);
    expect(value.optionBackground,value.id).toBe(value.panel);
  }
}

async function openConnection(page,kind='api') {
  await page.evaluate(()=>{OrchestrixSettings.close();OrchestrixConnections.addForm(OrchestrixApp.getState(),OrchestrixApp.helpers());});
  await page.locator(`[name="kind"][value="${kind}"]`).check();
  await page.locator('#conn-kind-form button[type="submit"]').click();
}

test('shared select treatment covers Settings and both connection paths without replacing native controls',async ({page})=>{
  await page.goto('/');
  const response=await page.request.get('/controls.css');expect(response.status()).toBe(200);
  await expect(page.locator('link[href="controls.css"]')).toHaveCount(1);
  for(const tab of ['general','orchestration','layout']) {
    await openSettings(page,tab);await assertSelectStyles(page,'#floating-settings select');
    expect(await page.locator('#floating-settings [role="combobox"]:not(select)').count()).toBe(0);
  }
  await openConnection(page,'own-plan');await assertSelectStyles(page,'#modal select');
  await page.getByRole('button',{name:'Close',exact:true}).click();
  await openConnection(page);await assertSelectStyles(page,'#modal select');
  await page.locator('#conn-api-provider').selectOption('bedrock');await assertSelectStyles(page,'#modal select');
  await page.locator('#conn-api-auth').selectOption('aws-profile');
  await expect(page.locator('#conn-api-profile')).toBeVisible();await expect(page.locator('#conn-api-key')).toHaveCount(0);
});

test('native selection, focus, disabled and invalid states remain available to keyboard and forms',async ({page})=>{
  await page.goto('/');await openSettings(page);
  const density=page.locator('#settings-density');await density.focus();
  await page.keyboard.press('End');await expect(density).toHaveValue('comfortable');
  expect(await page.evaluate(()=>OrchestrixApp.getUI().density)).toBe('comfortable');
  await page.keyboard.press('Home');await expect(density).toHaveValue('compact');
  expect(await density.evaluate(element=>({focus:element.matches(':focus-visible'),outline:getComputedStyle(element).outlineWidth}))).toEqual({focus:true,outline:'2px'});
  await density.evaluate(element=>{element.disabled=true;});
  const disabled=await density.evaluate(element=>({cursor:getComputedStyle(element).cursor,opacity:getComputedStyle(element).opacity}));
  expect(disabled).toEqual({cursor:'not-allowed',opacity:'0.55'});await expect(density).toBeDisabled();
  await density.evaluate(element=>{element.disabled=false;element.setAttribute('aria-invalid','true');});
  const errorColor=await page.evaluate(()=>{
    const probe=document.createElement('span');probe.style.color='var(--danger)';document.body.append(probe);const color=getComputedStyle(probe).color;probe.remove();return color;
  });
  await expect.poll(()=>density.evaluate(element=>getComputedStyle(element).borderColor)).toBe(errorColor);
  await density.evaluate(element=>element.removeAttribute('aria-invalid'));await density.blur();
  await assertSelectStyles(page,'#settings-density');
  await density.hover();const accentColor=await page.evaluate(()=>{
    const probe=document.createElement('span');probe.style.color='var(--accent)';document.body.append(probe);const color=getComputedStyle(probe).color;probe.remove();return color;
  });
  await expect.poll(()=>density.evaluate(element=>getComputedStyle(element).borderColor)).toBe(accentColor);
});

test('Studio connection selectors use the same controls and retain existing native selections',async ({page})=>{
  await usePortuguese(page);await page.goto('/');
  await page.evaluate(()=>OrchestrixConnections.addForm(OrchestrixApp.getState(),OrchestrixApp.helpers()));
  await assertSelectStyles(page,'#conn-provider,#conn-mode');
  await page.locator('#conn-provider').selectOption('Claude Code');await expect(page.locator('#conn-provider')).toHaveValue('Claude Code');
  await page.locator('#conn-mode').selectOption('api');await expect(page.locator('#conn-mode')).toHaveValue('api');
  await page.getByRole('button',{name:'Fechar',exact:true}).click();
  await page.evaluate(()=>OrchestrixConnections.click({dataset:{connAction:'authorize',connId:'codex-b'}},OrchestrixApp.getState(),OrchestrixApp.helpers()));
  await assertSelectStyles(page,'#conn-identity-result');await page.locator('#conn-identity-result').selectOption('different');await expect(page.locator('#conn-identity-result')).toHaveValue('different');
});

test('theme, language and 80–200% scaling apply to selects and native menu options with mobile reflow',async ({page})=>{
  test.setTimeout(90000);await page.goto('/');
  const themes=await page.evaluate(()=>OrchestrixApp.getThemes().map(theme=>theme.id));
  for(const language of ['en','pt-BR','es']) {
    await page.evaluate(language=>OrchestrixI18n.setLanguage(language),language);
    for(const scale of [80,125,200]) {
      await page.setViewportSize({width:390,height:844});
      await page.evaluate(scale=>{OrchestrixApp.getUI().textScale=scale;OrchestrixApp.saveUI();},scale);
      for(const theme of themes) {
        await page.evaluate(theme=>OrchestrixApp.setTheme(theme),theme);
        await openSettings(page);await assertSelectStyles(page,'#floating-settings select');
        expect(await page.locator('#settings-language').evaluate(element=>parseFloat(getComputedStyle(element).fontSize))).toBeCloseTo(14*scale/100,1);
        expect(await page.evaluate(()=>document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
        const bounds=await page.locator('#floating-settings select').evaluateAll(elements=>elements.map(element=>{
          const control=element.getBoundingClientRect(),body=element.closest('.floating-settings-body').getBoundingClientRect();return {left:control.left,right:control.right,bodyLeft:body.left,bodyRight:body.right};
        }));
        for(const bound of bounds){expect(bound.left).toBeGreaterThanOrEqual(bound.bodyLeft);expect(bound.right).toBeLessThanOrEqual(bound.bodyRight);}
      }
      await openSettings(page,'orchestration');await assertSelectStyles(page,'#floating-settings select');
      await openConnection(page);await assertSelectStyles(page,'#modal select');
      await page.locator('#conn-api-provider').selectOption('bedrock');await assertSelectStyles(page,'#modal select');
      expect(await page.evaluate(()=>document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
      await page.evaluate(()=>OrchestrixApp.helpers().closeModal());
    }
    expect(await page.evaluate(()=>OrchestrixI18n.missing())).toEqual([]);
  }
});

test('forced colors retain platform select affordances and readable options',async ({page})=>{
  await page.emulateMedia({forcedColors:'active'});await page.goto('/');await openSettings(page);
  const styles=await page.locator('#settings-density').evaluate(element=>{
    const style=getComputedStyle(element);return {appearance:style.appearance,image:style.backgroundImage,forced:style.forcedColorAdjust};
  });
  expect(styles).toEqual({appearance:'auto',image:'none',forced:'auto'});
  await page.locator('#settings-density').focus();await page.keyboard.press('End');await expect(page.locator('#settings-density')).toHaveValue('comfortable');
});

import {test,expect} from '@playwright/test';

test.beforeEach(async ({page})=>{await page.goto('/');});

test('fechar a inspeção automática após preparar projeto foca o trabalho atual',async ({page})=>{
  await page.locator('[data-action="onboarding"]').click();
  await page.getByRole('button',{name:'Preparar projeto de exemplo',exact:true}).click();
  await expect(page.getByRole('dialog')).toContainText('Autoriza\u00e7\u00e3o pendente');
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toBeHidden();
  await expect(page.locator('#content')).toBeFocused();
  await expect(page.locator('#content')).toHaveAttribute('tabindex','-1');
  await expect(page.getByRole('button',{name:'Iniciar demonstra\u00e7\u00e3o',exact:true})).toBeVisible();
  await page.keyboard.press('Tab');
  await expect(page.getByRole('button',{name:'Ver plano',exact:true})).toBeFocused();
});

test('adicionar conexão restaura foco útil depois que o opener é substituído',async ({page})=>{
  await page.locator('[data-view="connections"]').click();
  await page.getByRole('button',{name:'Adicionar conex\u00e3o simulada',exact:true}).click();
  await page.getByLabel('Nome para reconhecer a conta').fill('Codex \u00b7 segunda conex\u00e3o de exemplo');
  await page.getByLabel('Workspace de exemplo').fill('Pessoal \u00b7 QA');
  await page.getByRole('button',{name:'Adicionar registro pendente',exact:true}).click();
  await expect(page.getByRole('dialog')).toBeHidden();
  await expect(page.locator('.conn-row').last()).toContainText('Codex \u00b7 segunda conex\u00e3o de exemplo');
  await expect(page.locator('#content')).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(page.getByRole('button',{name:'Adicionar conex\u00e3o simulada',exact:true})).toBeFocused();
});

test('movimento reduzido remove transição do aviso e mantém foco visível',async ({page})=>{
  await page.emulateMedia({reducedMotion:'reduce'});
  expect(await page.evaluate(()=>matchMedia('(prefers-reduced-motion: reduce)').matches)).toBe(true);
  await page.getByRole('button',{name:'Personalizar apar\u00eancia',exact:true}).click();
  await page.getByRole('button',{name:'Restaurar Studio',exact:true}).click();
  await expect(page.locator('#toast')).toHaveClass(/visible/);
  const motion=await page.locator('#toast').evaluate(el=>{
    const css=getComputedStyle(el);return {transition:css.transitionDuration,animation:css.animationDuration,opacity:css.opacity};
  });
  expect(motion).toEqual({transition:'0s',animation:'0s',opacity:'1'});
  await page.locator('[data-view="work"]').click();
  await page.getByRole('button',{name:'Nova tarefa',exact:true}).click();
  await page.keyboard.press('Tab');
  await expect(page.getByLabel('T\u00edtulo da tarefa')).toBeFocused();
  const focus=await page.getByLabel('T\u00edtulo da tarefa').evaluate(el=>{
    const css=getComputedStyle(el);return {style:css.outlineStyle,width:css.outlineWidth};
  });
  expect(focus).toEqual({style:'solid',width:'2px'});
});

test('cores forçadas preservam seleção, campos e foco sem depender do fundo do tema',async ({page})=>{
  await page.emulateMedia({forcedColors:'active'});
  expect(await page.evaluate(()=>matchMedia('(forced-colors: active)').matches)).toBe(true);
  for(const selector of ['.task-item.selected','.nav-item.active']){
    const marker=await page.locator(selector).evaluate(el=>{
      const css=getComputedStyle(el);return {style:css.outlineStyle,width:css.outlineWidth};
    });
    expect(marker).toEqual({style:'solid',width:'2px'});
  }
  await page.getByRole('button',{name:'Nova tarefa',exact:true}).click();
  await page.keyboard.press('Tab');
  await expect(page.getByLabel('T\u00edtulo da tarefa')).toBeFocused();
  const field=await page.getByLabel('T\u00edtulo da tarefa').evaluate(el=>{
    const css=getComputedStyle(el);
    return {outline:css.outlineStyle,width:css.outlineWidth,border:css.borderColor,text:css.color,background:css.backgroundColor};
  });
  expect(field.outline).toBe('solid');expect(field.width).toBe('2px');
  expect(field.border).not.toBe(field.background);expect(field.text).not.toBe(field.background);
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toBeHidden();
  await page.getByRole('button',{name:'Personalizar apar\u00eancia',exact:true}).click();
  await expect(page.getByRole('radio',{name:'Studio',exact:true})).toBeChecked();
  await expect(page.getByRole('radio',{name:'Studio',exact:true})).toBeVisible();
});

test('textos Unicode no limite dos campos cabem em controles compactos e tela de 320px',async ({page})=>{
  await page.getByRole('button',{name:'Personalizar apar\u00eancia',exact:true}).click();
  await page.getByLabel('Densidade da interface').selectOption('compact');
  await page.locator('[data-view="work"]').click();
  const buttonHeight=await page.getByRole('button',{name:'Nova tarefa',exact:true}).evaluate(el=>el.getBoundingClientRect().height);
  expect(buttonHeight).toBeGreaterThanOrEqual(36);
  await page.setViewportSize({width:320,height:720});
  await page.getByRole('button',{name:'Nova tarefa',exact:true}).click();
  const title=('\u00c1\u6f22\u5b57&<x>'.repeat(20)).slice(0,100);
  const goal=('\u00c1\u6f22\u5b57&<literal>'+ 'x'.repeat(120)+'\n').repeat(20).slice(0,2000);
  await page.getByLabel('T\u00edtulo da tarefa').fill(title);
  await page.getByLabel('Objetivo e crit\u00e9rios').fill(goal);
  expect(await page.locator('#task-form').evaluate(el=>el.checkValidity())).toBe(true);
  await page.getByRole('button',{name:'Criar tarefa simulada',exact:true}).click();
  await expect(page.locator('.detail-title h2')).toHaveText(title);
  expect(await page.locator('.prose').first().textContent()).toBe(goal.trim());
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  await page.locator('[data-view="connections"]').click();
  await page.getByRole('button',{name:'Adicionar conex\u00e3o simulada',exact:true}).click();
  const identity=('\u00c1\u6f22\u5b57&<conta>'.repeat(10)).slice(0,70);
  await page.getByLabel('Nome para reconhecer a conta').fill(identity);
  await page.getByLabel('Workspace de exemplo').fill(identity);
  await page.getByRole('button',{name:'Adicionar registro pendente',exact:true}).click();
  await expect(page.locator('.conn-row').last()).toContainText(identity);
  await expect(page.locator('#content')).toBeFocused();
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  for(const platform of ['native','wsl']){
    await page.locator('[data-action="onboarding"]').click();
    await page.getByLabel('Ambiente de execu\u00e7\u00e3o').selectOption(platform);
    const path=((platform==='native'?'C:\\Projetos\\':'/home/leo/')+('\u00c1\u6f22\u5b57 pasta/').repeat(30)).slice(0,180);
    await page.getByLabel('Nome do projeto').fill(('\u00c1\u6f22\u5b57 projeto'.repeat(10)).slice(0,50));
    await page.getByLabel('Caminho do reposit\u00f3rio \u00b7 exemplo').fill(path);
    await page.getByRole('button',{name:'Preparar projeto de exemplo',exact:true}).click();
    await expect(page.locator('.project-meta').first()).toContainText(path);
    await page.keyboard.press('Escape');
    await expect(page.getByRole('dialog')).toBeHidden();
    await expect(page.locator('#content')).toBeFocused();
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  }
});

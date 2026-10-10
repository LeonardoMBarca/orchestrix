import {test,expect} from '@playwright/test';
import {openStudio,usePortuguese,openSettings} from './navigation.mjs';

test.beforeEach(async ({page})=>{await usePortuguese(page);await page.goto('/');await openStudio(page);await page.locator('[data-view="work"]').click();});

test('closing the project form restores the conversation composer',async ({page})=>{
  await page.evaluate(()=>OrchestrixApp.openProject());await page.locator('#entry-name').fill('Accessible project');
  await page.getByRole('button',{name:'Criar projeto e sessão',exact:true}).click();
  await expect(page.getByRole('dialog')).toBeHidden();await expect(page.locator('#chat-message')).toBeFocused();
});

test('connection registration restores useful focus after its opener is replaced',async ({page})=>{
  await page.locator('[data-view="connections"]').click();await page.getByRole('button',{name:'Adicionar conexão',exact:true}).click();
  await page.getByLabel('Nome para reconhecer a conta').fill('Codex · segunda conexão');await page.locator('#conn-workspace').fill('Pessoal · QA');
  await page.getByRole('button',{name:'Adicionar registro pendente',exact:true}).click();
  await expect(page.getByRole('dialog')).toBeHidden();await expect(page.locator('.conn-row').last()).toContainText('Codex · segunda conexão');
  await expect(page.locator('#content')).toBeFocused();await page.keyboard.press('Tab');
  await expect(page.getByRole('button',{name:'Adicionar conexão',exact:true})).toBeFocused();
});

test('reduced motion removes toast transitions and retains visible focus',async ({page})=>{
  await page.emulateMedia({reducedMotion:'reduce'});await page.getByRole('button',{name:'Nova tarefa',exact:true}).click();
  await page.getByLabel('Título da tarefa').fill('Accessible task');await page.getByLabel('Objetivo e critérios').fill('Check visible focus and feedback.');
  await page.getByRole('button',{name:'Criar tarefa simulada',exact:true}).click();
  await expect(page.locator('#toast')).toHaveClass(/visible/);
  expect(await page.locator('#toast').evaluate(e=>{const c=getComputedStyle(e);return [c.transitionDuration,c.animationDuration,c.opacity];})).toEqual(['0s','0s','1']);
  await page.getByRole('button',{name:'Nova tarefa',exact:true}).click();await page.keyboard.press('Tab');await expect(page.getByLabel('Título da tarefa')).toBeFocused();
  expect(await page.getByLabel('Título da tarefa').evaluate(e=>{const c=getComputedStyle(e);return [c.outlineStyle,c.outlineWidth];})).toEqual(['solid','2px']);
});

test('forced colors preserve selection, fields, focus and theme radios',async ({page})=>{
  await page.emulateMedia({forcedColors:'active'});
  for(const selector of ['.task-item.selected','.nav-item.active'])expect(await page.locator(selector).evaluate(e=>{const c=getComputedStyle(e);return [c.outlineStyle,c.outlineWidth];})).toEqual(['solid','2px']);
  await page.getByRole('button',{name:'Nova tarefa',exact:true}).click();await page.keyboard.press('Tab');
  const field=await page.getByLabel('Título da tarefa').evaluate(e=>{const c=getComputedStyle(e);return {outline:c.outlineStyle,width:c.outlineWidth,border:c.borderColor,text:c.color,background:c.backgroundColor};});
  expect(field.outline).toBe('solid');expect(field.width).toBe('2px');expect(field.border).not.toBe(field.background);expect(field.text).not.toBe(field.background);
  await page.keyboard.press('Escape');await openSettings(page,'themes');await expect(page.getByRole('radio',{name:'Studio',exact:true})).toBeChecked();
});

test('maximum Unicode text reflows at 320px without interpreting markup',async ({page})=>{
  await openSettings(page,'general');await page.getByLabel('Densidade da interface').selectOption('compact');await page.evaluate(()=>OrchestrixSettings.close());
  await page.setViewportSize({width:320,height:720});await page.getByRole('button',{name:'Nova tarefa',exact:true}).click();
  const title=('Á漢字&<x>'.repeat(20)).slice(0,100),goal=('Á漢字&<literal>'+'x'.repeat(120)+'\n').repeat(20).slice(0,2000);
  await page.getByLabel('Título da tarefa').fill(title);await page.getByLabel('Objetivo e critérios').fill(goal);await page.getByRole('button',{name:'Criar tarefa simulada',exact:true}).click();
  await expect(page.locator('.detail-title h2')).toHaveText(title);expect(await page.locator('.prose').first().textContent()).toBe(goal.trim());
  await page.getByRole('button',{name:'Conversa',exact:true}).click();await page.evaluate(()=>OrchestrixApp.openProject());
  const path=('C:\\Work\\'+'Á漢字 pasta/'.repeat(30)).slice(0,180);
  await page.locator('#entry-name').fill(('Á漢字 projeto'.repeat(10)).slice(0,50));await page.locator('#entry-path').fill(path);
  await page.getByRole('button',{name:'Criar projeto e sessão',exact:true}).click();
  expect(await page.evaluate(()=>OrchestrixApp.getState().projectPath)).toBe(path);
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);await expect(page.locator('#chat-message')).toBeFocused();
});

import {test,expect} from '@playwright/test';

const input=page=>page.locator('#chat-message');
const state=page=>page.evaluate(()=>({projectId:OrchestrixApp.getState().projectId,chatId:OrchestrixApp.getState().chat.id,isLoose:OrchestrixApp.getState().isLoose}));
async function sessions(page){await page.evaluate(()=>OrchestrixDock.activate('sessions'));}
async function newSession(page){await sessions(page);await page.locator('.new-session-trigger').click();}
async function createProject(page,name,path=''){
  await sessions(page);await page.locator('.session-toolbar [data-action="onboarding"]').click();
  await page.locator('#entry-name').fill(name);await page.locator('#entry-path').fill(path);
  await page.getByRole('button',{name:'Create project and session',exact:true}).click();
}
async function select(page,id){await sessions(page);await page.locator(`[data-session-id="${id}"]`).click();}

test('first use is an empty independent session and orders entry controls above the composer',async ({page})=>{
  const errors=[];page.on('pageerror',error=>errors.push(error.message));await page.goto('/');
  expect(await page.evaluate(()=>{const s=OrchestrixApp.getState();return [s.isLoose,s.tasks.length,s.connections.length,s.chat.messages.length];})).toEqual([true,0,0,0]);
  await expect(page.locator('#chat-project,#chat-session,.project-switch,#interface-language')).toHaveCount(0);
  await expect(page.getByRole('heading',{name:'What shall we create today?',exact:true})).toBeVisible();
  await expect(page.locator('#content')).not.toContainText(/demo|simulated response|example accounts|local project/i);
  const positions=await page.evaluate(()=>['.welcome-stage','.quick-start','.prompt-suggestions','#chat-form'].map(selector=>document.querySelector(selector).getBoundingClientRect().top));
  expect(positions).toEqual([...positions].sort((a,b)=>a-b));expect(errors).toEqual([]);
});

test('standalone sessions keep independent drafts and restore after reload',async ({page})=>{
  await page.goto('/');const first=await state(page);await input(page).fill('First session draft');
  await newSession(page);const second=await state(page);expect(second.chatId).not.toBe(first.chatId);
  await input(page).fill('Second session draft');await select(page,first.chatId);await expect(input(page)).toHaveValue('First session draft');
  await select(page,second.chatId);await expect(input(page)).toHaveValue('Second session draft');
  await page.evaluate(()=>OrchestrixApp.persistSessions());await page.reload();expect((await state(page)).chatId).toBe(second.chatId);await expect(input(page)).toHaveValue('Second session draft');
  await select(page,first.chatId);await expect(input(page)).toHaveValue('First session draft');
});

test('project creation needs no account and adds a nested session without losing the independent draft',async ({page})=>{
  await page.goto('/');const independent=await state(page);await input(page).fill('Keep my independent idea');
  await createProject(page,'Scientific notes','C:\\Work\\Papers');const project=await state(page);expect(project.isLoose).toBe(false);
  expect(await page.evaluate(()=>OrchestrixApp.getState().connections.length)).toBe(0);
  await expect(page.locator(`.session-project[data-project-id="${project.projectId}"]`)).toContainText('Scientific notes');
  await input(page).fill('Project session draft');await page.locator(`.session-project[data-project-id="${project.projectId}"] [data-new-session-project]`).click();
  const second=await state(page);expect(second.projectId).toBe(project.projectId);expect(second.chatId).not.toBe(project.chatId);await expect(input(page)).toHaveValue('');
  await select(page,project.chatId);await expect(input(page)).toHaveValue('Project session draft');await select(page,independent.chatId);await expect(input(page)).toHaveValue('Keep my independent idea');
});

test('attach to an existing project preserves identity, history, draft and old context provenance',async ({page})=>{
  await page.goto('/');await createProject(page,'Shared project','/work/shared');const project=await state(page);
  await newSession(page);await page.locator('[data-action="session-directory"]').click();await page.locator('#session-directory').fill('/work/independent');await page.getByRole('button',{name:'Save directory',exact:true}).click();
  const original=await state(page);await input(page).fill('Investigate the parser');await input(page).press('Enter');await input(page).fill('Unsent clarification');
  const before=await page.evaluate(()=>structuredClone(OrchestrixApp.getState().tasks[0].contextPack));
  await page.locator('[data-action="session-attach"]').click();await page.locator(`[data-attach-project="${project.projectId}"]`).click();
  const after=await state(page);expect(after.chatId).toBe(original.chatId);expect(after.projectId).toBe(project.projectId);expect(after.isLoose).toBe(false);
  await expect(input(page)).toHaveValue('Unsent clarification');await expect(page.getByRole('log',{name:'Chat',exact:true})).toContainText('Investigate the parser');
  expect(await page.evaluate(()=>OrchestrixApp.getState().tasks[0].contextPack)).toEqual(before);
  await sessions(page);await expect(page.locator(`[data-session-project="${original.projectId}"]`)).toHaveCount(0);
  await page.evaluate(()=>OrchestrixApp.persistSessions());await page.reload();expect((await state(page)).chatId).toBe(original.chatId);await expect(input(page)).toHaveValue('Unsent clarification');
});

test('create project around current session keeps its identity and draft',async ({page})=>{
  await page.goto('/');const before=await state(page);await input(page).fill('Build this with me');
  await page.locator('[data-action="session-attach"]').click();await page.locator('#entry-name').fill('New direction');await page.getByRole('button',{name:'Create and attach session',exact:true}).click();
  const after=await state(page);expect(after.chatId).toBe(before.chatId);expect(after.projectId).toBe(before.projectId);expect(after.isLoose).toBe(false);await expect(input(page)).toHaveValue('Build this with me');
});

test('future tasks snapshot personal instructions and shared context without changing old attempts',async ({page})=>{
  await page.goto('/');await createProject(page,'Architecture');
  await page.evaluate(()=>{OrchestrixApp.getState().projectContext='Existing architecture';OrchestrixApp.updateSettings({systemPrompt:'Be concise'});});
  await input(page).fill('Describe the current architecture');await input(page).press('Enter');
  const old=await page.evaluate(()=>structuredClone(OrchestrixApp.getState().tasks[0].contextPack));
  await page.evaluate(()=>{OrchestrixApp.updateSettings({systemPrompt:'Explain tradeoffs'});OrchestrixApp.getState().projectContext='Updated architecture';});
  await page.getByRole('button',{name:'New work in this chat',exact:true}).click();await input(page).fill('Compare the alternatives');await input(page).press('Enter');
  const packs=await page.evaluate(()=>OrchestrixApp.getState().tasks.map(task=>task.contextPack));expect(packs[0]).toEqual(old);
  expect(old.systemPrompt).toBe('Be concise');expect(old.project.sharedContext).toBe('Existing architecture');expect(packs[1].systemPrompt).toBe('Explain tradeoffs');expect(packs[1].project.sharedContext).toBe('Updated architecture');
});

test('search finds a session by project path and selection never starts its prepared task',async ({page})=>{
  await page.goto('/');await createProject(page,'Parser','/unique/search-path');await input(page).fill('Review parser options');await input(page).press('Enter');const target=await state(page);
  await newSession(page);await input(page).fill('Keep this draft');await page.keyboard.press('Control+k');await page.locator('#command-search').fill('search-path');
  await page.locator(`[data-search-session="${target.chatId}"]`).click();expect((await state(page)).chatId).toBe(target.chatId);
  expect(await page.evaluate(()=>OrchestrixApp.getState().tasks[0].status)).toBe('ready');
});

test('technical pages use product copy and preserve unknown runtime capabilities',async ({page})=>{
  await page.goto('/');await page.evaluate(()=>OrchestrixApp.savePreferences({context:'module'}));await input(page).fill('Explain the architecture');await input(page).press('Enter');
  expect(await page.evaluate(()=>OrchestrixApp.getState().tasks[0].contextPack.sources)).toEqual([]);
  await page.evaluate(()=>OrchestrixDock.activate('navigation'));await page.locator('#studio-navigation').evaluate(element=>element.open=true);await page.locator('[data-view="work"]').click();
  await expect(page.locator('#content')).not.toContainText(/demo|simulat|example account|prototype/i);
  await page.locator('[data-tab="code"]').click();await expect(page.locator('#content')).not.toContainText(/demo|simulat|prototype/i);
  await page.locator('[data-view="review"]').click();await expect(page.locator('#content')).not.toContainText(/demo|simulat|example|prototype/i);
  await page.locator('[data-view="work"]').click();await page.locator('[data-action="start"]').click();expect(await page.evaluate(()=>OrchestrixApp.getState().tasks[0].status)).toBe('ready');
  await expect(page.locator('#toast')).toContainText('runtime is not available');
});

for(const language of ['pt-BR','es'])test(`project dialog translates and preserves form values in ${language}`,async ({page})=>{
  await page.goto('/');await sessions(page);await page.locator('.session-toolbar [data-action="onboarding"]').click();await page.locator('#entry-name').fill('My unedited name');await page.locator('#entry-context').fill('My shared goal');
  await page.evaluate(language=>OrchestrixI18n.setLanguage(language),language);await expect(page.locator('#entry-name')).toHaveValue('My unedited name');await expect(page.locator('#entry-context')).toHaveValue('My shared goal');
  await expect(page.getByRole('dialog')).toHaveAccessibleName(language==='es'?'Abrir o crear proyecto':'Abrir ou criar projeto');
  expect(await page.evaluate(()=>OrchestrixI18n.missing())).toEqual([]);
});

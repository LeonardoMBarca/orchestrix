import {test,expect} from '@playwright/test';

const input=page=>page.locator('#chat-message');
const state=page=>page.evaluate(()=>({projectId:OrchestrixApp.getState().projectId,chatId:OrchestrixApp.getState().chat.id,isLoose:OrchestrixApp.getState().isLoose}));
async function sessions(page){await page.evaluate(()=>OrchestrixDock.activate('sessions'));}
async function newSession(page){await sessions(page);await page.locator('.new-session-trigger').click();}
async function contextAction(page,action){await page.locator('#chat-add-context').click();await page.locator(`#chat-context-menu [data-action="${action}"]`).click();}
async function createProject(page,name,path=''){
  await sessions(page);await page.locator('.session-toolbar [data-action="onboarding"]').click();
  await page.locator('#entry-name').fill(name);await page.locator('#entry-path').fill(path);
  await page.getByRole('button',{name:'Create project',exact:true}).click();
}
async function select(page,id){await sessions(page);await page.locator(`[data-session-id="${id}"]`).click();}
async function send(page,text){await input(page).fill(text);await input(page).press('Enter');}
async function home(page){await expect(page.getByRole('heading',{name:'What shall we create today?',exact:true})).toBeVisible();expect((await state(page)).chatId).toBeNull();await expect(input(page)).toHaveValue('');await expect(page.locator('.session-item.active')).toHaveCount(0);}

test('first use is an empty home without a saved session and orders entry controls above the composer',async ({page})=>{
  const errors=[];page.on('pageerror',error=>errors.push(error.message));await page.goto('/');
  expect(await page.evaluate(()=>{const s=OrchestrixApp.getState();return [s.isLoose,s.tasks.length,s.connections.length,s.chat.messages.length];})).toEqual([true,0,0,0]);
  await expect(page.locator('#chat-project,#chat-session,.project-switch,#interface-language')).toHaveCount(0);
  await expect(page.getByRole('heading',{name:'What shall we create today?',exact:true})).toBeVisible();
  await expect(page.locator('#content')).not.toContainText(/demo|simulated response|example accounts|local project/i);
  const positions=await page.evaluate(()=>['.welcome-stage','.quick-start','.prompt-suggestions','#chat-form'].map(selector=>document.querySelector(selector).getBoundingClientRect().top));
  expect(positions).toEqual([...positions].sort((a,b)=>a-b));expect(errors).toEqual([]);await home(page);
  expect(await page.evaluate(()=>OrchestrixApp.getState().conversations)).toEqual([]);await expect(page.locator('[data-session-id]')).toHaveCount(0);
});

test('reload and reopening always start at home while saved standalone sessions retain independent drafts',async ({page})=>{
  await page.goto('/');await send(page,'First session request');const first=await state(page);await input(page).fill('First session draft');
  await newSession(page);expect((await state(page)).chatId).toBeNull();await expect(page.locator('[data-session-id]')).toHaveCount(1);
  await send(page,'Second session request');const second=await state(page);expect(second.chatId).not.toBe(first.chatId);
  await input(page).fill('Second session draft');await select(page,first.chatId);await expect(input(page)).toHaveValue('First session draft');
  await select(page,second.chatId);await expect(input(page)).toHaveValue('Second session draft');
  await page.evaluate(()=>OrchestrixApp.persistSessions());
  await page.addInitScript(({projectId,chatId})=>{const saved=JSON.parse(localStorage.getItem('orchestrix-sessions-v1'));saved.activeProjectId=projectId;saved.activeSessionId=chatId;localStorage.setItem('orchestrix-sessions-v1',JSON.stringify(saved));},second);
  await page.reload();await home(page);await expect(page.locator('[data-session-id]')).toHaveCount(2);
  await select(page,second.chatId);await expect(input(page)).toHaveValue('Second session draft');await page.evaluate(()=>OrchestrixApp.persistSessions());
  const reopened=await page.context().newPage();await reopened.goto('/');await home(reopened);await expect(reopened.locator('[data-session-id]')).toHaveCount(2);
  await select(reopened,second.chatId);await expect(input(reopened)).toHaveValue('Second session draft');await reopened.close();
  await select(page,first.chatId);await expect(input(page)).toHaveValue('First session draft');
});

test('a project starts empty and creates each nested session only at its first nonempty send',async ({page})=>{
  await page.goto('/');await send(page,'Independent request');const independent=await state(page);await input(page).fill('Keep my independent idea');
  await createProject(page,'Scientific notes','C:\\Work\\Papers');const project=await state(page);expect(project.isLoose).toBe(false);
  expect(await page.evaluate(()=>OrchestrixApp.getState().connections.length)).toBe(0);
  await expect(page.locator(`.session-project[data-project-id="${project.projectId}"]`)).toContainText('Scientific notes');
  expect(project.chatId).toBeNull();expect(await page.evaluate(()=>OrchestrixApp.getState().conversations)).toEqual([]);
  await input(page).fill('Unsent project idea');await expect(page.locator(`.session-project[data-project-id="${project.projectId}"] [data-session-id]`)).toHaveCount(0);
  await send(page,'Investigate the scientific notes');const first=await state(page);expect(first.chatId).toEqual(expect.any(String));await input(page).fill('Project session draft');
  await page.locator(`[data-new-session-project="${project.projectId}"]`).click();expect((await state(page)).chatId).toBeNull();await expect(input(page)).toHaveValue('');
  await expect(page.locator(`.session-project[data-project-id="${project.projectId}"] [data-session-id]`)).toHaveCount(1);
  await send(page,'A second project request');const second=await state(page);expect(second.projectId).toBe(project.projectId);expect(second.chatId).not.toBe(first.chatId);
  await expect(page.locator(`.session-project[data-project-id="${project.projectId}"] [data-session-id]`)).toHaveCount(2);
  await select(page,first.chatId);await expect(input(page)).toHaveValue('Project session draft');await select(page,independent.chatId);await expect(input(page)).toHaveValue('Keep my independent idea');
});

test('attach to an existing project preserves identity, history, draft and old context provenance',async ({page})=>{
  await page.goto('/');await createProject(page,'Shared project','/work/shared');const project=await state(page);
  await newSession(page);await contextAction(page,'session-directory');await page.locator('#session-directory').fill('/work/independent');await page.getByRole('button',{name:'Save directory',exact:true}).click();
  await send(page,'Investigate the parser');const original=await state(page);await input(page).fill('Unsent clarification');
  const before=await page.evaluate(()=>structuredClone(OrchestrixApp.getState().tasks[0].contextPack));
  await contextAction(page,'session-attach');await page.locator(`[data-attach-project="${project.projectId}"]`).click();
  const after=await state(page);expect(after.chatId).toBe(original.chatId);expect(after.projectId).toBe(project.projectId);expect(after.isLoose).toBe(false);
  await expect(input(page)).toHaveValue('Unsent clarification');await expect(page.getByRole('log',{name:'Chat',exact:true})).toContainText('Investigate the parser');
  expect(await page.evaluate(()=>OrchestrixApp.getState().tasks[0].contextPack)).toEqual(before);
  await sessions(page);await expect(page.locator(`[data-session-project="${original.projectId}"]`)).toHaveCount(0);
  await page.evaluate(()=>OrchestrixApp.persistSessions());await page.reload();await home(page);await select(page,original.chatId);await expect(input(page)).toHaveValue('Unsent clarification');
});

test('create project around current session keeps its identity and draft',async ({page})=>{
  await page.goto('/');await send(page,'Start building this idea');const before=await state(page);await input(page).fill('Build this with me');
  await contextAction(page,'session-attach');await page.locator('#entry-name').fill('New direction');await page.getByRole('button',{name:'Create and attach session',exact:true}).click();
  const after=await state(page);expect(after.chatId).toBe(before.chatId);expect(after.projectId).toBe(before.projectId);expect(after.isLoose).toBe(false);await expect(input(page)).toHaveValue('Build this with me');
});

test('typing, empty sends, configuration, navigation and New session never create empty sessions',async ({page})=>{
  await page.goto('/');await input(page).fill('My unsent request');
  await page.locator('#activity-items>[data-activity="files"]').click();await page.locator('#activity-items>[data-activity="chat"]').click();await expect(input(page)).toHaveValue('My unsent request');
  await page.locator('#activity-items>[data-activity="connections"]').click();await page.locator('#activity-items>[data-activity="chat"]').click();await expect(input(page)).toHaveValue('My unsent request');
  await page.locator('.app-settings-trigger').click();await expect(page.locator('#floating-settings')).toBeVisible();await page.evaluate(()=>OrchestrixSettings.close());
  await page.locator('#chat-form [data-action="session-mode"]').click();await page.locator('#session-mode').selectOption('research');await page.locator('#session-mode-form button[type="submit"]').click();
  await page.locator('#chat-form [data-action="session-model"]').click();await page.locator('#session-model-form button[type="submit"]').click();
  await contextAction(page,'session-directory');await page.locator('#session-directory').fill('/work/unsent');await page.getByRole('button',{name:'Save directory',exact:true}).click();
  expect((await state(page)).chatId).toBeNull();await expect(page.locator('[data-session-id]')).toHaveCount(0);await expect(input(page)).toHaveValue('My unsent request');
  await input(page).fill('   \n  ');await input(page).press('Enter');expect((await state(page)).chatId).toBeNull();
  for(let index=0;index<3;index++)await newSession(page);
  expect(await page.evaluate(()=>OrchestrixApp.getState().conversations)).toEqual([]);await expect(page.locator('[data-session-id]')).toHaveCount(0);
  await page.evaluate(()=>OrchestrixApp.persistSessions());await page.reload();await home(page);await expect(page.locator('[data-session-id],.session-project')).toHaveCount(0);
});

test('attaching a pending composer creates an empty project and preserves its draft until first send',async ({page})=>{
  await page.goto('/');await input(page).fill('An idea not sent yet');await contextAction(page,'session-attach');await page.locator('#entry-name').fill('A future project');
  await page.getByRole('button',{name:'Create and use project',exact:true}).click();
  expect((await state(page)).chatId).toBeNull();expect((await state(page)).isLoose).toBe(false);await expect(input(page)).toHaveValue('An idea not sent yet');
  await expect(page.locator('.session-project')).toHaveCount(1);await expect(page.locator('[data-session-id]')).toHaveCount(0);
  await input(page).press('Enter');expect((await state(page)).chatId).toEqual(expect.any(String));await expect(page.locator('[data-session-id]')).toHaveCount(1);
  await send(page,'Continue the same session');await expect(page.locator('[data-session-id]')).toHaveCount(1);
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
  await page.locator('#activity-items>[data-activity="studio"]').click();await page.locator('[data-view="work"]').click();
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

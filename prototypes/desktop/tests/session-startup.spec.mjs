import {test,expect} from '@playwright/test';

const input=page=>page.locator('#chat-message');
const saved=page=>page.evaluate(()=>JSON.parse(localStorage.getItem('orchestrix-sessions-v1')));
async function home(page){
  await expect(page.getByRole('heading',{name:'What shall we create today?',exact:true})).toBeVisible();
  expect(await page.evaluate(()=>OrchestrixApp.getState().chat.id)).toBeNull();
  await expect(page.locator('.session-item.active')).toHaveCount(0);
  await expect(input(page)).toHaveValue('');
}

test('an empty project survives reload and supplies context only when its first message creates a session',async ({page})=>{
  await page.goto('/');
  const projectId=await page.evaluate(()=>{
    OrchestrixApp.createProject('Future work','/work/future','Use the shared research notes');
    OrchestrixApp.persistSessions();return OrchestrixApp.getState().projectId;
  });
  expect((await saved(page)).projects).toHaveLength(1);
  expect((await saved(page)).projects[0]).toMatchObject({chat:null,conversations:[]});
  await page.reload();await home(page);await expect(page.locator('.session-project')).toHaveCount(1);
  await expect(page.locator('[data-session-id]')).toHaveCount(0);
  await page.locator(`[data-new-session-project="${projectId}"]`).click();
  await input(page).fill('Not submitted yet');await page.evaluate(()=>OrchestrixApp.persistSessions());
  expect((await saved(page)).projects[0]).toMatchObject({chat:null,conversations:[]});
  await page.reload();await home(page);
  await page.locator(`[data-new-session-project="${projectId}"]`).click();
  await input(page).fill('Investigate the research notes');await input(page).press('Enter');
  const current=await page.evaluate(()=>OrchestrixApp.getState());
  expect(current.projectId).toBe(projectId);expect(current.conversations).toHaveLength(1);
  expect(current.chat.id).toEqual(expect.any(String));
  expect(current.tasks[0].contextPack.project).toMatchObject({path:'/work/future',sharedContext:'Use the shared research notes'});
});

test('legacy sessions with only a draft remain recoverable without being opened automatically',async ({page})=>{
  await page.goto('/');
  const legacy=await page.evaluate(()=>{
    const project=structuredClone(OrchestrixApp.getState());
    project.projectId='legacy-project';project.chat.id='legacy-draft-session';
    project.chat.draft='Preserve my draft from the previous build';project.conversations=[project.chat];
    return {version:1,activeProjectId:project.projectId,activeSessionId:project.chat.id,projects:[project]};
  });
  // Install the previous build's data before app startup, after its writer stops.
  await page.goto('about:blank');
  const reopened=await page.context().newPage();
  await reopened.addInitScript(legacy=>localStorage.setItem('orchestrix-sessions-v1',JSON.stringify(legacy)),legacy);
  await reopened.goto('/');await home(reopened);
  await expect(reopened.locator('[data-session-id]')).toHaveCount(1);
  await reopened.evaluate(()=>OrchestrixApp.persistSessions());
  expect((await saved(reopened)).projects[0].conversations[0].draft).toBe('Preserve my draft from the previous build');
  await reopened.locator('[data-session-id="legacy-draft-session"]').click();
  await expect(input(reopened)).toHaveValue('Preserve my draft from the previous build');
  await input(reopened).press('Enter');
  expect(await reopened.evaluate(()=>OrchestrixApp.getState().chat.id)).toBe('legacy-draft-session');
  await expect(reopened.locator('[data-session-id]')).toHaveCount(1);
});

test('closing and reopening the page preserves history while clearing the active view',async ({page})=>{
  await page.goto('/');await input(page).fill('Keep this conversation');await input(page).press('Enter');
  const sessionId=await page.evaluate(()=>OrchestrixApp.getState().chat.id);
  await input(page).fill('Saved session draft');await page.evaluate(()=>OrchestrixApp.persistSessions());
  const context=page.context();await page.close();
  const reopened=await context.newPage();await reopened.goto('/');await home(reopened);
  await expect(reopened.locator('[data-session-id]')).toHaveCount(1);
  await reopened.locator(`[data-session-id="${sessionId}"]`).click();
  await expect(input(reopened)).toHaveValue('Saved session draft');
  await expect(reopened.getByRole('log',{name:'Chat',exact:true})).toContainText('Keep this conversation');
});

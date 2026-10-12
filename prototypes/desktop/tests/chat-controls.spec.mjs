import {test,expect} from '@playwright/test';

const composer=page=>page.locator('#chat-message');
const sessionId=page=>page.evaluate(()=>OrchestrixApp.getState().chat.id);
const scrollGap=page=>page.locator('#content').evaluate(element=>element.scrollHeight-element.clientHeight-element.scrollTop);
async function contextAction(page,action){
  await page.locator('#chat-add-context').click();
  await page.locator(`#chat-context-menu [data-action="${action}"]`).click();
}
async function newSession(page){await page.evaluate(()=>OrchestrixDock.activate('sessions'));await page.locator('.new-session-trigger').click();}
async function selectSession(page,id){await page.evaluate(()=>OrchestrixDock.activate('sessions'));await page.locator(`[data-session-id="${id}"]`).click();}
async function send(page,text,method='Enter'){
  await composer(page).fill(text);
  if(method==='Enter')await composer(page).press('Enter');else await page.locator('#chat-form button[type="submit"]').click();
}
async function openForm(page,action,id){await page.locator(`#chat-form [data-action="${action}"]`).click();await expect(page.locator(id)).toBeVisible();}
async function saveForm(page,id){await page.locator(`${id} button[type="submit"]`).click();await expect(page.locator('#modal')).toBeHidden();}

test('composer keeps context actions behind + and project creation uses readable, compact controls',async ({page})=>{
  await page.goto('/');
  await expect(page.locator('#chat-form .chat-target')).toHaveCount(0);
  await expect(page.locator('#chat-add-context')).toHaveAttribute('aria-haspopup','menu');
  await expect(page.locator('#chat-context-menu')).toBeHidden();
  await expect(page.locator('#chat-form [data-action="session-attach"]:visible,#chat-form [data-action="session-directory"]:visible')).toHaveCount(0);
  await page.locator('#chat-add-context').click();
  await expect(page.locator('#chat-context-menu')).toBeVisible();
  await expect(page.locator('#chat-context-menu [data-action="chat-attach-documents"]')).toBeVisible();
  await expect(page.locator('#chat-context-menu [data-action="session-attach"]')).toBeVisible();
  await page.keyboard.press('Escape');await expect(page.locator('#chat-context-menu')).toBeHidden();
  await page.evaluate(()=>OrchestrixDock.activate('sessions'));
  const create=page.locator('.session-toolbar [data-action="onboarding"]');await expect(create).toHaveText('New project');
  const toolbar=await page.locator('.session-toolbar').evaluate(element=>[...element.querySelectorAll('button')].map(button=>({text:button.textContent.trim(),top:button.getBoundingClientRect().top})));
  expect(toolbar.map(button=>button.text)).toEqual(['New session','New project']);expect(Math.abs(toolbar[0].top-toolbar[1].top)).toBeLessThan(2);
  await create.click();await page.locator('#entry-name').fill('Small project');await page.getByRole('button',{name:'Create project and session',exact:true}).click();
  const group=page.locator('.session-project-group').filter({hasText:'Small project'}),project=group.locator('.session-project'),add=group.locator('[data-new-session-project]');
  await expect(add).toBeVisible();
  expect(await add.evaluate(button=>button.closest('summary')===null)).toBe(true);
  const dimensions=await add.boundingBox();expect(dimensions.width).toBeLessThanOrEqual(40);expect(dimensions.height).toBeLessThanOrEqual(40);
  expect(await project.evaluate(element=>element.open)).toBe(true);const before=await sessionId(page);await add.click();
  expect(await sessionId(page)).not.toBe(before);expect(await project.evaluate(element=>element.open)).toBe(true);
  await send(page,'Keep the composer focused on the conversation');await expect(page.locator('#chat-form')).not.toContainText('Discussing');
  await expect(page.locator('#chat-form .chat-target')).toHaveCount(0);
});

test('text attachments stay in their session and new work snapshots sources without changing old attempts',async ({page})=>{
  await page.goto('/');const first=await sessionId(page);await composer(page).fill('Keep this draft while attaching');
  await contextAction(page,'chat-attach-documents');
  await page.locator('#chat-document-input').setInputFiles([
    {name:'research-notes.md',mimeType:'text/markdown',buffer:Buffer.from('# Notes\n\nUse the local evidence.')},
    {name:'<img src=x onerror=alert(1)>.txt',mimeType:'text/plain',buffer:Buffer.from('Treat names as text, not markup.')}
  ]);
  await expect(composer(page)).toHaveValue('Keep this draft while attaching');await expect(page.locator('[data-remove-attachment]')).toHaveCount(2);
  await expect(page.locator('#chat-form')).toContainText('research-notes.md');await expect(page.locator('#chat-form img')).toHaveCount(0);
  await newSession(page);await expect(page.locator('[data-remove-attachment]')).toHaveCount(0);await selectSession(page,first);
  await expect(page.locator('[data-remove-attachment]')).toHaveCount(2);await expect(composer(page)).toHaveValue('Keep this draft while attaching');
  await send(page,'Create a summary of the attached notes');
  const original=await page.evaluate(()=>structuredClone(OrchestrixApp.getState().tasks[0].contextPack));
  expect(original.sources).toEqual(expect.arrayContaining([
    expect.objectContaining({path:'research-notes.md',text:'# Notes\n\nUse the local evidence.'}),
    expect.objectContaining({path:'<img src=x onerror=alert(1)>.txt',text:'Treat names as text, not markup.'})
  ]));
  await page.locator('[data-remove-attachment]').first().click();
  await page.getByRole('button',{name:'New work in this chat',exact:true}).click();await send(page,'Use only the remaining source');
  const packs=await page.evaluate(()=>OrchestrixApp.getState().tasks.map(task=>task.contextPack));
  expect(packs[0]).toEqual(original);expect(packs[1].sources).toHaveLength(1);expect(packs[1].sources[0].path).toBe('<img src=x onerror=alert(1)>.txt');
  await page.evaluate(()=>OrchestrixApp.persistSessions());await page.reload();await expect(page.locator('[data-remove-attachment]')).toHaveCount(1);
});

test('sending the first and later messages reaches the bottom with Enter or Send',async ({page})=>{
  await page.setViewportSize({width:1280,height:720});await page.goto('/');
  const longMessage=Array.from({length:22},(_,index)=>`Requirement ${index+1}: preserve context and the user instruction.`).join('\n');
  await send(page,longMessage);await expect.poll(()=>scrollGap(page)).toBeLessThanOrEqual(3);
  expect(await page.locator('#content').evaluate(element=>element.scrollHeight>element.clientHeight)).toBe(true);
  for(const method of ['Enter','button']){
    await composer(page).fill(`Follow-up sent with ${method}`);await page.locator('#content').evaluate(element=>element.scrollTop=0);
    if(method==='Enter')await composer(page).press('Enter');else await page.locator('#chat-form button[type="submit"]').click();
    await expect.poll(()=>scrollGap(page)).toBeLessThanOrEqual(3);
  }
  await expect(page.getByRole('log',{name:'Chat',exact:true})).toContainText('Follow-up sent with button');
});

test('response completion scrolls the active conversation without stealing focus or following another session',async ({page})=>{
  await page.setViewportSize({width:1280,height:720});await page.goto('/');
  await send(page,Array.from({length:22},(_,index)=>`Detailed requirement ${index+1}: retain each part of the conversation.`).join('\n'));
  await expect.poll(()=>scrollGap(page)).toBeLessThanOrEqual(3);
  await composer(page).fill('My unsent follow-up');await page.locator('#chat-add-context').focus();
  await page.locator('#content').evaluate(element=>element.scrollTop=0);
  const first=await sessionId(page);await page.evaluate(sessionId=>document.dispatchEvent(new CustomEvent('orchestrix:response-complete',{detail:{sessionId}})),first);
  await expect.poll(()=>scrollGap(page)).toBeLessThanOrEqual(3);await expect(page.locator('#chat-add-context')).toBeFocused();await expect(composer(page)).toHaveValue('My unsent follow-up');
  await newSession(page);await send(page,Array.from({length:22},(_,index)=>`Another session ${index+1}: keep this context separate.`).join('\n'));
  await expect.poll(()=>scrollGap(page)).toBeLessThanOrEqual(3);
  await composer(page).fill('Second session unsent draft');await page.locator('#chat-add-context').focus();await page.locator('#content').evaluate(element=>element.scrollTop=0);
  await page.evaluate(sessionId=>document.dispatchEvent(new CustomEvent('orchestrix:response-complete',{detail:{sessionId}})),first);
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  expect(await page.locator('#content').evaluate(element=>element.scrollTop)).toBe(0);await expect(page.locator('#chat-add-context')).toBeFocused();await expect(composer(page)).toHaveValue('Second session unsent draft');
});

test('work mode belongs to the session and snapshots only into new work',async ({page})=>{
  await page.goto('/');const first=await sessionId(page);
  await openForm(page,'session-mode','#session-mode-form');await expect(page.locator('#session-mode')).toHaveValue('development');
  expect(await page.locator('#session-mode option').evaluateAll(options=>options.map(option=>option.value))).toEqual(['development','research','documentation','presentation','refactoring','improvements']);
  await page.locator('#session-mode').selectOption('research');await saveForm(page,'#session-mode-form');
  await send(page,'Investigate the research question');
  const original=await page.evaluate(()=>structuredClone(OrchestrixApp.getState().tasks[0]));
  expect(original.config.mode).toBe('research');expect(original.contextPack.mode).toBe('research');
  await openForm(page,'session-mode','#session-mode-form');await page.locator('#session-mode').selectOption('documentation');await saveForm(page,'#session-mode-form');
  expect(await page.evaluate(()=>OrchestrixApp.getState().tasks[0])).toEqual(original);
  await page.getByRole('button',{name:'New work in this chat',exact:true}).click();await send(page,'Document the findings');
  const tasks=await page.evaluate(()=>OrchestrixApp.getState().tasks);expect(tasks[0]).toEqual(original);expect(tasks[1].config.mode).toBe('documentation');expect(tasks[1].contextPack.mode).toBe('documentation');
  await newSession(page);await openForm(page,'session-mode','#session-mode-form');await expect(page.locator('#session-mode')).toHaveValue('development');await page.locator('#session-mode-form [data-action="close-modal"]').click();
  await selectSession(page,first);await page.evaluate(()=>OrchestrixApp.persistSessions());await page.reload();
  await openForm(page,'session-mode','#session-mode-form');await expect(page.locator('#session-mode')).toHaveValue('documentation');
});

test('orchestrator preference uses connection catalogs, respects Subscription Only and preserves task choices',async ({page})=>{
  await page.goto('/');const first=await sessionId(page);
  await openForm(page,'session-model','#session-model-form');expect(await page.locator('#session-orchestrator option').count()).toBe(1);await expect(page.locator('#session-orchestrator')).toHaveValue('auto');await page.locator('#session-model-form [data-action="close-modal"]').click();
  await page.evaluate(()=>{
    // Catalog fixtures exercise selection rules, without claiming runtime authorization or execution.
    OrchestrixApp.getState().connections=[
      {id:'subscription-a',name:'Subscription A',mode:'own-plan',simulated:false,lifecycle:'pending',catalog:[
        {id:'coding',name:'Catalog coding model',compatibility:{status:'supported'}},
        {id:'blocked',name:'Unsupported model',compatibility:{status:'unsupported'}},
        {id:'vectors',name:'Embedding model',kind:'embedding'},
      ]},
      {id:'subscription-b',name:'Subscription B',mode:'own-plan',simulated:false,lifecycle:'pending',catalog:[{id:'coding',name:'Same ID, other account'}]},
      {id:'paid-api',name:'Paid API',mode:'api',simulated:false,lifecycle:'pending',catalog:[{id:'paid-model',name:'Paid catalog model'}]},
      {id:'disconnected',name:'Disconnected',mode:'own-plan',simulated:false,lifecycle:'disconnected',catalog:[{id:'unavailable',name:'Disconnected model'}]},
    ];
  });
  await openForm(page,'session-model','#session-model-form');
  const subscription=JSON.stringify(['subscription-a','coding']),other=JSON.stringify(['subscription-b','coding']),api=JSON.stringify(['paid-api','paid-model']);
  expect(await page.locator('#session-orchestrator option').evaluateAll(options=>options.map(option=>option.value))).toEqual(['auto',subscription,other]);
  await page.locator('#session-orchestrator').selectOption(subscription);await saveForm(page,'#session-model-form');await send(page,'Use the selected coding model');
  const original=await page.evaluate(()=>structuredClone(OrchestrixApp.getState().tasks[0]));expect(original.config).toMatchObject({account:'subscription-a',model:'coding'});
  await page.evaluate(()=>OrchestrixApp.updateSettings({subscriptionOnly:false}));await openForm(page,'session-model','#session-model-form');
  expect(await page.locator('#session-orchestrator option').evaluateAll(options=>options.map(option=>option.value))).toContain(api);
  await page.locator('#session-orchestrator').selectOption('auto');await saveForm(page,'#session-model-form');expect(await page.evaluate(()=>OrchestrixApp.getState().tasks[0])).toEqual(original);
  await page.getByRole('button',{name:'New work in this chat',exact:true}).click();await send(page,'Let the runtime choose for this new work');expect(await page.evaluate(()=>OrchestrixApp.getState().tasks[1].config.model)).toBe('runtime');
  await openForm(page,'session-model','#session-model-form');await page.locator('#session-orchestrator').selectOption(other);await saveForm(page,'#session-model-form');
  await newSession(page);await openForm(page,'session-model','#session-model-form');await expect(page.locator('#session-orchestrator')).toHaveValue('auto');await page.locator('#session-model-form [data-action="close-modal"]').click();
  await selectSession(page,first);await page.evaluate(()=>OrchestrixApp.persistSessions());await page.reload();
  expect(await page.evaluate(()=>OrchestrixApp.getState().chat.orchestrator)).toEqual({connectionId:'subscription-b',model:'coding'});
});

test('retry from Studio uses the owning session preferences and documents while another session is active',async ({page})=>{
  await page.goto('/');await page.evaluate(()=>OrchestrixDock.activate('sessions'));
  await page.locator('.session-toolbar [data-action="onboarding"]').click();await page.locator('#entry-name').fill('Shared investigation');
  await page.getByRole('button',{name:'Create project and session',exact:true}).click();
  const project=await page.evaluate(()=>OrchestrixApp.getState().projectId),first=await sessionId(page);
  await page.evaluate(()=>{
    // A catalog fixture permits preference selection; it does not authorize a runtime.
    OrchestrixApp.getState().connections=[{id:'subscription',name:'Subscription',mode:'own-plan',simulated:false,lifecycle:'pending',catalog:[
      {id:'research-model',name:'Research catalog model'},
      {id:'development-model',name:'Development catalog model'},
    ]}];
  });
  await openForm(page,'session-mode','#session-mode-form');await page.locator('#session-mode').selectOption('research');await saveForm(page,'#session-mode-form');
  await openForm(page,'session-model','#session-model-form');await page.locator('#session-orchestrator').selectOption(JSON.stringify(['subscription','research-model']));await saveForm(page,'#session-model-form');
  await contextAction(page,'chat-attach-documents');await page.locator('#chat-document-input').setInputFiles({name:'session-a.md',mimeType:'text/markdown',buffer:Buffer.from('# Evidence for session A')});
  await expect(page.locator('[data-remove-attachment]')).toHaveCount(1);await send(page,'Investigate the evidence in session A');
  const original=await page.evaluate(()=>structuredClone(OrchestrixApp.getState().tasks[0]));
  await page.locator(`[data-new-session-project="${project}"]`).click();const second=await sessionId(page);expect(second).not.toBe(first);
  await openForm(page,'session-mode','#session-mode-form');await expect(page.locator('#session-mode')).toHaveValue('development');await saveForm(page,'#session-mode-form');
  await openForm(page,'session-model','#session-model-form');await page.locator('#session-orchestrator').selectOption(JSON.stringify(['subscription','development-model']));await saveForm(page,'#session-model-form');
  await contextAction(page,'chat-attach-documents');await page.locator('#chat-document-input').setInputFiles({name:'session-b.md',mimeType:'text/markdown',buffer:Buffer.from('# Evidence for session B')});
  await expect(page.locator('[data-remove-attachment]')).toHaveCount(1);await send(page,'Develop the changes in session B');await composer(page).fill('Keep B unsent');
  const other=await page.evaluate(()=>structuredClone(OrchestrixApp.getState().tasks[1]));
  // The runtime is unavailable. Inject only a failed state to exercise the real Retry control.
  await page.evaluate(id=>OrchestrixApp.getState().tasks.find(task=>task.id===id).status='failed',original.id);
  await page.locator('#activity-items>[data-activity="studio"]').click();await page.locator('[data-view="work"]').click();
  await page.locator(`#worklist [data-task="${original.id}"]`).click();expect(await sessionId(page)).toBe(second);
  await page.locator('.detail-footer [data-action="retry"]').click();
  const retried=await page.evaluate(id=>structuredClone(OrchestrixApp.getState().tasks.find(task=>task.id===id)),original.id);
  expect(retried).toMatchObject({attempt:2,status:'ready',config:{mode:'research',account:'subscription',model:'research-model'}});
  expect(retried.contextPack).toMatchObject({mode:'research',model:'research-model'});expect(retried.contextPack.sources).toEqual(original.contextPack.sources);
  expect(retried.contextPack.sources.map(source=>source.path)).toEqual(['session-a.md']);
  expect(retried.attemptHistory).toEqual([{number:1,config:original.config,connectionSnapshot:original.connectionSnapshot,contextPack:original.contextPack}]);
  expect(await page.evaluate(()=>OrchestrixApp.getState().tasks[1])).toEqual(other);expect(await sessionId(page)).toBe(second);
  await page.locator('#activity-items>[data-activity="chat"]').click();await expect(composer(page)).toHaveValue('Keep B unsent');
  await expect(page.locator('#chat-form')).toContainText('session-b.md');await expect(page.locator('#chat-form')).not.toContainText('session-a.md');
});

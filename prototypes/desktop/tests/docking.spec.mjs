import {test,expect} from '@playwright/test';
test.beforeEach(async({page})=>{await page.goto('/');await expect(page.locator('#chat-message')).toBeVisible();});

test('default has one session creator on the left and mouse placement blurs the workspace',async({page})=>{
  await expect(page.locator('#dock-left>#sessions-panel')).toBeVisible();await expect(page.locator('#dock-right>#navigation-panel')).toBeVisible();
  await expect(page.locator('.new-session-trigger')).toHaveCount(1);await expect(page.locator('#navigation-panel [data-action="chat-loose"]')).toHaveCount(0);
  const field=page.locator('#chat-message');await field.fill('Preserve my draft while rearranging.');const original=await field.elementHandle();
  const handle=page.locator('#sessions-panel .dock-panel-handle'),start=await handle.boundingBox();
  await page.mouse.move(start.x+start.width/2,start.y+start.height/2);await page.mouse.down();await page.mouse.move(start.x+start.width/2+30,start.y+start.height/2+20,{steps:5});
  await expect(page.locator('#dock-drop-targets')).toBeVisible();expect(await page.locator('#app').evaluate(e=>getComputedStyle(e).filter)).toBe('blur(7px)');
  await expect(page.locator('[data-drop-zone="bottom"]')).toHaveCount(0);
  const target=await page.locator('[data-drop-zone="right"]').boundingBox();await page.mouse.move(target.x+target.width/2,target.y+target.height/2,{steps:15});await page.mouse.up();
  await expect(page.locator('#dock-right>#sessions-panel')).toBeVisible();await expect(page.locator('#dock-right [role="tab"]')).toHaveCount(2);
  await expect(field).toHaveValue('Preserve my draft while rearranging.');expect(await field.evaluate((e,old)=>e===old,original)).toBe(true);
  expect(await page.locator('#app').evaluate(e=>getComputedStyle(e).filter)).toBe('none');
  await page.locator('[data-dock-tab="navigation"]').click();await expect(page.locator('.app-settings-trigger')).toBeVisible();
  await page.locator('[data-dock-tab="sessions"]').click();await expect(page.locator('#session-list')).toBeVisible();
});

test('side chooser has only left/right; cancel and placement restore clarity and persist',async({page})=>{
  const handle=page.locator('#sessions-panel .dock-panel-handle');await handle.click();
  await expect(page.locator('#dock-position-menu button')).toHaveCount(2);expect(await page.locator('#app').evaluate(e=>getComputedStyle(e).filter)).toBe('blur(7px)');
  await page.keyboard.press('Escape');await expect(page.locator('#dock-position-menu')).toBeHidden();await expect(handle).toBeFocused();
  expect(await page.locator('#app').evaluate(e=>getComputedStyle(e).filter)).toBe('none');
  await handle.click();await page.getByRole('button',{name:'Move to right',exact:true}).click();await expect(page.locator('#dock-right>#sessions-panel')).toBeVisible();
  await handle.press('ArrowDown');expect(await page.evaluate(()=>OrchestrixDock.getLayout().positions.sessions)).toBe('right');expect(await page.evaluate(()=>OrchestrixDock.move('sessions','bottom'))).toBe(false);
  await page.reload();await expect(page.locator('#dock-right>#sessions-panel')).toBeVisible();await page.evaluate(()=>OrchestrixDock.restore());await expect(page.locator('#dock-left>#sessions-panel')).toBeVisible();
});

test('shared dock tabs keep arrow navigation separate from moving',async({page})=>{
  await page.evaluate(()=>{OrchestrixDock.move('navigation','left');OrchestrixDock.activate('sessions');});
  const sessions=page.locator('[data-dock-tab="sessions"]');await sessions.focus();await sessions.press('ArrowLeft');await expect(page.locator('[data-dock-tab="navigation"]')).toBeFocused();
  await page.locator('[data-dock-tab="navigation"]').press('ArrowRight');await expect(sessions).toBeFocused();await sessions.press('Shift+ArrowRight');await expect(page.locator('#dock-right>#sessions-panel')).toBeVisible();
});

test('moving work independently preserves its task identity and the conversation draft',async({page})=>{
  await page.locator('#chat-message').fill('Investigate a parser edge case');await page.locator('#chat-message').press('Enter');await page.evaluate(()=>OrchestrixDock.activate('work'));
  const identity=await page.evaluate(()=>OrchestrixApp.getState().tasks.map(t=>[t.id,t.attempt,t.contextPack.id]));
  await page.locator('#work-panel .dock-panel-handle').press('ArrowLeft');await expect(page.locator('#dock-left .chat-task-card')).toHaveCount(1);
  await page.locator('#chat-message').fill('Unsent follow-up');await page.locator('#work-panel .dock-panel-handle').press('ArrowRight');
  expect(await page.evaluate(()=>OrchestrixApp.getState().tasks.map(t=>[t.id,t.attempt,t.contextPack.id]))).toEqual(identity);await expect(page.locator('#chat-message')).toHaveValue('Unsent follow-up');
});

test('resize bounds and focus mode preserve the available center at 100% and 200%',async({page})=>{
  const resize=page.locator('#dock-left [role="separator"]');await resize.focus();await resize.press('ArrowRight');await expect(resize).toHaveAttribute('aria-valuenow','300');
  for(let i=0;i<12;i++)await resize.press('ArrowRight');await expect(resize).toHaveAttribute('aria-valuenow','440');
  for(const scale of [100,200]){
    await page.evaluate(scale=>{document.documentElement.dataset.textSize=scale===200?'large':'normal';document.documentElement.style.setProperty('--font-scale',scale/100);},scale);
    await page.locator('.focus-toggle').click();await expect(page.locator('#sessions-panel')).not.toBeVisible();expect(await page.locator('.main').evaluate(e=>Math.round(e.getBoundingClientRect().width))).toBe(1440);
    await page.locator('.focus-toggle').click();await expect(page.locator('#sessions-panel')).toBeVisible();
  }
});

test('legacy bottom layouts migrate safely and mobile keeps essentials reachable',async({page})=>{
  await page.addInitScript(()=>localStorage.setItem('orchestrix-panel-layout-v1','{"version":1,"positions":{"navigation":"evil","sessions":"bottom"},"sizes":{"left":100000,"bottom":100000}}'));
  await page.reload();expect(await page.evaluate(()=>OrchestrixDock.getLayout().positions.sessions)).toBe('left');expect(await page.evaluate(()=>OrchestrixDock.getLayout().sizes.left)).toBe(440);
  await expect(page.locator('#dock-bottom')).toHaveCount(0);
  for(const width of [320,390,1024]){await page.setViewportSize({width,height:900});await expect(page.locator('#chat-message')).toBeVisible();expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);}
});

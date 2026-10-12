import {test,expect} from '@playwright/test';
test.beforeEach(async({page})=>{await page.goto('/');await expect(page.locator('#chat-message')).toBeVisible();});
const openFiles=page=>page.locator('#activity-items>[data-activity="files"]').click();

test('default has one session creator on the right and mouse placement blurs the workspace',async({page})=>{
  await expect(page.locator('#navigation-panel')).toBeHidden();await expect(page.locator('#dock-right>#sessions-panel')).toBeVisible();
  await expect(page.locator('.new-session-trigger')).toHaveCount(1);await expect(page.locator('#navigation-panel [data-action="chat-loose"]')).toHaveCount(0);
  await openFiles(page);await expect(page.locator('#dock-left>#navigation-panel')).toBeVisible();
  const field=page.locator('#chat-message');await field.fill('Preserve my draft while rearranging.');const original=await field.elementHandle();
  const handle=page.locator('#sessions-panel .dock-panel-handle'),start=await handle.boundingBox();
  await page.mouse.move(start.x+start.width/2,start.y+start.height/2);await page.mouse.down();await page.mouse.move(start.x+start.width/2+30,start.y+start.height/2+20,{steps:5});
  await expect(page.locator('#dock-drop-targets')).toBeVisible();expect(await page.locator('#app').evaluate(e=>getComputedStyle(e).filter)).toBe('blur(7px)');
  await expect(page.locator('[data-drop-zone="bottom"]')).toHaveCount(0);
  const target=await page.locator('[data-drop-zone="left"]').boundingBox();await page.mouse.move(target.x+target.width/2,target.y+target.height/2,{steps:15});await page.mouse.up();
  await expect(page.locator('#dock-left>#sessions-panel')).toBeVisible();await expect(page.locator('#dock-left [role="tab"]')).toHaveCount(2);
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
  await handle.click();await page.getByRole('button',{name:'Move to left',exact:true}).click();await expect(page.locator('#dock-left>#sessions-panel')).toBeVisible();
  await handle.press('ArrowDown');expect(await page.evaluate(()=>OrchestrixDock.getLayout().positions.sessions)).toBe('left');expect(await page.evaluate(()=>OrchestrixDock.move('sessions','bottom'))).toBe(false);
  await page.reload();await expect(page.locator('#dock-left>#sessions-panel')).toBeVisible();await page.evaluate(()=>OrchestrixDock.restore());await expect(page.locator('#dock-right>#sessions-panel')).toBeVisible();
});

test('shared dock tabs keep arrow navigation separate from moving',async({page})=>{
  await openFiles(page);
  await page.evaluate(()=>{OrchestrixDock.move('sessions','left');OrchestrixDock.activate('sessions');});
  const sessions=page.locator('[data-dock-tab="sessions"]');await sessions.focus();await sessions.press('ArrowLeft');await expect(page.locator('[data-dock-tab="navigation"]')).toBeFocused();
  await page.locator('[data-dock-tab="navigation"]').press('ArrowRight');await expect(sessions).toBeFocused();await sessions.press('Shift+ArrowRight');await expect(page.locator('#dock-right>#sessions-panel')).toBeVisible();
});

test('rail activities reopen their single moved workspace instead of collapsing the sessions tab',async({page})=>{
  await openFiles(page);
  const workspace=await page.locator('#navigation-panel').elementHandle();
  for(const side of ['right','left','right']){
    const handle=page.locator('#navigation-panel .dock-panel-handle');
    await handle.press(side==='right'?'ArrowRight':'ArrowLeft');
    await expect(page.locator(`#dock-${side}>#navigation-panel`)).toBeVisible();
    await page.evaluate(side=>{OrchestrixDock.move('sessions',side);OrchestrixDock.activate('navigation');},side);
    for(const activity of ['files','studio','connections']){
      const button=page.locator(`#activity-items>[data-activity="${activity}"]`);
      await page.evaluate(activity=>OrchestrixShell.select(activity),activity);
      await expect(page.locator(`#dock-${side}>#navigation-panel`)).toBeVisible();
      await expect(page.locator(`#${activity}-pane`)).toBeVisible();
      expect(await page.locator('#navigation-panel').evaluate((element,original)=>element===original,workspace)).toBe(true);
      await page.locator('[data-dock-tab="sessions"]').click();
      await expect(button).toHaveAttribute('aria-expanded','false');
      await button.click();
      await expect(page.locator(`#dock-${side}>#navigation-panel`)).toBeVisible();
      await expect(page.locator(`#${activity}-pane`)).toBeVisible();
      await expect(button).toHaveAttribute('aria-expanded','true');
      await expect(page.locator('#navigation-panel')).toHaveCount(1);
      expect(await page.evaluate(()=>OrchestrixDock.getLayout().positions.navigation)).toBe(side);
    }
  }
  await page.reload();
  await openFiles(page);
  await expect(page.locator('#dock-right>#navigation-panel')).toBeVisible();
  await expect(page.locator('#navigation-panel')).toHaveCount(1);
});

test('moving an open mobile workspace retargets the overlay and rail icons follow its saved side',async({page})=>{
  await page.setViewportSize({width:390,height:844});
  await openFiles(page);
  await expect(page.locator('#app')).toHaveAttribute('data-mobile-dock','left');
  for(const side of ['right','left']){
    await page.locator('#navigation-panel .dock-panel-handle').press(side==='right'?'ArrowRight':'ArrowLeft');
    await expect(page.locator('#app')).toHaveAttribute('data-mobile-dock',side);
    await expect(page.locator(`#dock-${side}>#navigation-panel`)).toBeVisible();
    await page.keyboard.press('Escape');
    for(const activity of ['files','studio','connections']){
      await page.locator(`#activity-items>[data-activity="${activity}"]`).click();
      await expect(page.locator('#app')).toHaveAttribute('data-mobile-dock',side);
      await expect(page.locator(`#dock-${side}>#navigation-panel`)).toBeVisible();
      await expect(page.locator(`#${activity}-pane`)).toBeVisible();
      await page.keyboard.press('Escape');
    }
    await openFiles(page);
  }
});

test('moving work independently preserves its task identity and the conversation draft',async({page})=>{
  await page.locator('#chat-message').fill('Investigate a parser edge case');await page.locator('#chat-message').press('Enter');await page.evaluate(()=>OrchestrixDock.activate('work'));
  const identity=await page.evaluate(()=>OrchestrixApp.getState().tasks.map(t=>[t.id,t.attempt,t.contextPack.id]));
  await page.locator('#work-panel .dock-panel-handle').press('ArrowLeft');await expect(page.locator('#dock-left .chat-task-card')).toHaveCount(1);
  await page.locator('#chat-message').fill('Unsent follow-up');await page.locator('#work-panel .dock-panel-handle').press('ArrowRight');
  expect(await page.evaluate(()=>OrchestrixApp.getState().tasks.map(t=>[t.id,t.attempt,t.contextPack.id]))).toEqual(identity);await expect(page.locator('#chat-message')).toHaveValue('Unsent follow-up');
});

test('resize bounds and focus mode preserve the available center at 100% and 200%',async({page})=>{
  await openFiles(page);
  const resize=page.locator('#dock-left [role="separator"]');await resize.focus();await resize.press('ArrowRight');await expect(resize).toHaveAttribute('aria-valuenow','244');
  for(let i=0;i<12;i++)await resize.press('ArrowRight');await expect(resize).toHaveAttribute('aria-valuenow','440');
  for(const scale of [100,200]){
    await page.evaluate(scale=>{document.documentElement.dataset.textSize=scale===200?'large':'normal';document.documentElement.style.setProperty('--font-scale',scale/100);},scale);
    await page.locator('.focus-toggle').click();await expect(page.locator('#sessions-panel')).not.toBeVisible();expect(await page.locator('.main').evaluate(e=>Math.round(e.getBoundingClientRect().width))).toBe(1388);await expect(page.locator('#activity-bar')).toBeVisible();
    await page.locator('.focus-toggle').click();await expect(page.locator('#sessions-panel')).toBeVisible();
  }
});

test('legacy bottom layouts migrate safely and mobile keeps essentials reachable',async({page})=>{
  await page.addInitScript(()=>localStorage.setItem('orchestrix-panel-layout-v1','{"version":1,"positions":{"navigation":"evil","sessions":"bottom"},"sizes":{"left":100000,"bottom":100000}}'));
  await page.reload();expect(await page.evaluate(()=>OrchestrixDock.getLayout().positions.sessions)).toBe('right');expect(await page.evaluate(()=>OrchestrixDock.getLayout().sizes.left)).toBe(440);
  await expect(page.locator('#dock-bottom')).toHaveCount(0);
  for(const width of [320,390,1024]){await page.setViewportSize({width,height:900});await expect(page.locator('#chat-message')).toBeVisible();expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);}
});

test('keyboard collapse persists and restores the remembered width without replacing the draft',async({page})=>{
  const field=page.locator('#chat-message');await field.fill('Keep this draft while collapsing.');const original=await field.elementHandle();
  const resize=page.locator('#dock-right [role="separator"]');await resize.focus();await resize.press('ArrowLeft');await expect(resize).toHaveAttribute('aria-valuenow','300');
  await resize.press('Home');await expect(page.locator('#dock-right')).toHaveClass(/is-collapsed/);await expect(resize).toHaveAttribute('aria-valuemin','0');await expect(resize).toHaveAttribute('aria-valuenow','0');await expect(resize).toBeFocused();
  expect(await page.evaluate(()=>OrchestrixDock.getLayout().sizes.right)).toBe(300);expect(await page.locator('#app').evaluate(e=>e.style.getPropertyValue('--dock-right'))).toBe('0px');
  await expect(field).toHaveValue('Keep this draft while collapsing.');expect(await field.evaluate((e,old)=>e===old,original)).toBe(true);
  await page.reload();await expect(page.locator('#dock-right')).toHaveClass(/is-collapsed/);await resize.focus();await resize.press('End');await expect(resize).toHaveAttribute('aria-valuenow','300');await expect(page.locator('#sessions-panel')).toBeVisible();
  await page.evaluate(()=>{OrchestrixDock.collapse('right');OrchestrixDock.activate('sessions');});await expect(page.locator('#sessions-panel')).toBeVisible();
});

test('pointer shrink collapses and the edge separator pulls the panel open again',async({page})=>{
  const field=page.locator('#chat-message');await field.fill('Pointer draft');const original=await field.elementHandle();
  const resize=page.locator('#dock-right [role="separator"]'),start=await resize.boundingBox();
  await page.mouse.move(start.x+start.width/2,start.y+start.height/2);await page.mouse.down();await page.mouse.move(start.x+start.width/2+225,start.y+start.height/2,{steps:12});await page.mouse.up();
  await expect(page.locator('#dock-right')).toHaveClass(/is-collapsed/);await expect(resize).toHaveAttribute('aria-valuenow','0');await expect(resize).toBeVisible();
  const edge=await resize.boundingBox();await page.mouse.move(edge.x+edge.width/2,edge.y+edge.height/2);await page.mouse.down();await page.mouse.move(edge.x+edge.width/2-260,edge.y+edge.height/2,{steps:12});await page.mouse.up();
  await expect(page.locator('#sessions-panel')).toBeVisible();await expect(resize).toHaveAttribute('aria-valuenow','260');await expect(field).toHaveValue('Pointer draft');expect(await field.evaluate((e,old)=>e===old,original)).toBe(true);
});

test('v2 defaults migrate but customized positions and widths are retained',async({page})=>{
  await page.evaluate(()=>{localStorage.removeItem('orchestrix-panel-layout-v3');localStorage.setItem('orchestrix-panel-layout-v2',JSON.stringify({version:2,positions:{navigation:'right',sessions:'left',work:'right'},active:{left:'sessions',right:'navigation'},sizes:{left:280,right:224}}));});
  await page.reload();await expect(page.locator('#navigation-panel')).toBeHidden();await expect(page.locator('#dock-right>#sessions-panel')).toBeVisible();expect(await page.evaluate(()=>OrchestrixDock.getLayout().version)).toBe(3);
  await openFiles(page);await expect(page.locator('#dock-left>#navigation-panel')).toBeVisible();
  await page.evaluate(()=>{localStorage.removeItem('orchestrix-panel-layout-v3');localStorage.setItem('orchestrix-panel-layout-v2',JSON.stringify({version:2,positions:{navigation:'right',sessions:'left',work:'left'},active:{left:'sessions',right:'navigation'},sizes:{left:340,right:250}}));});
  await page.reload();expect(await page.evaluate(()=>OrchestrixDock.getLayout())).toMatchObject({positions:{navigation:'right',sessions:'left',work:'left'},sizes:{left:340,right:250},collapsed:{left:false,right:false}});await expect(page.locator('#dock-left>#sessions-panel')).toBeVisible();
  await openFiles(page);await expect(page.locator('#dock-right>#navigation-panel')).toBeVisible();
  await page.evaluate(()=>OrchestrixDock.collapse('right'));expect(await page.evaluate(()=>JSON.parse(localStorage.getItem('orchestrix-panel-layout-v3')))).toMatchObject({version:3,positions:{navigation:'right',sessions:'left',work:'left'},sizes:{left:340,right:250},collapsed:{left:false,right:true}});
});

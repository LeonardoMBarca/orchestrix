import {test,expect} from '@playwright/test';

test.beforeEach(async({page})=>{await page.goto('/');await expect(page.locator('#chat-message')).toBeVisible();});

test('Chat opens directly and search stretches across the free header space',async({page})=>{
  await page.setViewportSize({width:1440,height:900});
  await expect(page.locator('#navigation-panel')).toBeHidden();await expect(page.locator('#sessions-panel')).toBeVisible();
  await expect(page.locator('[data-toggle-dock="left"]')).toBeHidden();
  const search=page.locator('#command-trigger'),before=await search.boundingBox();expect(before.width).toBeGreaterThan(500);
  await page.locator('[data-toggle-dock="right"]').click();const after=await search.boundingBox();expect(after.width-before.width).toBeGreaterThan(250);
  await search.click();await expect(page.locator('#command-search')).toBeFocused();await page.keyboard.press('Escape');
  for(const width of [1280,390]){await page.setViewportSize({width,height:800});expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);}
});

test('returning to Chat removes only its workspace pane on either side and preserves the draft',async({page})=>{
  const input=page.locator('#chat-message');await input.fill('Start a conversation to preserve');await input.press('Enter');const session=await page.evaluate(()=>OrchestrixApp.getState().chat.id);
  await input.fill('Keep this conversation draft');await input.evaluate(element=>{element.focus();element.setSelectionRange(4,12);});const original=await input.elementHandle();
  for(const side of ['left','right']){
    await page.locator('#activity-items>[data-activity="files"]').click();await expect(page.locator('#files-pane')).toBeVisible();
    await page.evaluate(side=>{OrchestrixDock.move('navigation',side);OrchestrixDock.move('sessions',side);OrchestrixDock.activate('navigation');},side);
    const layout=await page.evaluate(()=>OrchestrixDock.getLayout());
    await page.locator('#activity-items>[data-activity="chat"]').click();await expect(page.locator('#navigation-panel')).toBeHidden();await expect(page.locator('#sessions-panel')).toBeVisible();
    await expect(input).toHaveValue('Keep this conversation draft');await expect(input).toBeFocused();expect(await input.evaluate((element,previous)=>element===previous,original)).toBe(true);expect(await input.evaluate(element=>[element.selectionStart,element.selectionEnd])).toEqual([4,12]);
    expect(await page.evaluate(()=>{const {positions,sizes,collapsed}=OrchestrixDock.getLayout();return {positions,sizes,collapsed};})).toEqual({positions:layout.positions,sizes:layout.sizes,collapsed:layout.collapsed});
    await page.locator('#activity-items>[data-activity="chat"]').click();await expect(page.locator('#sessions-panel')).toBeVisible();
  }
  await page.evaluate(()=>OrchestrixApp.persistSessions());await page.reload();await expect(page.locator('#navigation-panel')).toBeHidden();await expect(input).toHaveValue('');expect(await page.evaluate(()=>OrchestrixApp.getState().chat.id)).toBeNull();
  await page.locator(`[data-session-id="${session}"]`).click();await expect(input).toHaveValue('Keep this conversation draft');
});

test('fixed rail keeps essential actions available after both docks collapse',async({page})=>{
  await expect(page.locator('#activity-items>[data-activity]')).toHaveCount(4);
  expect(await page.locator('#activity-items>[data-activity]').evaluateAll(buttons=>buttons.map(button=>button.dataset.activity))).toEqual(['chat','files','studio','connections']);
  await page.evaluate(()=>{OrchestrixDock.collapse('left');OrchestrixDock.collapse('right');});
  await expect(page.locator('#activity-bar')).toBeVisible();await expect(page.locator('.app-settings-trigger')).toBeVisible();
  await page.locator('.app-settings-trigger').click();await expect(page.locator('#floating-settings')).toBeVisible();await page.evaluate(()=>OrchestrixSettings.close());
  await page.locator('[data-activity-profile]').click();await expect(page.locator('#settings-profile-name')).toBeVisible();
  await expect(page.locator('#help-link')).toHaveAttribute('target','_blank');
});

test('activity reordering works with keyboard and pointer and persists independently of docking',async({page})=>{
  const files=page.locator('#activity-items>[data-activity="files"]');await files.focus();await files.press('Alt+ArrowUp');
  expect(await page.evaluate(()=>OrchestrixShell.getOrder())).toEqual(['files','chat','studio','connections']);await expect(files).toBeFocused();
  const from=await page.locator('#activity-items>[data-activity="connections"]').boundingBox(),to=await files.boundingBox();
  await page.mouse.move(from.x+from.width/2,from.y+from.height/2);await page.mouse.down();await page.mouse.move(to.x+to.width/2,to.y+to.height/2,{steps:12});await page.mouse.up();
  expect(await page.evaluate(()=>OrchestrixShell.getOrder())).toEqual(['connections','files','chat','studio']);
  await page.reload();expect(await page.evaluate(()=>OrchestrixShell.getOrder())).toEqual(['connections','files','chat','studio']);
  expect(await page.evaluate(()=>OrchestrixDock.getLayout().positions.sessions)).toBe('right');
});

test('File and View menus expose working app actions, keyboard navigation and layout reset',async({page})=>{
  await page.locator('#chat-message').fill('Keep this session draft');const original=await page.evaluate(()=>OrchestrixApp.getState().chat.id);
  await page.evaluate(()=>{
    window.filePickerCalls=0;window.folderPickerCalls=0;
    window.showOpenFilePicker=async()=>{window.filePickerCalls++;throw new DOMException('Cancelled','AbortError');};
    window.showDirectoryPicker=async()=>{window.folderPickerCalls++;throw new DOMException('Cancelled','AbortError');};
  });
  const fileMenu=page.locator('[data-app-menu="file"]');await fileMenu.focus();await fileMenu.press('ArrowDown');
  await expect(page.locator('#app-menu-popup')).toBeVisible();
  expect(await page.locator('#app-menu-popup [data-menu-command]').evaluateAll(buttons=>buttons.map(button=>button.dataset.menuCommand))).toEqual(['new-file','open-file','open-folder']);
  await expect(page.locator('[data-menu-command="new-file"]')).toBeFocused();await page.keyboard.press('Escape');await expect(fileMenu).toBeFocused();
  await fileMenu.click();await page.locator('[data-menu-command="open-file"]').click();await expect.poll(()=>page.evaluate(()=>window.filePickerCalls)).toBe(1);
  await page.keyboard.press('Control+o');await expect.poll(()=>page.evaluate(()=>window.filePickerCalls)).toBe(2);
  await page.keyboard.press('Control+Shift+o');await expect.poll(()=>page.evaluate(()=>window.folderPickerCalls)).toBe(1);
  await fileMenu.click();await page.locator('[data-menu-command="open-folder"]').click();await expect.poll(()=>page.evaluate(()=>window.folderPickerCalls)).toBe(2);
  await page.keyboard.press('Control+n');await expect(page.locator('#new-file-dialog')).toBeVisible();await page.locator('#new-file-dialog [data-new-file-cancel]').click();
  await fileMenu.click();await page.locator('[data-menu-command="new-file"]').click();await expect(page.locator('#new-file-dialog')).toBeVisible();await page.locator('#new-file-dialog [data-new-file-cancel]').click();
  expect(await page.evaluate(()=>OrchestrixApp.getState().chat.id)).toBe(original);await expect(page.locator('#chat-message')).toHaveValue('Keep this session draft');
  await page.locator('[data-app-menu="view"]').click();await page.locator('[data-menu-command="toggle-right"]').click();await expect(page.locator('#dock-right')).toHaveClass(/is-collapsed/);
  await page.locator('[data-app-menu="view"]').click();await page.locator('[data-menu-command="restore-layout"]').click();await expect(page.locator('#sessions-panel')).toBeVisible();
  await expect(page.locator('#app-menubar')).not.toContainText(/Run|Terminal/);
});

test('Edit menu keeps a keyboard selection and disables commands without a visible field',async({page})=>{
  const input=page.locator('#chat-message'),menu=page.locator('[data-app-menu="edit"]');
  await input.fill('Keep this draft');await input.evaluate(element=>{element.focus();element.setSelectionRange(5,9);});
  await menu.focus();await menu.press('ArrowDown');await page.locator('[data-menu-command="cut"]').click();
  await expect(input).toHaveValue('Keep  draft');await expect(input).toBeFocused();
  await page.locator('#activity-items>[data-activity="connections"]').click();await menu.focus();await menu.press('ArrowDown');
  for(const command of ['undo','redo','cut','copy','paste','selectAll'])await expect(page.locator(`[data-menu-command="${command}"]`)).toBeDisabled();
});

test('files and chat keep the original composer, selection and unsaved file content',async({page})=>{
  const input=page.locator('#chat-message');await input.fill('My unsent request');await input.evaluate(element=>{element.focus();element.setSelectionRange(3,8);});const original=await input.elementHandle();
  await page.locator('#activity-items>[data-activity="files"]').click();await expect(page.locator('#files-pane')).toBeVisible();
  await page.evaluate(async()=>{OrchestrixFiles.importFiles([new File(['KEY=value\n'],'settings.env')]);await OrchestrixFiles.openFile('settings.env');});
  await expect(page.locator('#file-editor')).toBeVisible();await page.locator('.file-text-editor').fill('KEY=changed\n');
  await page.locator('[data-file-chat]').click();await expect(input).toHaveValue('My unsent request');expect(await input.evaluate((element,previous)=>element===previous,original)).toBe(true);
  expect(await input.evaluate(element=>[element.selectionStart,element.selectionEnd])).toEqual([3,8]);
  await page.locator('[data-file-tab="settings.env"]').click();await expect(page.locator('.file-text-editor')).toHaveValue('KEY=changed\n');await expect(page.locator('.file-dirty')).toBeVisible();
});

test('mobile uses dismissible panels while rail and chat remain reachable at large text sizes',async({page})=>{
  await page.setViewportSize({width:390,height:844});
  for(const scale of [100,200]){
    await page.evaluate(scale=>{document.documentElement.dataset.textSize=scale===200?'large':'normal';document.documentElement.style.setProperty('--font-scale',scale/100);},scale);
    await page.locator('#activity-items>[data-activity="files"]').click();await expect(page.locator('#files-pane')).toBeVisible();await page.keyboard.press('Escape');await expect(page.locator('#chat-message')).toBeVisible();
    await page.locator('[data-toggle-dock="right"]').click();await expect(page.locator('#session-list')).toBeVisible();await page.keyboard.press('Escape');
    await expect(page.locator('#activity-bar')).toBeVisible();expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  }
});

for(const language of ['en','pt-BR','es'])test(`${language}: every activity and menu is translated without missing keys`,async({page})=>{
  await page.evaluate(language=>OrchestrixI18n.setLanguage(language),language);
  for(const activity of ['files','studio','connections','chat'])await page.locator(`#activity-items>[data-activity="${activity}"]`).click();
  for(const menu of ['file','edit','view','help']){await page.locator(`[data-app-menu="${menu}"]`).click();await expect(page.locator('#app-menu-popup')).toBeVisible();await page.keyboard.press('Escape');}
  expect(await page.evaluate(()=>OrchestrixI18n.missing())).toEqual([]);
});

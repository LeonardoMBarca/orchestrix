import {test,expect} from '@playwright/test';
import {resolve} from 'node:path';

// Exercise actual browser file APIs through controlled handles; never touch the repo.
test.beforeEach(async ({page})=>{
  await page.goto('/help.html');
  await page.setContent('<main><aside id="files-pane"><div id="files-toolbar"></div><div id="files-tree"></div></aside><div id="editor-tabs" hidden></div><section id="content"><textarea id="chat-draft">Keep my conversation</textarea></section><section id="file-editor" hidden></section><output id="files-notification"></output></main>');
  await page.addScriptTag({path:resolve('locales/catalog.js')});
  await page.addScriptTag({path:resolve('i18n.js')});
  await page.addScriptTag({path:resolve('files.js')});
  await page.evaluate(()=>OrchestrixFiles.init({
    notify:text=>document.querySelector('#files-notification').textContent=text,
    onEditorChange:({visible})=>{document.querySelector('#file-editor').hidden=!visible;document.querySelector('#content').hidden=visible;}
  }));
});

async function importFile(page,name,text,path=name){
  await page.evaluate(({name,text,path})=>{const file=new File([text],name,{type:'text/plain'});Object.defineProperty(file,'webkitRelativePath',{value:path});OrchestrixFiles.importFiles([file]);},{name,text,path});
}

async function installNativeFixtures(page){
  await page.evaluate(()=>{
    window.fileOperations={created:[],writes:[],permissionRequests:0};window.fileAccess='granted';
    window.makeTestDirectory=(name,contents={},children={})=>{
      const files=new Map(),directories=new Map(Object.entries(children));
      const permission={queryPermission:async()=>window.fileAccess,requestPermission:async()=>{window.fileOperations.permissionRequests++;return window.fileAccess;}};
      const makeFile=(filename,text='')=>({name:filename,kind:'file',...permission,getFile:async()=>new File([text],filename),createWritable:async()=>{
        window.fileOperations.writes.push(filename);let next=text;
        return {write:async value=>{next=String(value);},close:async()=>{text=next;},abort:async()=>{}};
      }});
      for(const [filename,text] of Object.entries(contents))files.set(filename,makeFile(filename,text));
      return {name,kind:'directory',...permission,async *values(){yield* directories.values();yield* files.values();},
        getDirectoryHandle:async filename=>{if(directories.has(filename))return directories.get(filename);throw new DOMException('Missing directory','NotFoundError');},
        getFileHandle:async(filename,options={})=>{
          if(files.has(filename))return files.get(filename);
          if(!options.create)throw new DOMException('Missing file','NotFoundError');
          window.fileOperations.created.push(`${name}/${filename}`);const file=makeFile(filename);files.set(filename,file);return file;
        }
      };
    };
  });
}

async function openNewFileDialog(page){
  await page.evaluate(()=>{OrchestrixFiles.newFile();});await expect(page.locator('#new-file-dialog')).toBeVisible();
}

test('opening a standalone file preserves the folder and dirty document even when names match',async({page})=>{
  await installNativeFixtures(page);
  await page.evaluate(async()=>{
    window.currentFolder=makeTestDirectory('project',{'README.md':'# Project'});
    window.externalFolder=makeTestDirectory('external',{'README.md':'# External'});
    window.showDirectoryPicker=async()=>currentFolder;await OrchestrixFiles.openFolder();
    window.showOpenFilePicker=async()=>[await externalFolder.getFileHandle('README.md')];
  });
  await page.locator('[data-file-path="README.md"]').click();await page.locator('.file-text-editor').fill('# Keep my edits');
  await page.locator('[data-files-pick]').first().click();await expect(page.locator('.file-text-editor')).toHaveValue('# External');
  const state=await page.evaluate(()=>OrchestrixFiles.getState());expect(state.folder).toBe('project');expect(state.opened).toHaveLength(2);
  expect(state.opened.find(doc=>doc.path==='README.md').dirty).toBe(true);expect(state.opened[1].path).not.toBe('README.md');
  await page.locator('[data-file-tab="README.md"]').click();await expect(page.locator('.file-text-editor')).toHaveValue('# Keep my edits');await expect(page.locator('[data-file-tab="README.md"] .file-dirty')).toBeVisible();
  expect(await page.evaluate(()=>fileOperations)).toEqual({created:[],writes:[],permissionRequests:0});await expect(page.locator('#chat-draft')).toHaveValue('Keep my conversation');
});

test('cancelling the single-file picker leaves the active document and drafts unchanged',async({page})=>{
  await importFile(page,'notes.md','# Notes');await page.locator('[data-file-path="notes.md"]').click();await page.locator('.file-text-editor').fill('# Unsent edits');
  const before=await page.evaluate(()=>OrchestrixFiles.getState());
  await page.evaluate(()=>{window.showOpenFilePicker=async()=>{throw new DOMException('Cancelled','AbortError');};return OrchestrixFiles.pickFile();});
  expect(await page.evaluate(()=>OrchestrixFiles.getState())).toEqual(before);await expect(page.locator('.file-text-editor')).toHaveValue('# Unsent edits');await expect(page.locator('#chat-draft')).toHaveValue('Keep my conversation');
  await expect(page.locator('#files-notification')).toHaveText('');
});

test('new text files are created in the open folder only after explicit confirmation',async({page})=>{
  await installNativeFixtures(page);
  await page.evaluate(()=>{window.currentFolder=makeTestDirectory('project',{'existing.txt':'Keep this'});window.showDirectoryPicker=async()=>currentFolder;return OrchestrixFiles.openFolder();});
  await page.locator('[data-file-path="existing.txt"]').click();await page.locator('.file-text-editor').fill('Keep these unsaved edits');
  await page.locator('[data-files-new]').first().click();await expect(page.locator('#new-file-dialog')).toBeVisible();await page.locator('#new-file-dialog [name="filename"]').fill('notes');
  expect(await page.evaluate(()=>fileOperations.created)).toEqual([]);
  await page.locator('[data-new-file-create]').click();await expect(page.locator('#new-file-dialog')).toBeHidden();await expect(page.locator('.file-text-editor')).toHaveValue('');
  expect(await page.evaluate(()=>fileOperations.created)).toEqual(['project/notes.txt']);
  await expect(page.locator('[data-file-tab="notes.txt"]')).toBeVisible();await page.locator('[data-file-tab="existing.txt"]').click();await expect(page.locator('.file-text-editor')).toHaveValue('Keep these unsaved edits');
  expect(await page.evaluate(async()=>await(await currentFolder.getFileHandle('existing.txt')).getFile().then(file=>file.text()))).toBe('Keep this');
});

test('creating in the selected subfolder validates names and never overwrites a collision',async({page})=>{
  await installNativeFixtures(page);
  await page.evaluate(()=>{
    window.docsFolder=makeTestDirectory('docs',{'README.md':'# Existing'});window.currentFolder=makeTestDirectory('project',{}, {docs:docsFolder});window.showDirectoryPicker=async()=>currentFolder;return OrchestrixFiles.openFolder();
  });
  await page.locator('[data-file-path="docs"]').click();await openNewFileDialog(page);
  const name=page.locator('#new-file-dialog [name="filename"]'),create=page.locator('[data-new-file-create]');
  await name.fill('../escape.txt');await create.click();await expect(page.locator('[data-new-file-error]')).toBeVisible();expect(await page.evaluate(()=>fileOperations.created)).toEqual([]);
  await name.fill('README.md');await create.click();await expect(page.locator('[data-new-file-error]')).toBeVisible();expect(await page.evaluate(()=>fileOperations.created)).toEqual([]);
  expect(await page.evaluate(async()=>await(await docsFolder.getFileHandle('README.md')).getFile().then(file=>file.text()))).toBe('# Existing');
  await name.fill('analysis.csv');await create.click();await expect(page.locator('#new-file-dialog')).toBeHidden();expect(await page.evaluate(()=>fileOperations.created)).toEqual(['docs/analysis.csv']);
  await expect(page.locator('[data-file-tab="docs/analysis.csv"]')).toBeVisible();await expect(page.locator('.file-text-editor')).toHaveValue('');
});

test('without an open folder a new file requires a destination and cancelled selection writes nothing',async({page})=>{
  await installNativeFixtures(page);
  await page.evaluate(()=>{window.showDirectoryPicker=async()=>{throw new DOMException('Cancelled','AbortError');};});
  await openNewFileDialog(page);await page.locator('#new-file-dialog [name="filename"]').fill('research.md');await expect(page.locator('[data-new-file-create]')).toBeDisabled();
  await expect(page.locator('#new-file-dialog')).toBeVisible();expect(await page.evaluate(()=>fileOperations.created)).toEqual([]);
  await page.locator('[data-new-file-directory]').click();await expect(page.locator('#new-file-dialog')).toBeVisible();expect(await page.evaluate(()=>fileOperations.created)).toEqual([]);
  await page.evaluate(()=>{window.destination=makeTestDirectory('papers');window.showDirectoryPicker=async()=>destination;});await page.locator('[data-new-file-directory]').click();
  await page.locator('[data-new-file-create]').click();await expect(page.locator('#new-file-dialog')).toBeHidden();expect(await page.evaluate(()=>fileOperations.created)).toEqual(['papers/research.md']);
  await expect(page.locator('.file-text-editor')).toHaveValue('');await expect(page.locator('#chat-draft')).toHaveValue('Keep my conversation');
});

test('denied creation permission and cancelling the dialog preserve dirty text without writing',async({page})=>{
  await installNativeFixtures(page);
  await page.evaluate(()=>{window.currentFolder=makeTestDirectory('project',{'draft.txt':'Original'});window.showDirectoryPicker=async()=>currentFolder;return OrchestrixFiles.openFolder();});
  await page.locator('[data-file-path="draft.txt"]').click();await page.locator('.file-text-editor').fill('Keep my unsaved text');
  await page.evaluate(()=>{window.fileAccess='denied';});await openNewFileDialog(page);await page.locator('#new-file-dialog [name="filename"]').fill('blocked.txt');await page.locator('[data-new-file-create]').click();
  await expect(page.locator('[data-new-file-error]')).toBeVisible();await expect(page.locator('#new-file-dialog')).toBeVisible();
  expect(await page.evaluate(()=>({created:fileOperations.created,writes:fileOperations.writes}))).toEqual({created:[],writes:[]});
  await page.locator('#new-file-dialog [data-new-file-cancel]').click();await expect(page.locator('#new-file-dialog')).toBeHidden();await expect(page.locator('.file-text-editor')).toHaveValue('Keep my unsaved text');await expect(page.locator('.file-dirty')).toBeVisible();
  await expect(page.locator('#chat-draft')).toHaveValue('Keep my conversation');
});

test('native folder includes dotfiles, grants write access only on save, and preserves drafts across Chat',async ({page})=>{
  await page.evaluate(()=>{
    window.fileDisk='TOKEN=initial\n';window.filePermissionRequests=0;
    const handle={name:'.env',kind:'file',getFile:async()=>new File([window.fileDisk],'.env'),queryPermission:async()=> 'prompt',requestPermission:async()=>{window.filePermissionRequests++;return 'granted';},createWritable:async()=>({write:async text=>{window.fileDisk=text;},close:async()=>{}})};
    window.showDirectoryPicker=async()=>({name:'project',kind:'directory',async *values(){yield handle;}});
    return OrchestrixFiles.openFolder();
  });
  await expect(page.locator('[data-file-path=".env"]')).toBeVisible();
  expect(await page.evaluate(()=>window.filePermissionRequests)).toBe(0);
  await page.locator('[data-file-path=".env"]').click();
  await page.locator('.file-text-editor').fill('TOKEN=changed\n');
  await page.locator('[data-file-chat]').click();
  await expect(page.locator('#chat-draft')).toHaveValue('Keep my conversation');
  await page.locator('[data-file-tab=".env"]').click();
  await expect(page.locator('.file-text-editor')).toHaveValue('TOKEN=changed\n');
  await page.locator('[data-file-save]').click();
  expect(await page.evaluate(()=>window.fileDisk)).toBe('TOKEN=changed\n');
  expect(await page.evaluate(()=>window.filePermissionRequests)).toBe(1);
  await expect(page.locator('.file-save-status')).toHaveText('No pending changes');
});

test('imported files save as a download without pretending to write the source file',async ({page})=>{
  await importFile(page,'notes.md','# Original','project/docs/notes.md');
  await page.locator('[data-file-path="docs"]').click();
  await page.locator('[data-file-path="docs/notes.md"]').click();
  await expect(page.locator('[data-file-save]')).toHaveText('Save copy');
  await page.locator('.file-text-editor').fill('# Edited');
  const downloadPromise=page.waitForEvent('download');
  await page.locator('[data-file-save]').click();
  const download=await downloadPromise;
  expect(download.suggestedFilename()).toBe('notes.md');
  await expect(page.locator('#files-notification')).toHaveText('File copy prepared for download.');
  const stream=await download.createReadStream();let received='';for await(const chunk of stream)received+=chunk.toString();
  expect(received).toBe('# Edited');
});

test('Markdown preview formats headings while keeping HTML and script URLs inert',async ({page})=>{
  await importFile(page,'report.md','# Result\n<script>window.fileXss=1</script>\n<img src=x onerror="window.fileXss=2">\n[click](javascript:window.fileXss=3)\n**Important**\n```html\n<iframe src=evil>\n```');
  await page.locator('[data-file-path="report.md"]').click();
  await page.locator('[data-file-view="preview"]').click();
  await expect(page.locator('.file-markdown-preview h1')).toHaveText('Result');
  await expect(page.locator('.file-markdown-preview strong')).toHaveText('Important');
  await expect(page.locator('.file-markdown-preview script,.file-markdown-preview img,.file-markdown-preview iframe,.file-markdown-preview a')).toHaveCount(0);
  await expect(page.locator('.file-markdown-preview')).toContainText('<script>window.fileXss=1</script>');
  expect(await page.evaluate(()=>window.fileXss)).toBeUndefined();
});

test('closing dirty files requires explicit discard and permission failures retain the draft',async ({page})=>{
  await page.evaluate(()=>{
    const file={name:'.env',kind:'file',getFile:async()=>new File(['ORIGINAL=yes'],'.env'),queryPermission:async()=> 'denied',requestPermission:async()=> 'denied'};
    window.showDirectoryPicker=async()=>({name:'project',async *values(){yield file;}});
    return OrchestrixFiles.openFolder();
  });
  await page.locator('[data-file-path=".env"]').click();
  await page.locator('.file-text-editor').fill('RETAIN=this');
  await page.locator('[data-file-save]').click();
  await expect(page.locator('#files-notification')).toHaveText('Write permission was not granted.');
  await expect(page.locator('.file-text-editor')).toHaveValue('RETAIN=this');
  page.once('dialog',dialog=>dialog.dismiss());
  await page.locator('[data-file-close=".env"]').click();
  await expect(page.locator('.file-text-editor')).toHaveValue('RETAIN=this');
  page.once('dialog',dialog=>dialog.accept());
  await page.locator('[data-file-close=".env"]').click();
  await expect(page.locator('#file-editor')).toBeHidden();
});

test('binary and oversized files are rejected without losing the current text',async ({page})=>{
  await page.evaluate(()=>OrchestrixFiles.importFiles([new File(['safe'],'ok.txt'),new File([new Uint8Array([0,1,2])],'binary.dat'),new File(['x'.repeat(2*1024*1024+1)],'big.txt')]));
  await page.locator('[data-file-path="ok.txt"]').click();
  await page.locator('[data-file-path="binary.dat"]').click();
  await expect(page.locator('#files-notification')).toHaveText('This file type cannot be edited as text.');
  await page.locator('[data-file-path="big.txt"]').click();
  await expect(page.locator('#files-notification')).toHaveText('This file exceeds the 2 MB editing limit.');
  await expect(page.locator('.file-text-editor')).toHaveValue('safe');
});

test('native directories load on demand and do not traverse dependency folders',async ({page})=>{
  await page.evaluate(()=>{
    window.directoryVisits=0;
    const dependency={name:'node_modules',kind:'directory',async *values(){window.directoryVisits++;yield {name:'dependency.js',kind:'file',getFile:async()=>new File(['export {}'],'dependency.js')};}};
    const rootFile={name:'README.md',kind:'file',getFile:async()=>new File(['# Project'],'README.md')};
    window.showDirectoryPicker=async()=>({name:'project',async *values(){yield dependency;yield rootFile;}});
    return OrchestrixFiles.openFolder();
  });
  expect(await page.evaluate(()=>window.directoryVisits)).toBe(0);
  await expect(page.locator('[data-file-path="README.md"]')).toBeVisible();
  await page.locator('[data-file-path="node_modules"]').click();
  await expect(page.locator('[data-file-path="node_modules/dependency.js"]')).toBeVisible();
  expect(await page.evaluate(()=>window.directoryVisits)).toBe(1);
});

test('Escape releases Tab navigation and preview preserves the editing selection',async ({page})=>{
  await importFile(page,'notes.md','# Notes\nKeep this text');
  await page.locator('[data-file-path="notes.md"]').click();
  await page.locator('.file-text-editor').evaluate(input=>{input.focus();input.setSelectionRange(12,16);input.dispatchEvent(new Event('select'));});
  await page.locator('[data-file-view="preview"]').click();
  await page.locator('[data-file-view="edit"]').click();
  expect(await page.locator('.file-text-editor').evaluate(input=>[input.selectionStart,input.selectionEnd])).toEqual([12,16]);
  await page.locator('.file-text-editor').focus();
  await page.keyboard.press('Escape');
  await page.keyboard.press('Tab');
  await expect(page.locator('.file-text-editor')).not.toBeFocused();
  await expect(page.locator('.file-text-editor')).toHaveValue('# Notes\nKeep this text');
});

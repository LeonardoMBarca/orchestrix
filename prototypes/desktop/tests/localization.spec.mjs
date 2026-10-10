import {test,expect} from '@playwright/test';
import {openStudio,openSettings} from './navigation.mjs';

const storageKey='orchestrix-prototype-language';
const labels={
  en:{chat:'Chat',connections:'Connections',language:'Interface language',commands:'Search sessions, projects and actions',search:'Search actions',settings:'Settings',query:'settings'},
  'pt-BR':{chat:'Conversa',connections:'Conexões',language:'Idioma da interface',commands:'Buscar sessões, projetos e ações',search:'Buscar ações',settings:'Configurações',query:'configurações'},
  es:{chat:'Conversación',connections:'Conexiones',language:'Idioma de la interfaz',commands:'Buscar sesiones, proyectos y acciones',search:'Buscar acciones',settings:'Configuración',query:'configuración'}
};
const composer=page=>page.locator('#chat-message');
const userText=page=>page.locator('.chat-message.from-user > p');
test.beforeEach(async ({page})=>{await page.addInitScript(()=>{window.__ORCHESTRIX_TEST_SCENARIO='seed';});});
async function chooseLanguage(page,language,keepOpen=false) {
  await openSettings(page,'general');
  await page.locator('#settings-language').focus();await page.locator('#settings-language').selectOption(language);
  await expect(page.locator('html')).toHaveAttribute('lang',language);
  await expect(page.locator('#settings-language')).toHaveValue(language);
  await expect(page.locator('#settings-language')).toHaveAccessibleName(labels[language].language);
  if(!keepOpen)await page.locator('[data-settings-close]').click();
}
async function view(page,name) {
  if(name==='settings'){await openSettings(page);return;}
  if(await page.locator('#floating-settings').isVisible())await page.locator('[data-settings-close]').click();
  if(!['chat','connections'].includes(name))await openStudio(page);
  await page.locator(`.nav-item[data-view="${name}"]`).click();
  await expect(page.locator(`.nav-item[data-view="${name}"]`)).toHaveAttribute('aria-current','page');
}
async function send(page,message) {await composer(page).fill(message);await composer(page).press('Enter');await expect(userText(page).last()).toHaveText(message);}
async function projectForm(page) {await page.evaluate(()=>OrchestrixApp.openProject());await expect(page.locator('#session-project-form')).toBeVisible();}

for(const browserLocale of ['pt-BR','es-MX']) {
  test(`primeira visita usa inglês mesmo com navegador ${browserLocale}`,async ({browser})=>{
    const context=await browser.newContext({locale:browserLocale,baseURL:test.info().project.use.baseURL});
    try {
      const page=await context.newPage();await page.goto('/');
      expect(await page.evaluate(()=>navigator.language)).toBe(browserLocale);
      await expect(page.locator('html')).toHaveAttribute('lang','en');
      await expect(page.getByRole('button',{name:'Chat',exact:true})).toBeVisible();
      await expect(page.getByRole('button',{name:'Connections',exact:true})).toBeVisible();
      await expect(page.locator('#interface-language')).toHaveCount(0);await expect(composer(page)).toBeVisible();
      await expect(page.getByRole('heading',{level:1})).not.toHaveText('O que vamos criar hoje?');
      await openSettings(page);await expect(page.locator('#settings-language')).toHaveValue('en');
      expect(await page.evaluate(key=>localStorage.getItem(key),storageKey)).toBeNull();
    } finally {await context.close();}
  });
}

test('idioma em Settings preserva preferências não salvas e sobrevive a reload e novas sessões',async ({page})=>{
  await page.goto('/');await chooseLanguage(page,'pt-BR');await openSettings(page,'orchestration');
  await page.locator('#settings-routing-routing').selectOption('availability');
  await page.locator('#settings-routing-account').selectOption('claude');
  await page.locator('#settings-routing-thinking').selectOption('high');
  await chooseLanguage(page,'es',true);await expect(page.locator('#settings-language')).toBeFocused();
  await page.locator('[data-settings-tab="orchestration"]').click();
  await expect(page.locator('#settings-routing-routing')).toHaveValue('availability');
  await expect(page.locator('#settings-routing-account')).toHaveValue('claude');
  await expect(page.locator('#settings-routing-thinking')).toHaveValue('high');
  expect(await page.evaluate(key=>localStorage.getItem(key),storageKey)).toBe('es');
  await page.reload();await expect(page.locator('html')).toHaveAttribute('lang','es');
  await page.locator('.new-session-trigger').click();
  await expect(page.locator('html')).toHaveAttribute('lang','es');await expect(composer(page)).toBeVisible();
  await chooseLanguage(page,'en',true);await expect(page.locator('#settings-language')).toHaveValue('en');
  await page.reload();await expect(page.locator('html')).toHaveAttribute('lang','en');
});

test('trocar idioma preserva projeto, caminho, sessão, mensagens literais e rascunho',async ({page})=>{
  await page.goto('/');await projectForm(page);await page.locator('#entry-name').fill('Conversa');
  const path='C:\\Projetos\\Conversa & Papers';await page.locator('#entry-path').fill(path);
  await page.locator('#session-project-form button[type="submit"]').click();
  await expect(page.locator('#modal')).toBeHidden();await send(page,'Preferências');
  const message='Conversa "original" & <img data-locale-injection src=x onerror="window.__localeInjected=true">';
  await send(page,message);
  const draft='Conversa & <b>"rascunho"</b> sem tradução';await composer(page).fill(draft);
  await composer(page).evaluate(element=>element.setSelectionRange(7,19,'backward'));
  const identity=await page.evaluate(()=>({project:OrchestrixApp.getState().projectId,session:OrchestrixApp.getState().chat.id}));
  const tasks=await page.locator('.chat-task-card').count();
  for(const language of ['pt-BR','es','en']) {
    await chooseLanguage(page,language);
    await expect(page.locator('#project-name')).toHaveText('Conversa');await expect(page.locator('#breadcrumb-project')).toHaveText('Conversa');
    expect(await page.evaluate(()=>({project:OrchestrixApp.getState().projectId,session:OrchestrixApp.getState().chat.id}))).toEqual(identity);
    await expect(page.locator(`.session-item[data-session-id="${identity.session}"]`)).toHaveAttribute('aria-current','page');
    await expect(userText(page).first()).toHaveText('Preferências');await expect(userText(page).last()).toHaveText(message);
    await expect(page.locator('.chat-task-card')).toHaveCount(tasks);await expect(composer(page)).toHaveValue(draft);
    expect(await composer(page).evaluate(element=>[element.selectionStart,element.selectionEnd,element.selectionDirection])).toEqual([7,19,'backward']);
    expect(await page.evaluate(()=>OrchestrixApp.getState().projectPath)).toBe(path);
    await expect(page.locator('[data-locale-injection]')).toHaveCount(0);expect(await page.evaluate(()=>window.__localeInjected===true)).toBe(false);
  }
});

test('rótulos e workspace de uma conta personalizada permanecem seguros e literais',async ({page})=>{
  await page.goto('/');await view(page,'connections');await page.locator('[data-conn-action="add"]').click();
  const name='Preferências & <b data-locale-injection>"Conta"</b>';
  await page.locator('#conn-name').fill(name);await page.locator('#conn-workspace').fill('Conversa');
  await page.locator('#conn-add-form button[type="submit"]').click();await expect(page.locator('#modal')).toBeHidden();
  const count=await page.locator('.conn-row').count();
  for(const language of ['es','pt-BR','en']) {
    await chooseLanguage(page,language);await expect(page.locator('.conn-row')).toHaveCount(count);
    const account=page.getByRole('article',{name,exact:true});await expect(account).toHaveCount(1);
    await expect(account).toContainText(name);await expect(account).toContainText('Conversa');await expect(page.locator('[data-locale-injection]')).toHaveCount(0);
  }
});

test('alterar idioma mantém o arquivo selecionado e o código do resultado',async ({page})=>{
  await page.goto('/');await view(page,'work');await page.locator('[data-tab="code"]').click();
  const code=await page.locator('.diff-lines').textContent();const path=await page.locator('.diff-header').textContent();
  expect(code).toMatch(/\S/);expect(path).toContain('src/auth/token-service.ts');
  for(const language of ['es','pt-BR','en']) {
    await chooseLanguage(page,language);await expect(page.locator('[data-view="work"]')).toHaveAttribute('aria-current','page');
    await expect(page.locator('[data-tab="code"]')).toHaveAttribute('aria-selected','true');
    expect(await page.locator('.diff-lines').textContent()).toBe(code);expect(await page.locator('.diff-header').textContent()).toBe(path);
  }
});

test('formulário de projeto mantém nome, caminho, contexto e seleção durante a troca de idioma',async ({page})=>{
  await page.goto('/');await projectForm(page);
  const name='Conversa & <b data-locale-injection>"A"</b>',path='C:\\Projetos\\Conversa & Research',context='Conversa: explicar papers & <b>preservar conteúdo</b>.';
  await page.locator('#entry-name').fill(name);await page.locator('#entry-path').fill(path);await page.locator('#entry-context').fill(context);
  await page.locator('#entry-context').evaluate(element=>element.setSelectionRange(3,14,'backward'));
  for(const language of ['es','pt-BR','en']) {
    // The project form is a native dialog; translation must also preserve its draft.
    await page.evaluate(language=>OrchestrixI18n.setLanguage(language),language);
    await expect(page.locator('#entry-name')).toHaveValue(name);await expect(page.locator('#entry-path')).toHaveValue(path);await expect(page.locator('#entry-context')).toHaveValue(context);
    expect(await page.locator('#entry-context').evaluate(element=>[element.selectionStart,element.selectionEnd,element.selectionDirection])).toEqual([3,14,'backward']);
    await expect(page.locator('[data-locale-injection]')).toHaveCount(0);
  }
});

test('idioma e busca de ações funcionam por teclado com nomes acessíveis traduzidos',async ({page})=>{
  await page.goto('/');await openSettings(page);await page.locator('#settings-language').focus();
  await page.keyboard.press('Home');await page.keyboard.press('ArrowDown');
  await expect(page.locator('html')).toHaveAttribute('lang','pt-BR');await expect(page.locator('#settings-language')).toBeFocused();await page.locator('[data-settings-close]').click();
  for(const language of ['en','pt-BR','es']) {
    await chooseLanguage(page,language);await expect(page.locator('[data-action="commands"]')).toHaveAccessibleName(labels[language].commands);
    await page.locator('[data-action="commands"]').focus();await page.keyboard.press('Control+k');await expect(page.locator('#command-search')).toBeFocused();
    await expect(page.locator('#command-search')).toHaveAccessibleName(labels[language].search);await page.locator('#command-search').fill(labels[language].query);
    await expect(page.locator('#command-list button')).toHaveCount(1);await expect(page.locator('[data-command="app-settings"]')).toHaveText(labels[language].settings);
    await page.keyboard.press('ArrowDown');await expect(page.locator('[data-command="app-settings"]')).toBeFocused();await page.keyboard.press('Enter');
    await expect(page.locator('#modal')).toBeHidden();await expect(page.locator('#floating-settings')).toBeVisible();
    await expect(page.locator('#settings-language')).toHaveAccessibleName(labels[language].language);await expect(page.locator('#settings-language')).toHaveValue(language);
    await page.locator('[data-settings-close]').click();
  }
});

for(const language of ['en','es']) {
  test(`${language}: chat refluído em 320px e desktop com texto a 200%`,async ({page})=>{
    await page.goto('/');await chooseLanguage(page,language);await openSettings(page);await page.locator('#settings-text-size').fill('200');await page.locator('[data-settings-close]').click();
    const message='ConversaSinEspaciosWithNoSpaces'.repeat(70).slice(0,2000);await page.setViewportSize({width:320,height:900});await send(page,message);
    for(const width of [320,1920]) {
      await page.setViewportSize({width,height:900});for(const selector of ['#chat-message','#chat-form button[type="submit"]'])await expect(page.locator(selector)).toBeVisible();
      await page.evaluate(()=>OrchestrixDock.activate('work'));await expect(page.locator('.chat-task-card').last()).toBeVisible();
      expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
      await openSettings(page);await expect(page.locator('#settings-language')).toBeVisible();await expect(page.locator('#floating-settings')).toBeVisible();
      expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);await page.locator('[data-settings-close]').click();
    }
  });
  test(`${language}: catálogo completo traduz navegação, painéis, configurações e diálogos`,async ({page})=>{
    test.setTimeout(45000);await page.goto('/');await chooseLanguage(page,language);
    expect(await page.evaluate(()=>Object.entries(OrchestrixLocaleCatalog).filter(([,entry])=>!['en','es'].every(locale=>typeof entry[locale]==='string'&&entry[locale].trim())).map(([source])=>source))).toEqual([]);
    await page.evaluate(()=>OrchestrixI18n.clearMissing());await view(page,'chat');
    await expect(page.locator('.nav-item[data-view="chat"]')).toHaveText(labels[language].chat);await expect(page.locator('[data-view="connections"]')).toHaveText(labels[language].connections);
    await expect(page.getByRole('heading',{level:1})).not.toHaveText('O que vamos criar hoje?');
    await view(page,'work');for(const tab of ['summary','code','checks','activity'])await page.locator(`[data-tab="${tab}"]`).click();
    for(const name of ['attention','review','history','connections','settings'])await view(page,name);
    await expect(page.locator('#settings-language')).toHaveAccessibleName(labels[language].language);
    for(const tab of ['accounts','themes','conversation','orchestration','layout'])await page.locator(`[data-settings-tab="${tab}"]`).click();
    await view(page,'connections');await page.locator('[data-conn-action="add"]').click();await expect(page.locator('#conn-name')).not.toHaveAccessibleName('Nome para reconhecer a conta');await page.keyboard.press('Escape');
    await projectForm(page);await expect(page.locator('#entry-name')).not.toHaveAccessibleName('Nome do novo projeto');await page.keyboard.press('Escape');
    await page.keyboard.press('Control+k');await expect(page.locator('#command-search')).toHaveAccessibleName(labels[language].search);await page.keyboard.press('Escape');
    expect(await page.evaluate(()=>OrchestrixI18n.missing())).toEqual([]);
  });
}

test('preferência inválida usa inglês e pode ser substituída pelo usuário',async ({page})=>{
  await page.addInitScript(key=>localStorage.setItem(key,'de-DE'),storageKey);await page.goto('/');await expect(page.locator('html')).toHaveAttribute('lang','en');
  await expect(page.getByRole('button',{name:'Chat',exact:true})).toBeVisible();await chooseLanguage(page,'es');expect(await page.evaluate(key=>localStorage.getItem(key),storageKey)).toBe('es');
});
test('armazenamento indisponível permite usar e trocar idioma sem erros',async ({page})=>{
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.addInitScript(()=>{for(const method of ['getItem','setItem'])Object.defineProperty(Storage.prototype,method,{configurable:true,value(){throw new DOMException('Storage blocked','SecurityError');}});});
  await page.goto('/');await expect(page.locator('html')).toHaveAttribute('lang','en');await chooseLanguage(page,'es');await expect(page.getByRole('button',{name:'Conversación',exact:true})).toBeVisible();
  await send(page,'Conversa & "sin almacenamiento"');await page.reload();await expect(page.locator('html')).toHaveAttribute('lang','en');await expect(composer(page)).toBeVisible();expect(errors).toEqual([]);
});
test('site e página de vídeo continuam em inglês com o app configurado em português',async ({page})=>{
  await page.goto('/');await chooseLanguage(page,'pt-BR');await page.goto('/website.html');await expect(page.locator('html')).toHaveAttribute('lang','en');
  await expect(page.getByRole('link',{name:/^Download Orchestrix/}).first()).toBeVisible();await expect(page.getByText('Coming soon',{exact:true})).toHaveCount(3);
  await page.locator('a[href="watch.html"]').first().click();await expect(page.locator('html')).toHaveAttribute('lang','en');await expect(page.locator('#video-placeholder')).toContainText('The film is on its way');await expect(page.locator('#product-video')).toBeHidden();
  await page.goto('/');await expect(page.locator('html')).toHaveAttribute('lang','pt-BR');await expect(page.getByRole('button',{name:'Conversa',exact:true})).toBeVisible();
});

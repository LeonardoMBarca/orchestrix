import {test,expect} from '@playwright/test';
import {openStudio,usePortuguese} from './navigation.mjs';

test.beforeEach(async ({page})=>{await usePortuguese(page);await page.goto('/');await navigation(page);await openStudio(page);});

const transcript=page=>page.locator('.chat-transcript[role="log"]');
const composer=page=>page.locator('#chat-message');
const userMessages=page=>transcript(page).getByRole('article',{name:'Você',exact:true});
const taskCard=(page,id)=>page.getByRole('article',{name:`Trabalho ${id}`,exact:true});
const projectIdentity=page=>page.evaluate(()=>OrchestrixApp.getState().projectId);
const conversationIdentity=page=>page.evaluate(()=>OrchestrixApp.getState().chat.id);
async function navigation(page){await page.evaluate(()=>OrchestrixDock.activate('navigation'));}
async function newSession(page){await page.evaluate(()=>OrchestrixDock.activate('sessions'));await page.locator('.new-session-trigger').click();}
async function selectTree(page,projectId,chatId) {
  await page.evaluate(()=>OrchestrixDock.activate('sessions'));
  const group=page.locator(`.session-project[data-project-id="${projectId}"]`);
  if(await group.count()&&!await group.evaluate(element=>element.open))await group.locator('summary').click();
  const selected=chatId?page.locator(`[data-session-id="${chatId}"]`):page.locator(`[data-session-project="${projectId}"]`).first();
  await selected.click();
  await page.evaluate(()=>OrchestrixDock.activate('work'));
}

async function conversation(page) {
  const close=page.locator('[data-settings-close]');if(await close.isVisible())await close.click();
  await navigation(page);
  await page.getByRole('button',{name:'Conversa',exact:true}).click();
  await expect(page.locator('.chat-transcript')).toBeAttached();
  await page.evaluate(()=>OrchestrixDock.activate('work'));
}
async function send(page,text) {
  await composer(page).fill(text);
  await page.getByRole('button',{name:'Enviar',exact:true}).click();
  await expect(userMessages(page).last()).toContainText(text);
  await page.evaluate(()=>OrchestrixDock.activate('work'));
}
async function request(page,text) {
  await send(page,text);
  const title=text.split('\n')[0].slice(0,100);
  const card=page.getByRole('article').filter({has:page.getByRole('heading',{level:3,name:title,exact:true})});
  await expect(card).toHaveCount(1);
  const identity=await card.getAttribute('aria-label');
  expect(identity).toMatch(/^Trabalho OX-\d+$/);
  return {id:identity.slice('Trabalho '.length),card};
}
async function inspectContext(page,card) {
  await card.getByText('Contexto e escolhas deste trabalho',{exact:true}).click();
  await card.getByRole('button',{name:'Inspecionar contexto e arquivos',exact:true}).click();
  await expect(page.getByRole('dialog')).toHaveAccessibleName('Contexto e arquivos da tentativa');
  return page.getByRole('dialog');
}
async function closeContext(page) {
  await page.getByRole('dialog').getByRole('button',{name:'Voltar',exact:true}).click();
  await expect(page.getByRole('dialog')).not.toBeVisible();
}
async function reviewedRequest(page,text) {
  const work=await request(page,text);
  await work.card.getByRole('button',{name:'Iniciar trabalho',exact:true}).click();
  await work.card.getByRole('button',{name:'Concluir trabalho',exact:true}).click();
  await expect(work.card).toContainText('Artefato r1 · tentativa produtora 1');
  return work;
}
async function prepareAnotherProject(page,name='orchestrix',path='C:\\Projetos\\Outro espaço') {
  await page.evaluate(()=>OrchestrixDock.activate('sessions'));
  await page.getByRole('button',{name:'Novo projeto',exact:true}).click();
  await page.getByRole('textbox',{name:'Nome do novo projeto',exact:true}).fill(name);
  await page.getByRole('textbox',{name:'Diretório de trabalho',exact:true}).fill(path);
  await page.getByRole('button',{name:'Criar projeto e sessão',exact:true}).click();
  await expect(page.getByRole('dialog')).not.toBeVisible();
  await expect(composer(page)).toBeVisible();
}

test('conversa é a entrada e apresenta começo limpo sem avisos de demonstração',async ({page})=>{
  await expect(page.getByRole('button',{name:'Conversa',exact:true})).toHaveAttribute('aria-current','page');
  await expect(page.locator('.chat-transcript')).toBeAttached();
  await expect(page.locator('.chat-transcript article')).toHaveCount(0);
  await expect(composer(page)).toBeVisible();
  await expect(page.locator('.welcome-stage')).toBeVisible();
  await expect(page.getByRole('heading',{level:1})).toHaveText('O que vamos criar hoje?');
  await expect(page.getByText(/Nenhum agente real é executado neste protótipo\./)).toHaveCount(0);
  await expect(page.getByText('Sobre esta demonstração',{exact:true})).toHaveCount(0);
  await expect(page.getByRole('button',{name:'Novo trabalho nesta conversa',exact:true})).toHaveCount(0);
});

test('Enter envia, Shift Enter mantém linhas, e composição ou texto vazio não enviam',async ({page})=>{
  const input=composer(page);
  await input.fill('   ');
  await input.press('Enter');
  await page.evaluate(()=>OrchestrixDock.activate('work'));
  await expect(userMessages(page)).toHaveCount(0);

  await input.fill('Adicionar validação');
  await input.press('Shift+Enter');
  await input.press('t');
  await expect(input).toHaveValue('Adicionar validação\nt');
  await expect(userMessages(page)).toHaveCount(0);
  await input.dispatchEvent('keydown',{key:'Enter',code:'Enter',isComposing:true,keyCode:229,bubbles:true});
  await expect(userMessages(page)).toHaveCount(0);
  await expect(input).toHaveValue('Adicionar validação\nt');

  await input.press('Enter');
  await expect(userMessages(page)).toHaveCount(1);
  await page.evaluate(()=>OrchestrixDock.activate('work'));
  await expect(userMessages(page).first()).toContainText('Adicionar validação\nt');
  await expect(input).toHaveValue('');
  await expect(input).toBeFocused();
  const card=page.getByRole('article').filter({has:page.getByRole('heading',{name:'Adicionar validação',exact:true})});
  await expect(card).toContainText('Pronta para iniciar');
  await expect(card.getByRole('button',{name:'Iniciar trabalho',exact:true})).toBeVisible();
});

test('pedido livre permanece texto seguro e enviar não inicia nem aplica',async ({page})=>{
  const text='Confira "aspas" & <img id="chat-injected" src="missing.png" onerror="window.__chatInjected=true"><button data-action="reset">Ação injetada</button>';
  const {id,card}=await request(page,text);
  await expect(userMessages(page).last()).toContainText(text);
  await expect(page.locator('#chat-injected')).toHaveCount(0);
  await expect(page.getByRole('button',{name:'Ação injetada',exact:true})).toHaveCount(0);
  expect(await page.evaluate(()=>window.__chatInjected===true)).toBe(false);
  await expect(card).toContainText('Pronta para iniciar');
  await expect(card.getByRole('button',{name:'Concluir trabalho',exact:true})).toHaveCount(0);
  await expect(card.getByRole('button',{name:'Revisar aplicação',exact:true})).toHaveCount(0);

  await card.getByRole('button',{name:'Ver tarefa',exact:true}).click();
  const detail=page.getByRole('region',{name:'Tarefa selecionada'});
  await expect(detail).toContainText(id);
  await expect(detail).toContainText('Tentativa 1');
  await expect(detail).toContainText('Pronta para iniciar');
  await expect(detail).toContainText(text);
  await page.getByRole('button',{name:'Histórico',exact:true}).click();
  await expect(page.getByRole('heading',{level:1})).toHaveText('Histórico de decisões e resultados');
  await expect(page.locator('#chat-injected')).toHaveCount(0);
  await page.getByRole('button',{name:'Conexões',exact:true}).click();
  await expect(page.getByRole('heading',{level:1})).toHaveText('Contas distintas, escolhas claras');
  await page.getByRole('button',{name:'Alterações',exact:true}).click();
  await expect(page.getByRole('heading',{level:1})).toHaveText('Examine o resultado da tarefa');
  await conversation(page);
  await expect(userMessages(page).last()).toContainText(text);
  await expect(taskCard(page,id)).toContainText('Pronta para iniciar');
  expect(await page.evaluate(()=>window.__chatInjected===true)).toBe(false);
});

test('orientação e novas preferências não alteram o contexto ou tentativa em andamento',async ({page})=>{
  const {id,card}=await request(page,'Revisar validação de endereço com testes.');
  await card.getByRole('button',{name:'Iniciar trabalho',exact:true}).click();
  let context=await inspectContext(page,card);
  await expect(context).toContainText(`CP-${id}-1 · c1`);
  const snapshot=await context.innerText();
  await closeContext(page);

  await navigation(page);await page.locator('.app-settings-trigger').click();await page.locator('[data-settings-tab="orchestration"]').click();
  await page.getByLabel('Raciocínio preferido').selectOption('medium');
  await page.getByLabel('Contexto inicial').selectOption('module');
  await page.getByRole('button',{name:'Salvar orquestração',exact:true}).click();
  await conversation(page);
  await send(page,'Preserve o comportamento e confira os casos vazios.');
  await expect(card).toContainText('Em execução');
  await expect(card.getByRole('button',{name:'Iniciar trabalho',exact:true})).toHaveCount(0);
  context=await inspectContext(page,card);
  expect(await context.innerText()).toBe(snapshot);
  await closeContext(page);
  await card.getByText('Contexto e escolhas deste trabalho',{exact:true}).click();
  await expect(card).toContainText('Raciocínio solicitado: Alto');
  await expect(card).toContainText(`CP-${id}-1 · c1`);
});

test('pedido livre percorre correção versionada, aceite e aplicação explícita e preserva o trabalho anterior',async ({page})=>{
  test.setTimeout(60000);
  const objective='Adicionar estados vazios à lista de produtos e verificar os casos sem itens.';
  const feedback='Preserve a mensagem acessível quando a lista estiver vazia.';
  const {id,card}=await request(page,objective);
  await card.getByRole('button',{name:'Iniciar trabalho',exact:true}).click();
  await expect(card).toContainText('Em execução');
  await card.getByRole('button',{name:'Concluir trabalho',exact:true}).click();
  await expect(card).toContainText('Artefato r1 · tentativa produtora 1');
  await expect(card).toContainText('Aguardando sua revisão');

  await send(page,feedback);
  await expect(card).toContainText('Artefato r1 · tentativa produtora 1');
  await card.getByRole('button',{name:'Pedir correção',exact:true}).click();
  let review=page.getByRole('dialog');
  await expect(review).toHaveAccessibleName(`Revisão da tarefa ${id}`);
  await expect(review).toContainText(objective);
  await expect(review).toContainText('docs/requested-change.md');
  await expect(review).toContainText('Artefato r1 · tentativa produtora 1');
  await expect(review.getByLabel('Correção deste resultado')).toHaveValue(feedback);
  await review.getByRole('button',{name:'Voltar',exact:true}).click();
  await expect(card).toContainText('Aguardando sua revisão');
  await expect(card).toContainText('Artefato r1 · tentativa produtora 1');

  await card.getByRole('button',{name:'Pedir correção',exact:true}).click();
  await page.getByRole('dialog').getByRole('button',{name:'Pedir correção desta tarefa',exact:true}).click();
  await expect(card).toContainText('Em execução');
  const context=await inspectContext(page,card);
  await expect(context).toContainText(`CP-${id}-2 · c2`);
  await expect(context).toContainText('Instrução desta correção');
  await expect(context).toContainText(feedback);
  await closeContext(page);
  await expect(card).toContainText(`CP-${id}-1 · c1`);
  await expect(card).toContainText('Artefato r1 · tentativa produtora 1');
  await card.getByRole('button',{name:'Concluir trabalho',exact:true}).click();
  await expect(card).toContainText('Artefato r2 · tentativa produtora 2');
  await card.getByRole('button',{name:'Revisar resultado',exact:true}).click();
  review=page.getByRole('dialog');
  await expect(review).toContainText('Artefato r2 · tentativa produtora 2');
  await expect(review).toContainText(feedback);
  await review.getByRole('button',{name:'Validar resultado complementar',exact:true}).click();
  await expect(card).toContainText('Validada no Run');
  await expect(card).not.toContainText('Aplicado · simulação');
  await send(page,'Pode aplicar o resultado.');
  await expect(card).toContainText('Validada no Run');
  await expect(card).not.toContainText('Aplicado · simulação');
  await card.getByRole('button',{name:'Revisar aplicação',exact:true}).click();
  await expect(page.getByRole('dialog')).toContainText('tentativa produtora 2');
  await page.getByRole('dialog').getByRole('button',{name:'Aplicar este Run na demonstração',exact:true}).click();
  await expect(card).toContainText('Aplicado · simulação');

  await page.getByRole('button',{name:'Novo trabalho nesta conversa',exact:true}).click();
  const {card:second}=await request(page,'Revisar a paginação da lista para dez itens por página.');
  await expect(second).toContainText('Pronta para iniciar');
  await expect(card).toContainText('Aplicado · simulação');
  await expect(card).toContainText('Artefato r2 · tentativa produtora 2');
  await expect(transcript(page)).toContainText(objective);
  await expect(transcript(page)).toContainText(feedback);
  await card.getByRole('button',{name:'Ver resultado aplicado',exact:true}).click();
  await expect(page.getByRole('dialog')).toContainText('Artefato r2 · tentativa produtora 2');
  await expect(page.getByRole('dialog')).toContainText(feedback);
  await expect(page.getByRole('dialog')).toContainText('aplicado');
  await page.getByRole('dialog').getByRole('button',{name:'Voltar',exact:true}).click();
  await second.getByRole('button',{name:'Ver tarefa',exact:true}).click();
  await expect(page.getByRole('region',{name:'Tarefa selecionada'})).toContainText('Pronta para iniciar');
  await conversation(page);
  await expect(card).toContainText('Aplicado · simulação');
  await expect(second).toContainText('Pronta para iniciar');
});

test('rascunho, foco e cursor são preservados por evento e navegação avançada',async ({page})=>{
  const input=composer(page);
  await input.fill('Ainda estou explicando a alteração desejada.');
  await input.evaluate(element=>element.setSelectionRange(7,12));
  await page.keyboard.press('Control+Shift+e');
  await expect(input).toBeFocused();
  expect(await input.evaluate(element=>[element.selectionStart,element.selectionEnd])).toEqual([7,12]);
  await expect(input).toHaveValue('Ainda estou explicando a alteração desejada.');
  await expect(userMessages(page)).toHaveCount(0);
  await page.locator('.app-settings-trigger').click();await page.locator('[data-settings-tab="orchestration"]').click();
  await conversation(page);
  await expect(input).toHaveValue('Ainda estou explicando a alteração desejada.');
  await expect(userMessages(page)).toHaveCount(0);
});

test('duas conversas do mesmo projeto isolam mensagens e trabalhos e restauram resultado e rascunho',async ({page})=>{
  test.setTimeout(60000);
  const firstProject=await projectIdentity(page);
  const firstConversation=await conversationIdentity(page);
  const firstObjective='Conferir acessibilidade das mensagens de erro do cadastro.';
  const secondObjective='Simplificar a tela de recuperação de senha.';
  const {id,card}=await reviewedRequest(page,firstObjective);
  await composer(page).fill('Rascunho da primeira conversa.');
  await composer(page).evaluate(element=>element.setSelectionRange(3,11));
  await page.evaluate(()=>OrchestrixDock.activate('sessions'));await page.locator(`.session-project[data-project-id="${firstProject}"] .project-session-actions button`).click();
  const secondConversation=await conversationIdentity(page);
  expect(secondConversation).not.toBe(firstConversation);
  expect(await projectIdentity(page)).toBe(firstProject);
  await expect(userMessages(page)).toHaveCount(0);
  await expect(composer(page)).toHaveValue('');
  await expect(card).toHaveCount(0);
  const {card:second}=await request(page,secondObjective);
  await composer(page).fill('Rascunho da segunda conversa.');
  await composer(page).evaluate(element=>element.setSelectionRange(6,10));

  await selectTree(page,await projectIdentity(page),firstConversation);
  await expect(transcript(page)).toContainText(firstObjective);
  await expect(transcript(page)).not.toContainText(secondObjective);
  await expect(composer(page)).toHaveValue('Rascunho da primeira conversa.');
  expect(await composer(page).evaluate(element=>[element.selectionStart,element.selectionEnd])).toEqual([3,11]);
  await expect(card).toContainText('Artefato r1 · tentativa produtora 1');
  await expect(second).toHaveCount(0);
  const context=await inspectContext(page,card);
  await expect(context).toContainText(`CP-${id}-1 · c1`);
  await expect(context).toContainText(firstObjective);
  await closeContext(page);
  await card.getByRole('button',{name:'Revisar resultado',exact:true}).click();
  await expect(page.getByRole('dialog')).toContainText(firstObjective);
  await expect(page.getByRole('dialog')).not.toContainText(secondObjective);
  await page.getByRole('dialog').getByRole('button',{name:'Voltar',exact:true}).click();

  await selectTree(page,await projectIdentity(page),secondConversation);
  await expect(transcript(page)).toContainText(secondObjective);
  await expect(transcript(page)).not.toContainText(firstObjective);
  await expect(composer(page)).toHaveValue('Rascunho da segunda conversa.');
  expect(await composer(page).evaluate(element=>[element.selectionStart,element.selectionEnd])).toEqual([6,10]);
  await expect(second).toContainText('Pronta para iniciar');
  await expect(card).toHaveCount(0);
});

test('projetos com o mesmo nome têm identidade própria e restauram Run aplicado, contexto e conversa',async ({page})=>{
  const originalProject=await projectIdentity(page);
  const originalConversation=await conversationIdentity(page);
  const objective='Registrar filtros da busca de produtos no histórico local.';
  const {id,card}=await reviewedRequest(page,objective);
  await card.getByRole('button',{name:'Revisar resultado',exact:true}).click();
  await page.getByRole('dialog').getByRole('button',{name:'Validar resultado complementar',exact:true}).click();
  await card.getByRole('button',{name:'Revisar aplicação',exact:true}).click();
  await page.getByRole('dialog').getByRole('button',{name:'Aplicar este Run na demonstração',exact:true}).click();
  await composer(page).fill('Retomar este resultado amanhã.');
  await composer(page).evaluate(element=>element.setSelectionRange(8,13));

  await prepareAnotherProject(page);
  const secondProject=await projectIdentity(page);
  expect(secondProject).not.toBe(originalProject);
  await page.evaluate(()=>OrchestrixDock.activate('sessions'));await expect(page.locator('.session-project')).toHaveCount(2);
  await expect(userMessages(page)).toHaveCount(0);
  await expect(composer(page)).toHaveValue('');
  await expect(card).toHaveCount(0);
  const secondObjective='Adicionar dicas à tela de configurações deste outro projeto.';
  const {card:second}=await request(page,secondObjective);

  await selectTree(page,originalProject);
  expect(await conversationIdentity(page)).toBe(originalConversation);
  await expect(transcript(page)).toContainText(objective);
  await expect(transcript(page)).not.toContainText(secondObjective);
  await expect(card).toContainText('Aplicado · simulação');
  await expect(card).toContainText('Artefato r1 · tentativa produtora 1');
  await expect(composer(page)).toHaveValue('Retomar este resultado amanhã.');
  expect(await composer(page).evaluate(element=>[element.selectionStart,element.selectionEnd])).toEqual([8,13]);
  const context=await inspectContext(page,card);
  await expect(context).toContainText(`CP-${id}-1 · c1`);
  await expect(context).toContainText(objective);
  await closeContext(page);
  await card.getByRole('button',{name:'Ver resultado aplicado',exact:true}).click();
  await expect(page.getByRole('dialog')).toContainText('aplicado');
  await expect(page.getByRole('dialog')).toContainText(objective);
  await page.getByRole('dialog').getByRole('button',{name:'Voltar',exact:true}).click();

  await selectTree(page,secondProject);
  await expect(transcript(page)).toContainText(secondObjective);
  await expect(transcript(page)).not.toContainText(objective);
  await expect(second).toContainText('Pronta para iniciar');
});

test('conversa avulsa permite preparar e revisar mas não aplicar sem destino',async ({page})=>{
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  const originalProject=await projectIdentity(page);
  await newSession(page);
  const looseProject=await projectIdentity(page);
  expect(looseProject).not.toBe(originalProject);
  await expect(composer(page)).toBeVisible();
  await expect(page.locator('[data-action="session-directory"]')).toHaveText('Definir diretório');
  await expect(userMessages(page)).toHaveCount(0);
  await navigation(page);await page.getByRole('button',{name:/^Trabalho\s*\d*$/}).click();
  await expect(page.getByRole('heading',{level:1})).toHaveText('Comece pela conversa');
  await navigation(page);await page.getByRole('button',{name:'Alterações',exact:true}).click();
  await expect(page.getByRole('heading',{level:1})).toHaveText('O resultado aparece quando o trabalho termina');
  await conversation(page);

  const objective='Organizar uma proposta de cache para estudar com a equipe.';
  const {card}=await reviewedRequest(page,objective);
  await card.getByRole('button',{name:'Revisar resultado',exact:true}).click();
  await expect(page.getByRole('dialog')).toContainText(objective);
  await page.getByRole('dialog').getByRole('button',{name:'Validar resultado complementar',exact:true}).click();
  await expect(card).toContainText('Validada no Run');
  await expect(card.getByRole('button',{name:'Revisar aplicação',exact:true})).toHaveCount(0);
  await card.getByRole('button',{name:'Rever resultado',exact:true}).click();
  await expect(page.getByRole('dialog')).toContainText('Destino pendente · conversa avulsa');
  await expect(page.getByRole('dialog').getByRole('button',{name:/Aplicar/})).toHaveCount(0);
  await page.getByRole('dialog').getByRole('button',{name:'Voltar',exact:true}).click();
  await send(page,'Aplique a proposta agora.');
  await expect(card).not.toContainText('Aplicado · simulação');
  await selectTree(page,originalProject);
  await expect(transcript(page)).not.toContainText(objective);
  await selectTree(page,looseProject);
  await expect(transcript(page)).toContainText(objective);
  await expect(card).toContainText('Artefato r1 · tentativa produtora 1');
  await expect(card).not.toContainText('Aplicado · simulação');
  expect(errors).toEqual([]);
});

test('desconexão global não regride ao trocar espaço ou abrir projeto e bloqueia admissão fixa',async ({page})=>{
  const originalProject=await projectIdentity(page);
  await navigation(page);await page.getByRole('button',{name:'Conexões',exact:true}).click();
  const codex=page.locator('.connection-row').filter({hasText:'Codex · pessoal A'});
  await codex.getByRole('button',{name:'Inspecionar conexão',exact:true}).click();
  await page.getByRole('dialog').getByRole('button',{name:'Desconectar para novos trabalhos',exact:true}).click();
  await page.getByRole('dialog').getByRole('button',{name:'Bloquear novos trabalhos',exact:true}).click();
  await expect(page.getByRole('dialog')).toContainText('Não confirmado; nenhuma tentativa foi encerrada');
  await page.getByRole('dialog').getByRole('button',{name:'Fechar',exact:true}).click();
  await conversation(page);
  await newSession(page);
  const looseProject=await projectIdentity(page);
  await navigation(page);await page.locator('.app-settings-trigger').click();await page.locator('[data-settings-tab="orchestration"]').click();
  await page.getByLabel('Conexão preferida').selectOption('codex-a');
  await page.getByRole('button',{name:'Salvar orquestração',exact:true}).click();
  await conversation(page);
  const {card}=await request(page,'Revisar limites de tentativas para uma rotina de importação.');
  await card.getByRole('button',{name:'Iniciar trabalho',exact:true}).click();
  await expect(page.getByRole('heading',{level:1})).toHaveText('Contas distintas, escolhas claras');
  await codex.getByRole('button',{name:'Inspecionar conexão',exact:true}).click();
  await expect(page.getByRole('dialog')).toContainText('Novos trabalhos bloqueados');
  await page.getByRole('dialog').getByRole('button',{name:'Fechar',exact:true}).click();
  await conversation(page);
  await expect(card).toContainText('Pronta para iniciar');
  await expect(card.getByRole('button',{name:'Concluir trabalho',exact:true})).toHaveCount(0);
  await selectTree(page,originalProject);
  await prepareAnotherProject(page,'Outro projeto sem reautorizar','C:\\Projetos\\Sem reautorizar');
  await navigation(page);await page.getByRole('button',{name:'Conexões',exact:true}).click();
  await codex.getByRole('button',{name:'Inspecionar conexão',exact:true}).click();
  await expect(page.getByRole('dialog')).toContainText('Novos trabalhos bloqueados');
  await page.getByRole('dialog').getByRole('button',{name:'Fechar',exact:true}).click();
  await conversation(page);
  await selectTree(page,looseProject);
  await expect(card).toContainText('Pronta para iniciar');
  await card.getByRole('button',{name:'Iniciar trabalho',exact:true}).click();
  await expect(page.getByRole('heading',{level:1})).toHaveText('Contas distintas, escolhas claras');
});

test('chat com texto longo e escala200% mantém seletores, envio e próximo passo em320px e wide',async ({page})=>{
  await navigation(page);await page.locator('.app-settings-trigger').click();await page.locator('[data-settings-tab="orchestration"]').click();
  await page.locator('[data-settings-tab="general"]').click();await page.getByLabel('Tamanho do texto').evaluate(element=>{element.value='200';element.dispatchEvent(new Event('input',{bubbles:true}));});
  await conversation(page);
  const text='SolicitaçãoLongaSemEspaços'.repeat(100).slice(0,2000);
  await page.setViewportSize({width:320,height:800});
  const {card}=await request(page,text);
  await expect(userMessages(page).last()).toContainText(text);
  for(const width of [320,1920]) {
    await page.setViewportSize({width,height:900});
    await page.evaluate(()=>OrchestrixDock.activate('sessions'));await expect(page.locator('#session-list')).toBeVisible();await page.evaluate(()=>OrchestrixDock.activate('work'));
    await expect(composer(page)).toBeVisible();
    await expect(page.getByRole('button',{name:'Enviar',exact:true})).toBeVisible();
    await expect(card.getByRole('button',{name:'Iniciar trabalho',exact:true})).toBeVisible();
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  }
  await page.setViewportSize({width:320,height:800});
  await send(page,'Manter todos os controles acessíveis neste pedido.');
  await expect(card).toContainText('Pronta para iniciar');
  await card.getByRole('button',{name:'Iniciar trabalho',exact:true}).click();
  await expect(card).toContainText('Em execução');
  await expect(card.getByRole('button',{name:'Concluir trabalho',exact:true})).toBeVisible();
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
});

test('mudança de base em outro Run bloqueia a aplicação do resultado livre já validado',async ({page})=>{
  const objective='Adicionar uma opção para ordenar pedidos pelo prazo de entrega.';
  const {card}=await reviewedRequest(page,objective);
  await card.getByRole('button',{name:'Revisar resultado',exact:true}).click();
  await page.getByRole('dialog').getByRole('button',{name:'Validar resultado complementar',exact:true}).click();
  await card.getByRole('button',{name:'Revisar aplicação',exact:true}).click();
  await expect(page.getByRole('dialog')).toContainText('b9f87d3');
  await page.getByRole('dialog').getByRole('button',{name:'Voltar',exact:true}).click();
  await expect(card).toContainText('Validada no Run');
  await expect(card).not.toContainText('Aplicado · simulação');

  await navigation(page);await page.getByRole('button',{name:'Alterações',exact:true}).click();
  await page.getByRole('button',{name:'Validar tarefa no Run…',exact:true}).click();
  await page.getByRole('dialog').getByRole('button',{name:'Validar na demonstração',exact:true}).click();
  await page.getByRole('button',{name:'Simular mudança de base',exact:true}).click();
  await expect(page.getByRole('button',{name:'Revalidar candidato do Run',exact:true})).toBeVisible();
  await conversation(page);
  await expect(card).toContainText('A base do projeto mudou');
  await expect(card).toContainText('Artefato r1 · tentativa produtora 1');
  await expect(card.getByRole('button',{name:'Revisar aplicação',exact:true})).toHaveCount(0);
  await card.getByRole('button',{name:'Ver resultado e base',exact:true}).click();
  const review=page.getByRole('dialog');
  await expect(review).toContainText(objective);
  await expect(review.getByRole('alert')).toContainText('Base alterada · aplicação bloqueada');
  await expect(review.getByRole('alert')).toContainText('b9f87d3');
  await expect(review.getByRole('alert')).toContainText('c3a91f0');
  await expect(review.getByRole('button',{name:/Aplicar|Revisar aplicação/})).toHaveCount(0);
  await review.getByRole('button',{name:'Voltar',exact:true}).click();
  await expect(card).not.toContainText('Aplicado · simulação');
});

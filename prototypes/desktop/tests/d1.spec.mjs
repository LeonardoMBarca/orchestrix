import {test,expect} from '@playwright/test';
import {mkdir} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';

test.beforeEach(async ({page})=>{await page.goto('/');});
async function prepare(page,template='short',agent='codex-a') {
  await page.locator('[data-action="onboarding"]').click();
  await page.getByLabel('Trabalho inicial').selectOption(template);
  await page.getByLabel('Conexão inicial').selectOption(agent);
  await page.getByRole('button',{name:'Preparar projeto de exemplo'}).click();
  await expect(page.getByRole('dialog')).toContainText('Autorização pendente');
  await page.getByRole('button',{name:'Continuar autorização simulada'}).click();
  await page.getByRole('checkbox').check();
  await page.getByRole('button',{name:'Validar registro simulado'}).click();
}
async function completePrimary(page) {
  await page.locator('[data-task="OX-24"]').click();
  await page.getByRole('button',{name:'Iniciar demonstração',exact:true}).click();
  await page.getByRole('button',{name:'Concluir trabalho simulado'}).click();
  await page.locator('[data-view="review"]').click();
  await page.getByRole('button',{name:'Validar tarefa no Run…'}).click();
  await page.getByRole('button',{name:'Validar na demonstração',exact:true}).click();
}

test('entrada curta usa uma conexão com confirmação e percorre o ciclo completo',async ({page})=>{
  await prepare(page,'short','claude');
  await expect(page.locator('.task-item')).toHaveCount(1);
  await expect(page.locator('.detail-footer')).toContainText('Claude · pessoal');
  await page.locator('[data-view="review"]').click();
  await expect(page.locator('h1')).toHaveText('O resultado aparece quando o trabalho termina');
  await page.locator('[data-view="work"]').click();
  await completePrimary(page);
  await page.getByRole('button',{name:'Revisar aplicação do Run…'}).click();
  await page.getByRole('button',{name:'Aplicar na demonstração',exact:true}).click();
  await expect(page.getByRole('button',{name:'Run aplicado na demonstração'})).toBeDisabled();
  await page.locator('[data-view="connections"]').click();
  await expect(page.locator('.connection-row')).toHaveCount(1);
});

test('feature bloqueia aplicação até as quatro tarefas serem validadas',async ({page})=>{
  await prepare(page,'feature');
  await page.getByRole('button',{name:'Nova tarefa',exact:true}).click();
  await page.getByLabel('Título da tarefa').fill('Outro Run independente');
  await page.getByLabel('Objetivo e critérios').fill('Esta tarefa não pertence ao candidato de R-08.');
  await page.getByRole('button',{name:'Criar tarefa simulada'}).click();
  await completePrimary(page);
  await expect(page.locator('.review-inspector')).toContainText('aguardando 3 tarefa(s)');
  await expect(page.getByRole('button',{name:'Revisar aplicação do Run…'})).toBeDisabled();
  await page.getByRole('button',{name:'Simular mudança de base'}).click();
  await page.getByRole('button',{name:'Revalidar candidato do Run'}).click();
  await expect(page.getByRole('button',{name:'Revisar aplicação do Run…'})).toBeDisabled();
  await page.locator('[data-view="work"]').click();
  await page.locator('[data-task="OX-27"]').click();
  await expect(page.getByRole('button',{name:'Iniciar demonstração',exact:true})).toBeDisabled();
  for(const id of ['OX-25','OX-26','OX-27']) {
    await page.locator(`[data-task="${id}"]`).click();
    if(id==='OX-26') {
      await page.getByRole('button',{name:'Responder decisão',exact:true}).first().click();
      await page.getByLabel('Destinos após o login').fill('/admin, /perfil');
      await page.getByRole('button',{name:'Registrar decisão simulada'}).click();
    }
    await page.getByRole('button',{name:'Iniciar demonstração',exact:true}).click();
    await page.getByRole('button',{name:'Concluir resultado complementar simulado'}).click();
    await page.getByRole('button',{name:'Revisar resultado complementar',exact:true}).click();
    await expect(page.getByRole('dialog')).toContainText(`${id} · R-08`);
    if(id==='OX-26')await expect(page.getByRole('dialog')).toContainText('/admin');
    await page.getByRole('button',{name:'Validar resultado complementar'}).click();
  }
  await page.locator('[data-view="review"]').click();
  await expect(page.locator('.review-inspector')).toContainText('5 arquivos');
  await page.getByRole('button',{name:'session-policy.md',exact:true}).click();
  await expect(page.locator('.diff')).toContainText('docs/session-policy.md');
  await expect(page.locator('.diff')).toContainText('/admin, /perfil');
  await page.locator('[data-view="work"]').click();
  await page.locator('[data-task="OX-24"]').click();
  await page.getByRole('tab',{name:'Código',exact:true}).click();
  await expect(page.locator('.diff-header')).toContainText('src/auth/token-service.ts');
  await page.locator('[data-view="review"]').click();
  await page.getByRole('button',{name:'Revisar aplicação do Run…'}).click();
  await expect(page.getByRole('dialog')).toContainText('OX-24, OX-25, OX-26, OX-27');
  await expect(page.getByRole('dialog')).not.toContainText('OX-28');
  await page.getByRole('button',{name:'Aplicar na demonstração',exact:true}).click();
  await expect(page.getByRole('button',{name:'Run aplicado na demonstração'})).toBeDisabled();
  await page.locator('[data-view="work"]').click();
  await page.locator('[data-task="OX-28"]').click();
  await page.getByRole('button',{name:'Iniciar demonstração',exact:true}).click();
  await page.getByRole('button',{name:'Concluir resultado complementar simulado'}).click();
  await page.getByRole('button',{name:'Revisar resultado complementar',exact:true}).click();
  await page.getByRole('button',{name:'Validar resultado complementar'}).click();
  await page.getByRole('button',{name:'Rever resultado e aplicação'}).click();
  await page.getByRole('button',{name:'Revisar aplicação deste Run'}).click();
  await expect(page.getByRole('dialog')).toContainText('R-12');
  await page.getByRole('button',{name:'Aplicar este Run na demonstração'}).click();
  await page.locator('[data-view="review"]').click();
  await expect(page.getByRole('button',{name:'Run aplicado na demonstração'})).toBeDisabled();
});

test('contexto futuro não altera pacote e revisão conserva contexto do produtor',async ({page})=>{
  await page.getByText('Conta, modelo, raciocínio e contexto',{exact:true}).click();
  await page.getByRole('button',{name:'Inspecionar contexto e arquivos'}).click();
  await expect(page.getByRole('dialog')).toContainText('CP-OX-24-1 · c1');
  await page.locator('[data-context-source="policy"]').click();
  await expect(page.locator('.context-preview')).toContainText('fixture-policy-v1');
  await page.getByRole('button',{name:'Preparar contexto para nova tentativa'}).click();
  await page.locator('input[value="test"]').uncheck();
  await page.getByRole('button',{name:'Salvar contexto futuro'}).click();
  await page.getByRole('button',{name:'Inspecionar contexto e arquivos'}).click();
  await expect(page.locator('.context-source')).toHaveCount(3);
  await page.getByRole('button',{name:'Voltar',exact:true}).click();
  await page.locator('[data-view="review"]').click();
  await page.getByLabel('O que precisa mudar?').fill('Preserve auditoria.');
  await page.getByRole('button',{name:'Pedir correção',exact:true}).click();
  await page.getByText('Conta, modelo, raciocínio e contexto',{exact:true}).click();
  await page.getByRole('button',{name:'Inspecionar contexto e arquivos'}).click();
  await expect(page.getByRole('dialog')).toContainText('CP-OX-24-2 · c2');
  await expect(page.getByRole('dialog')).toContainText('Instrução desta correção');
  await expect(page.getByRole('dialog')).toContainText('Preserve auditoria.');
  await expect(page.locator('.context-source')).toHaveCount(2);
  await page.getByRole('button',{name:'Voltar',exact:true}).click();
  await page.locator('[data-view="review"]').click();
  await page.locator('.review-checks summary').click();
  await page.getByRole('button',{name:'Contexto da revisão'}).click();
  await expect(page.getByRole('dialog')).toContainText('CP-OX-24-1 · c1');
  await expect(page.getByRole('dialog')).toContainText('mesma conexão Codex · pessoal A');
});

test('nova autorização e desconexão preservam tentativa ativa e não afirmam revogação',async ({page})=>{
  await page.locator('[data-view="connections"]').click();
  await page.getByRole('button',{name:'Adicionar conexão simulada'}).click();
  await page.getByLabel('Nome para reconhecer a conta').fill('Codex extra');
  await page.getByLabel('Workspace de exemplo').fill('Equipe fictícia');
  await page.getByRole('button',{name:'Adicionar registro pendente'}).click();
  await page.locator('.connection-row').filter({hasText:'Codex extra'}).getByRole('button',{name:'Inspecionar conexão'}).click();
  await page.getByRole('button',{name:'Continuar autorização simulada'}).click();
  await page.getByRole('checkbox').check();
  await page.getByRole('button',{name:'Validar registro simulado'}).click();
  await page.locator('[data-conn-id="codex-a"][data-conn-action="inspect"]').click();
  await page.getByRole('button',{name:'Desconectar para novos trabalhos'}).click();
  await page.getByRole('button',{name:'Bloquear novos trabalhos',exact:true}).click();
  await expect(page.getByRole('dialog')).toContainText('Não confirmado; nenhuma tentativa foi encerrada');
  await page.getByRole('button',{name:'Simular revogação sem resposta'}).click();
  await expect(page.getByRole('dialog')).toContainText('Não confirmada · sem resposta');
  await page.getByRole('button',{name:'Fechar',exact:true}).click();
  await page.locator('[data-view="work"]').click();
  await page.locator('[data-task="OX-25"]').click();
  await expect(page.locator('.detail-meta')).toContainText('Tentativa 1');
  await expect(page.locator('.detail-footer')).toContainText('Claude · pessoal');
  await expect(page.locator('.detail-meta .badge')).toHaveText('Em execução');
});

test('limite e catálogo incompatível bloqueiam início sem fallback para API',async ({page})=>{
  await prepare(page);
  await page.locator('[data-view="connections"]').click();
  await page.getByRole('button',{name:'Inspecionar conexão',exact:true}).click();
  await page.locator('.conn-scenarios summary').click();
  await page.getByRole('button',{name:'Simular descoberta em andamento'}).click();
  await expect(page.getByRole('dialog')).toContainText('Descobrindo catálogo · simulação');
  await page.locator('.conn-scenarios summary').click();
  await page.getByRole('button',{name:'Concluir descoberta simulada'}).click();
  await page.locator('.conn-scenarios summary').click();
  await page.getByRole('button',{name:'Simular limite',exact:true}).click();
  await page.getByRole('button',{name:'Fechar',exact:true}).click();
  await page.locator('[data-view="work"]').click();
  await page.getByRole('button',{name:'Iniciar demonstração',exact:true}).click();
  await expect(page.locator('h1')).toHaveText('Contas distintas, escolhas claras');
  await page.getByRole('button',{name:'Inspecionar conexão',exact:true}).click();
  await page.getByRole('button',{name:'Gerenciar uso simulado'}).click();
  await page.getByRole('button',{name:'Simular limite liberado'}).click();
  await page.locator('.conn-scenarios summary').click();
  await page.getByRole('button',{name:'Simular catálogo incompatível'}).click();
  await expect(page.getByRole('dialog')).toContainText('Nenhum fallback para API');
  await page.getByRole('button',{name:'Fechar',exact:true}).click();
  await page.locator('[data-view="work"]').click();
  await page.getByRole('button',{name:'Iniciar demonstração',exact:true}).click();
  await expect(page.locator('h1')).toHaveText('Contas distintas, escolhas claras');
});

test('permissão recusada e cancelamento incerto mantêm controle sem duplicar tentativa',async ({page})=>{
  await page.locator('[data-task="OX-25"]').click();
  await page.getByRole('button',{name:'Simular pedido de permissão'}).click();
  await page.getByRole('button',{name:'Revisar permissão',exact:true}).first().click();
  await page.getByRole('button',{name:'Recusar na demonstração'}).click();
  await expect(page.locator('.detail-meta .badge')).toHaveText('Permissão necessária');
  await page.getByRole('button',{name:'Revisar permissão',exact:true}).first().click();
  await page.getByRole('button',{name:'Permitir na demonstração'}).click();
  await page.getByRole('button',{name:'Cancelar',exact:true}).click();
  await page.getByRole('button',{name:'Simular cancelamento sem resposta'}).click();
  await expect(page.locator('.detail-meta .badge')).toHaveText('Execução sem confirmação');
  await expect(page.getByRole('button',{name:'Retomar',exact:true})).toHaveCount(0);
  await page.getByRole('button',{name:'Reconciliar demonstração',exact:true}).first().click();
  await expect(page.locator('.detail-meta .badge')).toHaveText('Cancelada');
  await expect(page.locator('.detail-meta')).toContainText('Tentativa 1');
});

test('novo evento preserva arquivo, foco, cursor e posição; candidato alterado invalida aprovação',async ({page})=>{
  await page.locator('[data-view="review"]').click();
  await page.getByRole('button',{name:'token-service.test.ts',exact:true}).click();
  const input=page.getByLabel('O que precisa mudar?');
  await input.fill('Confira este resultado.');
  await input.evaluate(el=>el.setSelectionRange(7,7));
  const scroll=await page.evaluate(()=>window.scrollY);
  await page.keyboard.press('Control+Shift+e');
  await expect(input).toBeFocused();
  expect(await input.evaluate(el=>el.selectionStart)).toBe(7);
  expect(await page.evaluate(()=>window.scrollY)).toBe(scroll);
  await expect(page.locator('[data-file="test"]')).toHaveAttribute('aria-pressed','true');
  await page.getByRole('button',{name:'Validar tarefa no Run…'}).click();
  await page.getByRole('button',{name:'Validar na demonstração',exact:true}).click();
  await page.getByRole('button',{name:'Revisar aplicação do Run…'}).click();
  await page.getByRole('button',{name:'Simular candidato alterado'}).click();
  await expect(page.getByRole('button',{name:'Aplicar na demonstração',exact:true})).toBeDisabled();
  await expect(page.getByRole('alert')).toContainText('O candidato mudou');
});

test('layout por teclado persiste; texto ampliado e entrada WSL permanecem navegáveis',async ({page})=>{
  const separator=page.getByRole('separator',{name:'Largura da fila de tarefas'});
  const box=await separator.boundingBox();
  await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();
  await page.mouse.move(box.x+box.width/2+50,box.y+box.height/2);await page.mouse.up();
  await expect(separator).toHaveAttribute('aria-valuenow','335');
  await separator.focus();await page.keyboard.press('End');
  await expect(separator).toHaveAttribute('aria-valuenow','420');
  await page.getByRole('button',{name:'Recolher tarefas'}).click();
  await expect(page.locator('.worklist')).toBeHidden();
  await page.getByRole('button',{name:'Mostrar tarefas'}).click();
  await page.getByRole('button',{name:'Personalizar aparência'}).click();
  await page.getByLabel('Densidade da interface').selectOption('compact');
  await page.getByLabel('Tamanho do texto').selectOption('large');
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-density','compact');
  await expect(page.locator('html')).toHaveAttribute('data-text-size','large');
  for(const width of [1920,1280,1024,720,390,320]) {
    await page.setViewportSize({width,height:800});
    for(const view of ['work','review','connections','settings']) {
      await page.locator(`[data-view="${view}"]`).click();
      expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
    }
  }
  await page.locator('[data-action="onboarding"]').click();
  await page.getByLabel('Ambiente de execução').selectOption('wsl');
  await expect(page.getByLabel('Caminho do repositório · exemplo')).toHaveValue('/home/leo/Meu projeto – sessão');
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
});

test('modelo nomeado automático seleciona catálogo elegível e conexão fixa incompatível bloqueia',async ({page})=>{
  await page.locator('[data-view="settings"]').click();
  await page.getByLabel('Estratégia de modelo').selectOption('claude-example');
  await page.getByRole('button',{name:'Salvar preferências simuladas'}).click();
  await page.locator('[data-view="work"]').click();
  await page.getByRole('button',{name:'Nova tarefa',exact:true}).click();
  await page.getByLabel('Título da tarefa').fill('Usar modelo Claude ilustrativo');
  await page.getByLabel('Objetivo e critérios').fill('Selecionar uma conexão compatível.');
  await page.getByRole('button',{name:'Criar tarefa simulada'}).click();
  await expect(page.locator('.detail-footer')).toContainText('Claude · pessoal');
  await page.locator('[data-view="settings"]').click();
  await page.getByLabel('Conexão preferida').selectOption('codex-a');
  await expect(page.getByLabel('Estratégia de modelo')).toHaveValue('claude-example');
  await page.getByRole('button',{name:'Salvar preferências simuladas'}).click();
  await page.locator('[data-view="work"]').click();
  await page.getByRole('button',{name:'Nova tarefa',exact:true}).click();
  await page.getByLabel('Título da tarefa').fill('Catálogo incompatível');
  await page.getByLabel('Objetivo e critérios').fill('A escolha fixa não permite trocar conta silenciosamente.');
  await page.getByRole('button',{name:'Criar tarefa simulada'}).click();
  await page.getByRole('button',{name:'Iniciar demonstração',exact:true}).click();
  await expect(page.locator('h1')).toHaveText('Contas distintas, escolhas claras');
});

test('capturas dos novos estados D1',async ({page})=>{
  const folder=fileURLToPath(new URL('../artifacts/',import.meta.url));await mkdir(folder,{recursive:true});
  await page.locator('[data-action="onboarding"]').click();
  await page.screenshot({path:`${folder}d1-entry.png`,fullPage:true});
  await page.locator('[data-view="connections"]').click();
  await page.locator('[data-conn-id="codex-b"][data-conn-action="inspect"]').click();
  await page.screenshot({path:`${folder}d1-connection.png`,fullPage:true});
  await page.getByRole('button',{name:'Fechar',exact:true}).click();
  await page.locator('[data-view="work"]').click();
  await page.getByText('Conta, modelo, raciocínio e contexto',{exact:true}).click();
  await page.getByRole('button',{name:'Inspecionar contexto e arquivos'}).click();
  await page.screenshot({path:`${folder}d1-context.png`,fullPage:true});
});

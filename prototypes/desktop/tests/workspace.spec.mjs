import {test,expect} from '@playwright/test';
import {mkdir} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';

test.beforeEach(async ({page}) => {await page.goto('/');});

async function openReview(page) {await page.locator('[data-view="review"]').click();}
async function chooseTheme(page,id) {
  const names={studio:'Studio',atelier:'Atelier',horizon:'Horizon','deep-black':'Deep Black',medieval:'Medieval',forest:'Forest',dawn:'Dawn'};
  await page.getByRole('button',{name:'Personalizar aparência'}).click();
  await page.getByRole('radio',{name:names[id],exact:true}).check();
  await page.locator('[data-view="work"]').click();
}
async function validateTask(page) {
  await openReview(page);
  await page.getByRole('button',{name:'Validar tarefa no Run…',exact:true}).click();
  await expect(page.getByRole('dialog')).toContainText('branch interna de R-08');
  await page.getByRole('button',{name:'Validar na demonstração',exact:true}).click();
  await expect(page.getByRole('button',{name:'Revisar aplicação do Run…',exact:true})).toBeEnabled();
}

test('cria tarefa com snapshot próprio e trata conteúdo do usuário como texto',async ({page}) => {
  await page.getByRole('button',{name:'Nova tarefa',exact:true}).click();
  const title='<img src=x onerror="window.unexpected=true">';
  await page.getByLabel('Título da tarefa').fill(title);
  await page.getByLabel('Objetivo e critérios').fill('Criar o registro de auditoria e documentar o comportamento.');
  await page.getByRole('button',{name:'Criar tarefa simulada'}).click();
  await expect(page.locator('.detail-title h2')).toHaveText(title);
  await expect(page.locator('.detail-meta')).toContainText('OX-28 · R-12');
  await expect(page.getByRole('button',{name:'Iniciar demonstração',exact:true})).toBeEnabled();
  expect(await page.evaluate(()=>window.unexpected)).toBeUndefined();
  await page.getByRole('button',{name:'Iniciar demonstração',exact:true}).click();
  await expect(page.locator('.detail-meta .badge')).toHaveText('Em execução');
});

test('inspecionar política não salva e novas preferências preservam tentativas existentes',async ({page}) => {
  await page.locator('[data-view="settings"]').click();
  await page.getByLabel('Raciocínio preferido').selectOption('medium');
  await page.getByRole('button',{name:'Inspecionar política'}).click();
  await expect(page.getByRole('dialog')).toContainText('Raciocínio: high');
  await page.getByRole('button',{name:'Fechar',exact:true}).click();
  await page.getByRole('button',{name:'Salvar preferências simuladas'}).click();
  await page.locator('[data-view="work"]').click();
  await page.getByText('Conta, modelo, raciocínio e contexto',{exact:true}).click();
  await expect(page.locator('.context-inspector')).toContainText('Alto / desconhecido');
  await page.getByRole('button',{name:'Nova tarefa',exact:true}).click();
  await page.getByLabel('Título da tarefa').fill('Nova configuração');
  await page.getByLabel('Objetivo e critérios').fill('Usar as novas preferências.');
  await page.getByRole('button',{name:'Criar tarefa simulada'}).click();
  await page.getByText('Conta, modelo, raciocínio e contexto',{exact:true}).click();
  await expect(page.locator('.context-inspector')).toContainText('Médio / desconhecido');
});

test('aprovação expira com mudança de base; revalidar Git não cria tentativa de agente',async ({page}) => {
  await validateTask(page);
  await page.getByRole('button',{name:'Revisar aplicação do Run…',exact:true}).click();
  await expect(page.getByRole('dialog')).toContainText('main · b9f87d3');
  await page.getByRole('button',{name:'Simular base alterada',exact:true}).click();
  await expect(page.getByRole('button',{name:'Aplicar na demonstração',exact:true})).toBeDisabled();
  await expect(page.getByRole('alert')).toContainText('confirmação expirou');
  await page.getByRole('button',{name:'Fechar',exact:true}).click();
  await page.getByRole('button',{name:'Revalidar candidato do Run'}).click();
  await expect(page.locator('.page-head')).toContainText('produzido pela tentativa 1');
  await expect(page.locator('.review-inspector')).toContainText('i2');
  await page.getByRole('button',{name:'Revisar aplicação do Run…',exact:true}).click();
  await expect(page.getByRole('dialog')).toContainText('main · c3a91f0');
  await page.getByRole('button',{name:'Aplicar na demonstração',exact:true}).click();
  await expect(page.getByRole('button',{name:'Run aplicado na demonstração'})).toBeDisabled();
  await page.locator('[data-view="history"]').click();
  await expect(page.locator('.timeline')).toContainText('R-08 · aplicação simulada concluída');
  await expect(page.locator('.timeline')).toContainText('nenhuma aplicação em main');
});

test('check falho impede aceite e correção preserva proveniência e rascunho entre arquivos',async ({page}) => {
  await openReview(page);
  await page.getByRole('button',{name:'Simular verificação falha'}).click();
  await expect(page.getByRole('button',{name:'Validar tarefa no Run…',exact:true})).toBeDisabled();
  await expect(page.locator('.review-checks')).toContainText('Exit 1');
  await page.getByLabel('O que precisa mudar?').fill('Corrija a revogação e preserve auditoria.');
  await page.getByRole('button',{name:'token-service.test.ts',exact:true}).click();
  await expect(page.getByLabel('O que precisa mudar?')).toHaveValue('Corrija a revogação e preserve auditoria.');
  await page.getByRole('button',{name:'Pedir correção',exact:true}).click();
  await expect(page.locator('.detail-meta')).toContainText('Tentativa 2');
  await openReview(page);
  await expect(page.locator('.page-head')).toContainText('produzido pela tentativa 1');
  await expect(page.locator('.page-head')).toContainText('tentativa 2 em andamento');
  await page.getByRole('button',{name:'Voltar ao trabalho'}).click();
  await page.getByRole('button',{name:'Concluir correção simulada'}).click();
  await openReview(page);
  await expect(page.locator('.page-head')).toContainText('Artefato r2 · produzido pela tentativa 2');
  await expect(page.getByRole('button',{name:'Validar tarefa no Run…',exact:true})).toBeEnabled();
});

test('sinal perdido exige reconciliação e é diferente de pausa',async ({page}) => {
  await page.locator('[data-task="OX-25"]').click();
  await page.getByRole('button',{name:'Simular perda de sinal'}).click();
  await expect(page.locator('.detail-meta .badge')).toHaveText('Execução sem confirmação');
  await expect(page.getByRole('button',{name:'Retomar',exact:true})).toHaveCount(0);
  await page.getByRole('button',{name:'Reconciliar demonstração',exact:true}).first().click();
  await expect(page.locator('.detail-meta .badge')).toHaveText('Em execução');
  await page.getByRole('button',{name:'Pausar',exact:true}).click();
  await expect(page.locator('.detail-meta .badge')).toHaveText('Pausada');
});

test('ações e abas operam por teclado; fechar modal restaura foco',async ({page}) => {
  await page.getByRole('button',{name:'Buscar ações do workspace'}).focus();
  await page.keyboard.press('Control+k');
  await expect(page.getByLabel('Buscar ações',{exact:true})).toBeFocused();
  await page.getByLabel('Buscar ações',{exact:true}).fill('preferências');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Enter');
  await expect(page.locator('h1')).toHaveText('Seu jeito de orquestrar');
  await page.locator('[data-view="work"]').click();
  await page.getByRole('tab',{name:'Resumo',exact:true}).focus();
  await page.keyboard.press('ArrowRight');
  await expect(page.getByRole('tab',{name:'Código',exact:true})).toBeFocused();
  await expect(page.getByRole('tabpanel')).toContainText('Artefato r1');
  await page.getByRole('button',{name:'Nova tarefa',exact:true}).click();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('button',{name:'Nova tarefa',exact:true})).toBeFocused();
});

test('layouts refluem e direções preservam legibilidade básica',async ({page}) => {
  const external=[];
  page.on('request',request=>{if(!request.url().startsWith('http://127.0.0.1:4173/'))external.push(request.url());});
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  for(const direction of ['studio','atelier','horizon','deep-black','medieval','forest','dawn']) {
    await chooseTheme(page,direction);
    for(const viewport of [{width:1920,height:1080},{width:1280,height:800},{width:1024,height:768},{width:720,height:480},{width:390,height:844},{width:320,height:720}]) {
      await page.setViewportSize(viewport);
      await expect(page.getByRole('button',{name:'Nova tarefa',exact:true})).toBeVisible();
      expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
      await openReview(page);
      expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
      await page.locator('[data-view="work"]').click();
    }
    const ratios=await page.evaluate(()=>{
      const css=getComputedStyle(document.documentElement);
      const luminance=hex=>{let c=hex.trim().replace('#','');if(c.length===3)c=[...c].map(n=>n+n).join('');return [0,2,4].map(i=>parseInt(c.slice(i,i+2),16)/255).map(n=>n<=.04045?n/12.92:((n+.055)/1.055)**2.4).reduce((sum,n,i)=>sum+n*[.2126,.7152,.0722][i],0);};
      const ratio=(a,b)=>{const x=luminance(css.getPropertyValue(a)),y=luminance(css.getPropertyValue(b));return (Math.max(x,y)+.05)/(Math.min(x,y)+.05);};
      return [ratio('--text','--panel'),ratio('--muted','--panel'),ratio('--accent','--accent-bg'),ratio('--on-accent','--accent')];
    });
    ratios.forEach(ratio=>expect(ratio).toBeGreaterThanOrEqual(4.5));
  }
  expect(errors).toEqual([]);expect(external).toEqual([]);
});

test('capturas dos temas e estados para inspeção visual',async ({page}) => {
  const destination=fileURLToPath(new URL('../artifacts/',import.meta.url));
  await mkdir(destination,{recursive:true});
  for(const direction of ['studio','atelier','horizon','deep-black','medieval','forest','dawn']){
    await chooseTheme(page,direction);
    await page.reload();
    await page.evaluate(()=>window.scrollTo(0,0));
    await page.screenshot({path:`${destination}${direction}.png`,fullPage:true});
  }
  await chooseTheme(page,'studio');
  await page.reload();
  await page.evaluate(()=>window.scrollTo(0,0));
  await openReview(page);
  await page.screenshot({path:`${destination}review.png`,fullPage:true});
  await page.setViewportSize({width:720,height:480});
  await page.screenshot({path:`${destination}compact.png`,fullPage:true});
  await page.setViewportSize({width:1440,height:960});
  await page.getByRole('button',{name:'Personalizar aparência'}).click();
  await page.locator('#appearance').screenshot({path:`${destination}appearance.png`});
});

test('Studio é padrão; os temas em Preferências persistem sem afetar o trabalho',async ({page}) => {
  await expect(page.locator('html')).toHaveAttribute('data-direction','studio');
  await expect(page.locator('.sidebar select')).toHaveCount(0);
  await page.getByRole('button',{name:'Personalizar aparência'}).click();
  await expect(page.getByRole('radio',{name:'Studio',exact:true})).toBeChecked();
  await page.getByRole('radio',{name:'Medieval',exact:true}).check();
  await expect(page.locator('html')).toHaveAttribute('data-direction','medieval');
  await expect(page.getByRole('radio',{name:'Medieval',exact:true})).toBeChecked();
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-direction','medieval');
  await page.getByRole('button',{name:'Personalizar aparência'}).click();
  await page.getByRole('radio',{name:'Deep Black',exact:true}).check();
  await expect(page.getByRole('radio',{name:'Deep Black',exact:true})).toBeChecked();
  await page.getByRole('button',{name:'Restaurar Studio'}).click();
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-direction','studio');
  await expect(page.locator('.detail-meta')).toContainText('OX-24 · R-08');
});

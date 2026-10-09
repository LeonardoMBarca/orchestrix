# D1 — Piloto inicial de experiência

Roteiro preparado em **8 de outubro de 2026**, após a [conclusão de D0](../research/d0-conclusion.md). **Piloto ainda não executado; gate de D1 aberto.** O piloto individual foi solicitado ao responsável; nenhuma resposta ou sessão humana foi registrada até esta atualização. Os campos de resultados abaixo permanecem em branco para observações reais.

O piloto avalia se o responsável pelo produto consegue conduzir desenvolvimento assistido no aplicativo: formular trabalho, fornecer contexto, acompanhar, ler código/diff, pedir correção e controlar a entrega. O protótipo é uma simulação, sem autenticação, inferência, acesso a arquivos ou operações Git. O resultado deste piloto informa a experiência; os contratos reais continuam em M0/M1/M2/M3.

Referências: [OX-D03/04/05 e dependências](../DEVELOPMENT_BACKLOG.md), [POS-01 a POS-16](../research/product-positioning.md), [ACC-01 a ACC-07](../research/subscription-account-ux.md), [protocolo de avaliação](../research/experience-decisions.md#8-piloto-e-avaliação-de-uso--roteiro-ainda-não-executado), [protótipo](../../prototypes/desktop/README.md) e [matriz dos casos](d1-scenario-matrix.md).

## Responsáveis e decisão

| Papel | Responsabilidade | Registro da sessão |
| --- | --- | --- |
| Participante inicial | Responsável pelo produto; realizar tarefas e explicar o que entende das telas. | Nome/identificação: ______ |
| Facilitador | Apresentar tarefas sem explicar onde clicar; preparar variações; registrar ajuda e erros. | Nome/identificação: ______ |
| Implementação/design | Resolver lacunas de prontidão e problemas observados; vincular alteração e reteste ao mesmo caso. | Responsável: ______ |
| Decisão do gate | Conferir registro, casos pendentes e problemas críticos, com o responsável pelo produto. | Responsável/data: ______ |

Se o responsável fizer um piloto individual guiado por este documento, registrar **piloto individual**, manter os prompts de tarefa separados das notas de facilitação e anotar toda ajuda recebida, inclusive do agente. Não apresentar essa sessão como estudo independente. A avaliação com três a cinco desenvolvedores externos permanece necessária antes de concluir OX-017 em M2.

## Preparação reproduzível

1. Abrir o [protótipo local](../../prototypes/desktop/README.md#abrir) e registrar a versão exata, navegador, viewport e escala. Não usar contas, repositórios privados ou credenciais reais.
2. Escolher Studio em **Personalizar aparência → Preferências → Aparência**, usando **Restaurar Studio**. Usar os mesmos dados para comparar outras aparências no caso P0-09.
3. Usar **Reiniciar demonstração** antes de cada caso independente. Recarregar também reinicia o trabalho e as conexões. Tema, densidade, tamanho do texto e largura da fila persistem; **Restaurar layout** retorna densidade confortável, texto normal e largura inicial. Recolher/mostrar tarefas e modo foco são controles da sessão, sem promessa de persistência. Resetar o trabalho não substitui restaurar as preferências visuais.
4. Conferir os estados abaixo pelas telas. A pessoa de facilitação pode preparar uma variação por controles explícitos de demonstração, mas deve registrar o caminho utilizado. Não substituir uma interação ausente por uma explicação verbal e chamá-la de testada.
5. Consultar a [entrega técnica D1](d1-delivery.md) e conferir que a versão/manifesto usados correspondem ao incremento verificado. A passagem técnica final executou `npm.cmd run check` e a suíte Playwright com **19 cenários aprovados**: dez em `d1.spec.mjs` e nove anteriores em `workspace.spec.mjs`, usando Chrome e Node 24.19.0. Inclui arraste real da fila, teclado, persistência e regressões dos fluxos. Repetir verificações pertinentes se houver mudanças/falhas novas; esse resultado técnico não produz dados humanos nem fecha OX-D05.

**Reiniciar demonstração** conserva o seed com OX-24 em revisão, OX-25 em execução, OX-26 esperando decisão e OX-27 com falha terminal confirmada; seus Runs são independentes. O botão do projeto abre **Começar** e permite preparar um novo cenário: **Correção curta · 1 tarefa**, com uma conexão, ou **Feature guiada · 4 tarefas**, com as quatro tarefas em R-08. Preparar o cenário substitui trabalho/conexões demonstrativos e abre a autorização pendente. A tela não lê o path nem converte caminhos entre Windows e WSL. A feature tem percurso de conclusão/revisão/aceite das tarefas complementares, com fixtures identificadas e aplicação final de cinco arquivos; conferir o roteiro abaixo na passagem técnica antes do piloto.

### Manifesto de cenários da sessão

Os caminhos abaixo correspondem aos controles do incremento D1; preencher versão e confirmar disponibilidade na passagem técnica. Todos os dados são fictícios. O mesmo manifesto deve permitir repetir um reteste. Os caminhos são notas de facilitação e não instruções a entregar antes de observar o participante.

| Variação | Dados/estado necessários | Como preparar na versão avaliada |
| --- | --- | --- |
| V01 — primeira entrada | Projeto `C:\Projetos de exemplo\Equipe São Paulo\serviço de autenticação\orchestrix-demo`; Windows nativo; uma conexão inicialmente pendente, com uso do plano. | Botão do projeto → **Começar** → nome, ambiente, path, **Correção curta · 1 tarefa**, conexão inicial → **Preparar projeto de exemplo** → **Continuar autorização simulada** → checkbox da modalidade → **Validar registro simulado** → Trabalho. |
| V02 — trabalho pequeno e dependente | Uma conexão; correção curta; depois feature de quatro tarefas em R-08. OX-24 aceita enquanto outras tarefas permanecem abertas; OX-27 depende de OX-24 + OX-25 aceitas. | Repetir Começar primeiro com **Correção curta · 1 tarefa** e depois com **Feature guiada · 4 tarefas**. Na curta: iniciar OX-24 → **Concluir trabalho simulado** → revisar/corrigir/validar → aplicação. Na feature: aceitar OX-24 e conferir aplicação bloqueada/OX-27 indisponível → OX-25 iniciar/concluir resultado complementar/revisar/validar → OX-26 responder `/conta, /configuracoes`, iniciar/concluir/revisar/validar → OX-27 iniciar após suas duas dependências aceitas/concluir/revisar/validar → revisão de R-08 com cinco arquivos/quatro tarefas → aplicação. |
| V03 — identidade e catálogo | Duas conexões ChatGPT A/B com mesmo e-mail fictício/rótulos distintos e Claude; modelos nominais de exemplo, descoberta em andamento e incompatibilidade; ambiente WSL no projeto. | Seed → Conexões → **Inspecionar conexão**; **Adicionar conexão simulada** → **Adicionar registro pendente** → autorizar/validar. Na autorização, variar identidade correspondente/divergente. Inspector → **Simular estados desta conexão** → **Simular descoberta em andamento** / **Concluir descoberta simulada** e **Simular catálogo incompatível**. Preferências → conexão + **Estratégia de modelo** → **Modelo Codex de exemplo** ou **Modelo Claude de exemplo**. Para WSL, Começar → **WSL · Linux** + `/home/exemplo/Equipe São Paulo/orchestrix-demo`; não reutilizar o path Windows. |
| V04 — sessões/contexto | Context Pack com fontes/path/versão/trecho; implementação/revisão em sessões separadas da mesma conexão; capacidade compartilhada/desconhecida; contexto produtor preservado enquanto uma correção está ativa. | Trabalho → **Conta, modelo, raciocínio e contexto** → **Inspecionar contexto e arquivos** → fontes → **Preparar contexto para nova tentativa** → selecionar referências → **Salvar contexto futuro**. Altera seleção futura; conferir pacote atual preservado. Alterações → **Verificações e revisão deste artefato** → **Contexto da revisão**. Durante correção, comparar o pacote da tentativa produtora do artefato anterior com o pacote da nova tentativa. |
| V05 — atenção | Pergunta, permissão de comando recusada/permitida, check falho, limite observado e autorização pendente/negada/expirada. | Seed → OX-25 em execução → **Simular pedido de permissão** → **Revisar permissão** → **Recusar na demonstração**; reabrir → **Permitir na demonstração**. OX-26 → responder decisão; revisão → **Simular verificação falha**. Conexões → inspector → **Simular limite**, **Simular expiração**, **Simular acesso negado** e **Reconectar na simulação**; também variar identidade divergente ao autorizar. |
| V06 — revisão/versionamento | OX-24 r1/tentativa 1, dois arquivos, check falho, correção; evento de outra tarefa durante diff/comentário; candidato/base alterados. | Seed com quatro tarefas → Alterações → alternar arquivo e escrever comentário → `Ctrl+Shift+E` sem sair do campo → conferir contador/atenção de outra tarefa → **Simular verificação falha** → pedir correção → Trabalho → **Concluir correção simulada**. Após validar tarefa, confirmação de aplicação → **Simular candidato alterado** ou **Simular base alterada**, em subcasos independentes. |
| V07 — controle incerto | Perda de sinal; cancelamento sem confirmação; encerramento de conexão com tentativa pendente; revogação remota sem confirmação. | Seed → OX-25 → **Simular perda de sinal**. Em variação separada: **Cancelar** → **Simular cancelamento sem resposta** → estado desconhecido → **Reconciliar demonstração**. Conexões → conexão vinculada → **Desconectar para novos trabalhos** → **Simular revogação sem resposta**. |
| V08 — adaptação | Mesmo diff/atividade; ampla/compacta; texto da interface 200%; densidade e painéis; teclado. Zoom navegador/escala Windows registrados separadamente quando usados. | **Personalizar aparência →** → **Densidade da interface** confortável/compacta (alvos-base 44/36px) e **Tamanho do texto → Ampliado · 200%**. Trabalho → **Recolher tarefas/Mostrar tarefas**; separador **Largura da fila de tarefas** por arraste ou `←`/`→`/`Home`/`End`; **Restaurar layout**. |

### Identificação do registro

| Campo | Preencher antes da execução |
| --- | --- |
| Sessão / data / participante / facilitador | ______ |
| Commit + indicação de alterações locais, ou identificador/hash do build | ______ |
| URL local e forma de iniciar/resetar | ______ |
| Sistema / navegador / versão | ______ |
| Viewport CSS / tamanho da janela | ______ |
| Escala física Windows / zoom navegador / ampliação de texto | ______ |
| Dispositivos de entrada / tecnologia assistiva utilizada | ______ |
| Variações prontas / indisponíveis e motivo | ______ |
| Interrupções ou alterações do protótipo durante a sessão | ______ |

**Tamanho do texto → Ampliado · 200%** é um controle da interface. Ele não é zoom do navegador nem escala física do Windows. Registrar cada condição usada separadamente; não generalizar um resultado para as demais. O piloto inclui a ampliação de texto disponível; zoom/escala e tecnologias assistivas não exercitados continuam pendentes para QA do Desktop real.

## Condução e registro

Apresentação inicial: “Este é um estudo simulado do Orchestrix. Queremos entender como as telas ajudam você a conduzir trabalho e controlar o resultado. Explique o que espera acontecer antes de ações que mudam conta, tentativa ou destino. Quando houver dificuldade, registre o que procurou. Estamos avaliando a interface.”

Antes de começar, registrar a rotina atual: agente/editor utilizados, como envia contexto, onde revisa e como aplica uma alteração. Essa descrição serve de comparação qualitativa; não fornece uma medição de produtividade.

Durante cada caso, entregar só o **prompt do participante**. O caminho e o resultado esperados ficam com a facilitação. Não ensinar os rótulos corretos antes de observar a interpretação. Uma pausa para pensar ou exploração alternativa não é automaticamente erro.

Anotar início/fim, caminho percorrido, hesitações, ajuda e ações inesperadas. Classificar ajuda como **nenhuma**, **prompt neutro** (“o que você procuraria?”) ou **orientação direta** (nomear a tela/botão). Se uma ação não existir ou a simulação quebrar, registrar **indisponível/bloqueado por protótipo**. Não registrar como erro do participante. Uma explicação do facilitador depois disso não conta como conclusão da tarefa.

Os casos podem ser divididos em sessões para preservar atenção. Registrar as partes e retomar com o mesmo manifesto. Não impor um tempo de conclusão como critério de usabilidade nesta amostra.

## Casos P0

### P0-01 — Começar e reconhecer conexões

**Preparação:** V01; depois V03 com OX-25 ativa. Reiniciar entre as duas variações.

**Notas de facilitação:** usar Começar e a autorização pendente conforme V01. Depois restaurar o seed para acrescentar uma conta enquanto OX-25 está ativa. Testar identidade divergente em um registro novo, sem sobrescrever o registro ativo. “Gerenciar uso simulado” mostra a intenção de controle; não abre gerenciamento real de assinatura.

**Prompt do participante:** “Abra este projeto de exemplo e prepare uma conexão para trabalhar usando o seu plano. Diga qual modalidade escolheu e onde conferiria o uso. Agora, com um trabalho em andamento, prepare outra conta sem trocar a conta dele. Identifique as contas que têm o mesmo e-mail fictício e o ambiente em que cada conexão executa.”

**Observar:** primeira entrada sem credenciais reais; confirmação inicial da modalidade simulada; identidade/estado perto da entrada de trabalho; acesso a gerenciamento de uso; rótulos estáveis; autorização nova pendente separada da conexão ativa; nenhuma alteração implícita de dono. Windows/WSL e cwd são explícitos. A interface não anuncia que os paths foram lidos nem que o suporte WSL foi validado.

**Perguntas após agir:** “Qual conexão está usando o plano? Qual ainda não foi autorizada? O que mudou na tentativa que já estava ativa? Como você distingue as duas contas de mesmo e-mail? O que seria cobrado pelo aplicativo e pelo provider?”

**Rastreabilidade:** POS-01/16; ACC-01/02/03/04. Autenticação e isolamento reais: M0/M3.

### P0-02 — Trabalho pequeno e objetivo com dependências

**Preparação:** V02 com somente uma conexão elegível.

**Notas de facilitação:** comparar os templates **Correção curta · 1 tarefa** e **Feature guiada · 4 tarefas** em Começar. A primeira execução usa **Iniciar demonstração** e **Concluir trabalho simulado**; uma correção usa **Concluir correção simulada**. Na feature, OX-27 deve explicar suas dependências e ter início bloqueado até OX-24/OX-25 aceitas. Nas outras tarefas, usar **Concluir resultado complementar simulado → Revisar resultado complementar → Validar resultado complementar**. OX-26 exige antes **Responder decisão** com destinos de exemplo. Resultados complementares são fixtures de três arquivos adicionais, sem execução/checks/Git reais. Esse percurso não equivale à agregação real de M2 ou a um planner autônomo.

**Prompt do participante:** “Peça uma correção pequena: preservar o registro de auditoria ao revogar uma sessão, com verificação desse comportamento. Depois prepare uma funcionalidade que depende da implementação antes da revisão e da documentação. Acompanhe o ciclo simulado até a entrega usando a mesma conexão.”

**Observar:** entrada curta por objetivo e critério, sem obrigar plano detalhado; diferença entre tarefa manual e objetivo com dependências; etapa seguinte e razão da espera compreensíveis; ciclo sequencial viável com uma conexão. Um plano demonstrativo não aparece como planner autônomo já implementado.

**Perguntas após agir:** “O que precisa terminar antes da próxima etapa? Para essa correção pequena, o que você teve de planejar? Precisou de uma segunda conta? Qual etapa ainda depende de você?”

**Rastreabilidade:** POS-01/02. Fluxo real manual: M1/M2; DAG/scheduler: M3; geração/replanejamento de plano: M4.

### P0-03 — Escolhas, compatibilidade e snapshots

**Preparação:** V03; uma tentativa ativa e uma nova tarefa pronta para iniciar.

**Notas de facilitação:** Preferências → conexão preferida, **Estratégia de modelo**, raciocínio → salvar. Além de **Preferido pelo runtime** e **Favorito do perfil (exemplo)**, o select oferece **Modelo Codex de exemplo** (`codex-example`) ou **Modelo Claude de exemplo** (`claude-example`), conforme a conexão/catálogo; são nomes de fixtures. Trocar a conexão atualiza as opções, preservando o snapshot de tentativas anteriores. Com conexão automática, observar escolha elegível/compatível; com conexão fixa incompatível, conferir bloqueio do início. Conexões → inspector → **Simular descoberta em andamento** mostra carregamento e bloqueia novas tentativas até **Concluir descoberta simulada**. Nada disso descobre ou executa um modelo real.

**Prompt do participante:** “Use a escolha automática. Depois prefira um modelo de exemplo fixo e mais raciocínio. Antes de salvar, explique o efeito na tentativa ativa. Prepare uma nova tentativa com outra conta e veja o que muda quando essa conta não oferece a opção escolhida ou ainda está descobrindo o catálogo.”

**Observar:** conexão selecionada atualiza catálogo/capabilities; descoberta pendente não parece acesso confirmado; incompatibilidade bloqueia ou explica alternativa. Seleção automática respeita elegibilidade/compatibilidade, enquanto conexão fixa incompatível bloqueia início. Solicitado, resolvido e informado pelo runtime separados, inclusive informação parcial/desconhecida; resumo da decisão tem motivo; tentativa atual mantém snapshot, nova tentativa usa nova preferência. Troca de conta não transporta conversa privada nem ativa API faturada silenciosamente.

**Perguntas após agir:** “O modelo realmente usado é conhecido? De onde veio essa informação? O que significa esse controle de raciocínio? Quais escolhas valem para a tentativa antiga e a nova? A opção aparecer no catálogo garante que sua conta pode usá-la?”

**Rastreabilidade:** POS-03/04/05/13; ACC-06. Capabilities/política/contexto: OX-003/008/016; habilitação multiaccount: M3.

### P0-04 — Contexto, sessões e revisão

**Preparação:** V04; contexto com objetivo, critérios, base, arquivo selecionado e evidências de revisão.

**Notas de facilitação:** seguir fontes do manifesto por path/versão/trecho; alterar referências para a próxima tentativa e reabrir a atual para conferir snapshot. Usar **Contexto da revisão** para comparar sessão e escopo. Durante correção, o artefato anterior e sua revisão conservam fontes/versão, conta e modelo solicitado da tentativa produtora; o histórico identifica a configuração/pacote por tentativa. Conferir se a pessoa distingue essa origem da preferência da tentativa nova. Modelo efetivo e consumo continuam desconhecidos, pois nenhuma inferência ocorre.

**Prompt do participante:** “Confira o que foi enviado para implementar e o que será enviado para revisar este resultado. Encontre a versão das fontes e o que ficou de fora. A implementação e a revisão usam a mesma conta: explique o que é separado e qual capacidade elas compartilham.”

**Observar:** seleção e inspeção de Context Pack, fontes/escopo/versão/omissões, associação com tentativa; revisor recebe diff/evidências pertinentes; sessões distintas não viram contas/cotas extras. Modelo igual é mostrado como igual; diversidade desconhecida permanece desconhecida. Nenhum acesso universal ao projeto ou histórico privado é presumido.

**Perguntas após agir:** “Qual sessão produziu o resultado? Por que esta revisão é separada? Há outro modelo confirmado? Criar outra sessão aumentou o limite? O contexto que você acrescentar agora altera o pacote já enviado?”

**Rastreabilidade:** POS-06/07/14. Pacote e revisão reais: OX-008/013; capacidade/pooling: M0/M3.

### P0-05 — Atenção e recuperação de autorização/limite

**Preparação:** V05, uma variação por vez. Estados de conta e eventos têm causa explícita.

**Notas de facilitação:** autorizações, limite e catálogo são controles de Conexões; os registros podem mudar de elegibilidade sem alterar a tentativa vinculada. Na OX-25 em execução, **Simular pedido de permissão → Revisar permissão** mostra comando, diretório, alcance e tentativa. Recusar mantém o trabalho aguardando; permitir libera somente a solicitação da mesma tentativa. Não apresentar negar autorização de conta como equivalente a recusar esse comando. Permissões de escrita de arquivos não têm uma variação própria neste incremento.

**Prompt do participante:** “Encontre o que precisa de você e faça o trabalho continuar dentro das condições que escolher. Há uma pergunta, uma permissão solicitada, uma conta que chegou ao limite e uma autorização que expirou. Depois veja o que acontece se negar a nova autorização.”

**Observar:** motivo, impacto, tentativa e próxima ação distinguem pergunta/permissão/check/limite/auth. Resposta ou recusa fica vinculada à tentativa; tarefa/contexto preservados. No limite, gerenciamento de uso, espera ou próxima conexão elegível são explícitos; não há promessa de cota independente nem fallback silencioso para API. Negar/renovar autorização não é retry oculto nem transfere uma sessão em andamento.

**Perguntas após cada variação:** “O que está esperando? O que a ação autoriza? Qual tentativa receberá sua resposta? Esperar é diferente de tentar novamente? Há cobrança diferente? O que foi preservado quando a autorização falhou?”

**Rastreabilidade:** POS-08/13; ACC-02/05/06. Classificação/controle real: OX-007/015; autenticação: M0; fallback/capacidade: M3.

### P0-06 — Ler evidência, corrigir e conservar foco

**Preparação:** V06, começando por OX-24 r1/tentativa 1. O facilitador dispara um evento sem pedir que o participante saia do diff.

**Notas de facilitação:** usar o seed com várias tarefas e `Ctrl+Shift+E` enquanto o campo de correção de OX-24 está focado. O evento registra atenção de outra tarefa e atualiza o contador sem renderizar novamente o conteúdo; observar arquivo/foco/rascunho/scroll preservados. Depois, por ação do participante, abrir **Precisa de você** e o item recebido; conferir tentativa/tarefa corretas e retorno à revisão. No cenário de uma única tarefa, o evento recai sobre ela própria e não substitui esta variação de POS-08.

**Prompt do participante:** “Confira este resultado nos dois arquivos. Uma verificação falhou: encontre a evidência e peça uma correção do artefato correspondente. Continue lendo e escrevendo enquanto chega uma nova solicitação de outra tarefa. Depois confira o resultado corrigido antes de aceitar.”

**Observar:** check falho bloqueia aceite; código/diff, critérios, findings e evidência disponíveis dentro do app. Comentário identifica tarefa/artefato; rascunho/arquivo/foco/posição preservados no evento e na troca de arquivo. Nova tentativa não reatribui o artefato antigo; novo resultado exige nova evidência/revisão. Atenção abre a tentativa correta e oferece retorno previsível.

**Perguntas após agir:** “Qual versão falhou e qual você corrigiu? O agente terminar bastava para aplicar? O que continua disponível da tentativa anterior? A notificação fez você perder o lugar? Você precisaria abrir outro editor para entender ou pedir essa correção?”

**Rastreabilidade:** POS-08/09 e POS-07/15. Proveniência/checks/revisão reais: OX-005/006/009/013/015.

### P0-07 — Aceite da tarefa e aplicação do Run

**Preparação:** V02 com duas tarefas no mesmo Run; depois V06 com candidato completo e evidência aprovada. Separar três subcasos: Run incompleto, candidato alterado, base alterada.

**Notas de facilitação:** escolher **Feature guiada · 4 tarefas**; depois do aceite de OX-24, a aplicação deve indicar as tarefas pendentes de R-08. Concluir/revisar/validar OX-25, responder os destinos de OX-26 e concluir/revisar/validar, depois executar OX-27 somente com OX-24/OX-25 aceitas. O candidato completo deve identificar quatro tarefas e cinco arquivos, incluindo os resultados complementares ilustrativos. No diálogo de aplicação, **Simular candidato alterado** e **Simular base alterada** devem desabilitar a confirmação antiga. Fechar, **Revalidar candidato do Run** e abrir uma nova revisão de aplicação. Para isolar as duas invalidações pode-se usar também o cenário curto; ele não substitui o caso de Run incompleto. Não usar os outros Runs independentes do seed como substituto de POS-11.

**Prompt do participante:** “Aceite esta tarefa, deixando outra tarefa do mesmo Run aberta. Explique se isso já entregou o trabalho ao destino. Quando o Run estiver pronto, confira o destino e prepare sua aplicação. Antes da confirmação, o candidato muda; depois, em outra variação, a base avança. Resolva cada situação e confira o resultado que efetivamente pode aplicar.”

**Observar:** aceite interno libera a etapa pertinente sem declarar Run entregue; Run incompleto não habilita aplicação final. Diálogo vincula tarefa(s), tentativa/artefato, candidato, destino, base e checks. Mudança de candidato ou base invalida aprovação antiga; caminho de revalidação e nova revisão identifica exatamente o novo resultado. Aplicação demonstrativa não aparece como efeito Git real.

**Perguntas antes de confirmar:** “O que você está aceitando? O que falta neste Run? Quais arquivos/versão e qual destino serão afetados? A sua aprovação anterior continua válida? Como sabe que o trabalho foi aplicado?”

**Rastreabilidade:** POS-10/11. Integração/aplicação/recuperação reais: OX-006/009/014. A versão inicial com outros Runs independentes não substitui o subcaso de várias tarefas no mesmo Run.

### P0-08 — Controle quando não há confirmação

**Preparação:** V07 sobre OX-25; separar término confirmado de ausência de sinal e revogação remota não confirmada.

**Notas de facilitação:** **Simular perda de sinal** e cancelamento sem resposta são subcasos diferentes. Depois de cancelamento incerto, a reconciliação deve confirmar o cancelamento da mesma tentativa, em vez de iniciar outra. **Desconectar para novos trabalhos** é um bloqueio de novas requisições daquela conexão, não uma pausa global de admissões nem confirmação de término do worker. Pausa global da fila não está implementada neste estudo; seu escopo segue o scheduler global de M3. Avaliar os controles de conexão/tentativa e registrar se a ausência global impede o fluxo do participante, sem declarar cobertura desse recurso.

**Prompt do participante:** “Pare novos trabalhos desta conexão sem perder a tentativa em andamento. Agora interrompa uma tentativa cujo sinal foi perdido. Explique o que sabe sobre o término antes de começar outra. Depois veja como retomar o controle quando não houver confirmação da revogação.”

**Observar:** bloqueio de novos trabalhos da conexão, pausa de turno simulada, cancelamento e desconexão têm escopos distintos; não generalizar para pausa global da fila. Estado incerto/reconciliando conserva tentativa/identidade e não permite duplicação automática do worker. Sair da conta ou parar novas requisições não confirma término do processo nem revogação remota. Confirmações e limites da simulação aparecem na tela.

**Perguntas após agir:** “O processo terminou ou isso ainda é desconhecido? Qual operação ficou proibida enquanto não houver confirmação? Fechar a conexão cancelou a tentativa? Qual autorização foi revogada e qual confirmação ainda falta?”

**Rastreabilidade:** POS-12; ACC-07. Prova terminal/reconciliação: OX-001/007/010/011; lifecycle de autenticação: M0/M3.

### P0-09 — Desenvolver com conforto, teclado e adaptação

**Preparação:** V08, Studio e um tema alternativo com os mesmos dados. Registrar janela, densidade e condição de ampliação efetivos. Comparar ampla/compacta e **Ampliado · 200%** da interface. Outros viewports, zoom navegador e escala física Windows podem ter QA técnica adicional.

**Prompt do participante:** “Continue a revisão usando só o teclado. Abra uma ação pela busca, mude uma aba/arquivo, escreva uma correção, abra e feche uma confirmação e volte ao ponto de trabalho. Depois reduza a janela e amplie o texto da interface a 200%. Organize os painéis para ler confortavelmente, compare a densidade e escolha outra aparência.”

**Notas de facilitação:** testar o separador da fila por arraste e, separadamente, `←`/`→`/`Home`/`End`; recolher/mostrar tarefas; observar densidade confortável/compacta. Recarregar para conferir tema/densidade/texto/largura persistidos. **Restaurar layout** deve recuperar texto normal, largura inicial e densidade confortável; **Restaurar Studio** restaura o tema. Não declarar escala Windows 200% testada por usar o controle de texto da interface.

**Observar:** `Tab`/`Shift+Tab`, setas nas abas, `Ctrl+K`, `Esc`, retorno de foco e foco visível; atalhos não interceptam texto digitado. Modais/diálogos e logs/diff extensos conservam ações acessíveis. Reflow, recolhimento/redimensionamento e modo foco permitem continuar o fluxo sem editor externo. Navegação e alertas mantêm semântica em cada tema. Ambiente/path longos ficam compreensíveis, sem exigir IDE completo.

**Perguntas após agir:** “Onde a leitura cansou? Que painel ocuparia mais espaço? Perdeu alguma ação ou o foco? Para concluir esse trabalho aqui, o que precisaria além da leitura e da correção assistida? Um pequeno ajuste manual seria frequente ou você o preferiria no seu editor?”

**Rastreabilidade:** POS-15/16; OX-D04/05. Desktop real/escala/acessibilidade: OX-012/015/017. Esta sessão não certifica todas as tecnologias assistivas, temas ou escalas Windows.

## Registro em branco

Usar uma linha por caso **e por subcaso**. Acrescentar linhas quando houver variações/retestes. Resultado possível: não executado, concluído sem ajuda, concluído com ajuda, interrompido, bloqueado/indisponível. Um caso parcialmente realizado não passa por agregação.

| Caso / variação | Início/fim / caminho percorrido | Resultado / tipo de ajuda | Interpretação, ação errada, desconforto ou fala observada | Issue / evidência / reteste |
| --- | --- | --- | --- | --- |
| P0-01 | ______ | ______ | ______ | ______ |
| P0-02 | ______ | ______ | ______ | ______ |
| P0-03 | ______ | ______ | ______ | ______ |
| P0-04 | ______ | ______ | ______ | ______ |
| P0-05 | ______ | ______ | ______ | ______ |
| P0-06 | ______ | ______ | ______ | ______ |
| P0-07 | ______ | ______ | ______ | ______ |
| P0-08 | ______ | ______ | ______ | ______ |
| P0-09 | ______ | ______ | ______ | ______ |

Registrar falas como citações somente quando efetivamente ditas; separar observação do que a equipe inferiu. Não preencher tempos, notas ou respostas com dados sintéticos. Uma captura do protótipo ou um teste automatizado pode acompanhar o issue técnico, mas não substituir o registro do participante.

### Problemas e reteste

| ID | Caso / versão | Evidência observada | Severidade / consequência | Correção / responsável | Versão e resultado do reteste |
| --- | --- | --- | --- | --- | --- |
| ______ | ______ | ______ | ______ | ______ | ______ |

- **Crítico:** aceitar/aplicar resultado ou destino errado, perder controle/trabalho, duplicar execução por incompreensão, não perceber mudança de cobrança, ou não conseguir concluir uma tarefa essencial. Corrigir e retestar; mantém o gate aberto.
- **Alto:** etapa essencial exige orientação direta ou linguagem induz interpretação recorrente errada. Registrar correção/prioridade; decidir impacto no gate à luz do controle e da conclusão do fluxo.
- **Médio:** descoberta, clareza ou desconforto que não impedem controle/conclusão. Vincular ao incremento pertinente.
- **Baixo:** acabamento sem impacto material na compreensão/controle.

A classificação segue [o protocolo de D0](../research/experience-decisions.md); não converter toda hesitação em falha crítica nem reduzir um erro de aprovação a acabamento.

### Perguntas finais e decisão de edição leve

| Pergunta | Resposta real / observação |
| --- | --- |
| Em comparação à sua rotina atual, onde este app economizaria coordenação e onde acrescentaria trabalho? | ______ |
| O fluxo foi útil com uma única conexão? O que impediria usá-lo em uma mudança real? | ______ |
| Onde esperava encontrar uma ação ou informação? | ______ |
| Quais escolhas devem ficar rápidas e quais podem ficar em configuração avançada? Os nomes dos presets ajudam? | ______ |
| O que faltou para desenvolver confortavelmente sem sair do app? | ______ |
| Uma edição manual pequena precisa entrar no alpha ou a correção assistida atende o primeiro fluxo? Qual caso concreto justifica a resposta? | ______ |

**Decisão sobre edição leve:** ______. **Evidência/caso:** ______. **Destino no backlog:** ______. Se entrar no alpha, especificar controle de escrita, versão/artefato e invalidação de evidências; não assumir um IDE completo.

## Checklist de passagem D1 → M0

Este documento prepara a avaliação e não marca itens como concluídos.

- [ ] Versão e manifesto registrados; casos P0 e critérios POS/ACC associados a resultados ou lacunas explícitas.
- [ ] Direção Studio mantida; interação e adaptação avaliadas, além da preferência visual já informada.
- [ ] Piloto com o responsável realmente realizado e registrado, incluindo ajuda, erros de controle/aprovação e desconforto.
- [ ] Nenhum problema crítico aberto; cada correção crítica tem reteste na versão identificada.
- [ ] Casos/condições indisponíveis têm impacto no gate decidido explicitamente. Interação P0 ausente continua pendência de D1; prova técnica continua no marco da matriz.
- [ ] Decisão de edição leve, nomes dos presets e próximos ajustes registrada com a evidência obtida.
- [ ] Avaliação externa de três a cinco desenvolvedores identificada como pendente para OX-017, quando ainda não realizada.
- [ ] Decisão de retomar M0 registrada no [status](../DEVELOPMENT_STATUS.md), sem atribuir ao piloto prova de OAuth, supervisão, Git, pooling ou ganhos de produtividade.

**Decisão do gate / responsável / data:** ______. **Problemas que bloqueiam:** ______. **Pendências posteriores com destino:** ______.

# D1 — Piloto inicial de experiência

Roteiro preparado em **8 de outubro de 2026**, atualizado em **10 de outubro de 2026**, incluindo os sete refinamentos seguintes de navegação e configurações, após a [conclusão de D0](../research/d0-conclusion.md). **O responsável adiou explicitamente o piloto até que a interface esteja de acordo com a direção desejada. Execução ainda não confirmada; gate de D1 aberto.** A identidade visual foi aprovada anteriormente; a [revisão de navegação, sessões e Settings](d1-shell-revision.md) e o [contrato vigente do shell](../../design-system/workspace-shell.md) ainda precisam de avaliação de uso. As observações, conclusão das tarefas e resultados da sessão permanecem pendentes, com os campos em branco.

O piloto avalia se o responsável pelo produto consegue conduzir desenvolvimento assistido no aplicativo: formular trabalho, fornecer contexto, acompanhar, ler código/diff, pedir correção e controlar a entrega. O estudo permite avaliar a organização da interface sem autenticação, inferência, acesso a arquivos ou operações Git. Os casos de execução históricos usam fixtures internas de teste. O resultado deste piloto informa a experiência; os contratos reais continuam em M0/M1/M2/M3.

Referências: [OX-D03/04/05 e dependências](../DEVELOPMENT_BACKLOG.md), [POS-01 a POS-16](../research/product-positioning.md), [ACC-01 a ACC-07](../research/subscription-account-ux.md), [protocolo de avaliação](../research/experience-decisions.md#8-piloto-e-avaliação-de-uso--roteiro-ainda-não-executado), [protótipo](../../prototypes/desktop/README.md) e [matriz dos casos](d1-scenario-matrix.md).

## Primeiro percurso atual — sete passos pelo chat

**Preparação para retomar após a avaliação da interface, sem novo convite de piloto nesta revisão.** Abra [index.html](../../prototypes/desktop/index.html) diretamente ou a [prévia HTTP](http://127.0.0.1:4173/index.html). Uma entrada normal sem estado salvo tem uma sessão independente, sem contas/projetos/tarefas de estudo preenchidos. Se houver trabalho salvo, use um perfil ou janela de avaliação separados; não apague os rascunhos da pessoa para preparar o caso.

Os nomes abaixo usam English. Idioma fica em **Settings → General → Interface language**; os temas em **Settings → Themes**. Sessions começa à esquerda; Navigation, à direita. Densidade inicial **Compact**, escala **100%**. O objetivo inicial é avaliar a organização sem depender de uma integração de runtime ainda indisponível.

1. **Começar:** localize logo, “What shall we create today?”, cartões de início, sugestões e campo **Message**. Escreva um rascunho e explique qual seria o próximo passo para trabalhar sem projeto.
2. **Trocar de sessão:** use o único comando principal **New session**, no painel Sessions à esquerda, escreva outro rascunho e volte à primeira pela lista. Confira as mensagens/rascunhos próprios e o destaque da sessão ativa; recarregue para avaliar a retomada no mesmo navegador.
3. **Organizar:** use **New project** ou **Open or create project**, informe nome, diretório e contexto compartilhado e crie a sessão do projeto. Volte à sessão independente, escolha **Attach to project** e selecione o projeto. Confira se a conversa preservou identidade e rascunho e se o novo contexto/destino ficou claro. O caminho registrado não concede acesso real à pasta.
4. **Encontrar:** crie outra sessão do projeto e use a busca/lista de Sessions para voltar ao trabalho desejado. Observe se projetos expansíveis e sessões independentes dispensam os dois seletores anteriores.
5. **Distribuir:** mova Navigation para a esquerda para compartilhar o lado com Sessions e alterne as abas. Observe os dois destinos laterais destacados e o blur temporário enquanto escolhe um lado; cancele uma escolha e confira que o efeito desaparece. Volte Navigation para a direita pelo menu/teclado e use **Settings → Panel layout → Reset layout**. Não há destino inferior. Confira foco, conversa e rascunho durante as mudanças.
6. **Configurar:** abra a engrenagem e observe a janela centralizada, até 940 × 720px, limitada ao viewport. Escreva um nome/instrução sem salvar, clique fora no chat e reabra Settings: confira o rascunho preservado. Depois salve explicitamente. Em General, altere idioma/densidade e experimente o slider de texto entre 80% e 200%, em passos de 5%; restaure 100%. Em Themes, escolha outra aparência. Verifique que fechar Settings não substitui a conversa ou desfaz preferências visuais já aplicadas.
7. **Conexões e ajuda:** em Accounts, diferencie ausência de conexão/uso desconhecido de um limite zero. Inicie o cadastro e escolha **Subscription ou API antes do provedor**; confira os campos pertinentes e experimente um valor temporário no campo de chave. O wizard o mantém somente em memória, sem salvar ou enviar ao provedor. Explique a diferença entre configuração salva e autenticação verificada, e por que Subscription Only não habilita fallback API. Abra **Help** e confira que ela abre outra página com busca e guias ilustrados. O [contrato de conexão](api-connection-contract.md) e a [pesquisa oficial](../research/api-provider-discovery-2026-10-10.md) orientam esse caso.

Ao retomar, registrar ajuda fornecida, ações concluídas, interpretações, desconforto e limitações encontradas. Este percurso prepara avaliação de descoberta/navegação, não substitui todos os P0 nem demonstra execução, autorização, quota, correção ou aplicação real. O ciclo completo de resultado → correção → validação → aplicação continuará com condição de execução explicitada, preservando os critérios da matriz e do gate. Quando o runtime ainda estiver indisponível, registrar essa condição em vez de atribuir um resultado inexistente à interface.

### Controles da revisão de sessões

| Área | Referência de facilitação |
| --- | --- |
| Conversa | `#chat-message`, `#chat-form`; rascunho da sessão ativa. |
| Sessões | `.new-session-trigger`, `#session-list`, itens `data-session-id`; ações de criação/associação e busca. |
| Projeto/diretório | `#session-project-form`, `#entry-name`, `#entry-path`, `#entry-context`; ação `data-action="session-attach"`. |
| Painéis | `.dock-panel-handle`, abas `data-dock-tab`, menu de posição e controles de Panel layout; somente esquerda/direita. |
| Settings | `#floating-settings`, seções `data-settings-tab`, fechamento `data-settings-close` ou clique externo; diálogo modeless centralizado, rascunhos preservados ao reabrir. |
| Texto | `#settings-text-size`: slider 80–200%, passo 5%, padrão 100%; valor e ação de restaurar 100%. |
| Conexões | Accounts: modalidade Subscription/API, depois provedor/backend; configuração sem prova de autenticação ou quota. |
| Ajuda | Link externo para `help.html`; a página não ocupa a navegação de conteúdo do app. |

**Evidência histórica anterior aos sete refinamentos:** o check de HTML direto passou sete composições e a jornada de sessões, rascunhos, associação, docking, Settings modeless, idiomas persistidos e ajuda com imagens, sem erros ou requisições externas. Aquela suíte passou 101/101, com manifesto de 52 arquivos e capturas examinadas; o [manifesto anterior a Settings compacta](builds/2026-10-10-before-compact-settings.json) preserva a versão. Esses resultados não certificam a composição atual, cujo fechamento técnico será registrado no [status](../DEVELOPMENT_STATUS.md) e na [revisão do shell](d1-shell-revision.md). Não há aceite humano do piloto.

<a id="primeiro-percurso-atual--seis-passos-pelo-chat"></a>

## Roteiro arquivado — seis passos da versão anterior

O bloco seguinte preserva as instruções já enviadas e sua prontidão técnica. Os controles de demonstração, dropdowns, Options e dados de estudo pertencem à versão anterior à revisão de sessões. Não usá-lo como roteiro da entrada normal atual.

Abra [index.html](../../prototypes/desktop/index.html) diretamente no navegador, ou use a [prévia em 4173](http://127.0.0.1:4173/index.html). Se já houver trabalho aberto, recarregue para voltar ao início da demonstração; não recarregue durante o percurso, pois conversas e trabalhos ficam em memória. Use o projeto de exemplo que já aparece, sem criar outro projeto, conectar contas ou configurar o Studio. O idioma padrão é **English**; os nomes abaixo correspondem a ele. **Language → Português** é opcional, e a escolha anterior pode já estar salva. Em janela mobile, abra **Options → Language** para selecionar o idioma; no desktop os utilitários permanecem abertos.

1. Em **Message for the project**, escreva: “Adicionar uma mensagem acessível quando a busca não encontrar resultados e verificar que ela aparece apenas quando a lista está vazia.” Clique em **Send** e veja o trabalho preparado no cartão.
2. Nesse cartão, clique em **Start demo** e depois em **Finish simulated work**. Confira a mudança de estado e o próximo passo oferecido.
3. Clique em **Review result**. No campo **Correction for this result**, escreva: “Explicitar também o comportamento quando a busca volta a ter resultados.” Clique em **Request a correction for this task**; de volta ao chat, conclua essa tentativa com **Finish simulated work**.
4. Abra **Review result** novamente, confira o resultado e a identificação da nova tentativa/versão e clique em **Validate supplementary result**. Antes de continuar, explique se isso já aplicou o resultado no destino.
5. No cartão, clique em **Review application**, confira o resultado e o destino apresentados e confirme em **Apply this Run in the demo**. Observe como o cartão informa a aplicação.
6. Clique em **New chat** na navegação e escreva um rascunho sem enviar. No seletor **Chat**, volte à primeira conversa e confira o pedido e o resultado; retorne à nova conversa e veja se o rascunho foi preservado.

Ao terminar, registre os passos concluídos, o que ficou confuso ou desconfortável, qualquer ajuda recebida e sua resposta sobre a diferença entre validar o resultado e aplicá-lo. Se travar, registre o passo e o que esperava encontrar; não preencher uma conclusão que não ocorreu. Trata-se de um **piloto individual guiado**, com resultados simulados, sem efeitos no código ou Git reais.

Este percurso inicia a avaliação de P0-02, P0-06, P0-07 e P0-09: pedido pequeno, correção versionada, validação/aplicação separadas e preservação ao trocar de conversa. **Não cobre todas as variações desses casos nem os nove casos P0**, incluindo feature dependente, mudanças de base/candidato, eventos durante revisão, lifecycle de contas e controle incerto. As lacunas continuam explícitas na matriz e sua decisão permanece no checklist de passagem; concluir estes seis passos, sozinho, não fecha D1.

### Referência arquivada de controles para a facilitação

| Passo | Controles naquele build |
| --- | --- |
| 1 | `#chat-message`; envio em `#chat-form`. |
| 2 | Cartão `.chat-task-card`: `data-chat-action="start"` e `"finish"`. |
| 3 | `data-chat-action="review"`; `#supplement-correction`; envio em `#supplement-correction-form`; nova conclusão `data-chat-action="finish"`. |
| 4 | Revisão `data-chat-action="review"`; diálogo `data-action="confirm-supplement"`. |
| 5 | `data-chat-action="apply"`; diálogo `data-action="confirm-independent"`. |
| 6 | `data-action="chat-session-new"`; rascunho `#chat-message`; seletor `#chat-session`. |

**Evidência histórica antes da revisão de painéis:** o refinamento mobile passou **27/27 casos focados** (12 de entrada e 15 de idiomas), com catálogo de **1.012 chaves** e check aprovado. A suíte completa final passou **77/77 em uma única rodada de 2,2 minutos**, sem falhas ou casos ignorados. `npm.cmd run check:file` confirmou sete composições, o percurso curto, rascunho, idiomas persistidos e reflow a 200% via `file://`, sem servidor. Os **73 casos por cobertura consolidada** da revisão de idiomas e os quatro focados da fluidez do site permanecem históricos separados. Consulte o [registro da experiência progressiva](d1-progressive-experience.md#refinamento-da-entrada-mobile--1010) e o [contrato de idiomas](interface-languages.md). Verificação técnica não substitui observações humanas; piloto ainda não confirmado.

**Versão da preparação anterior:** [manifesto arquivado antes dos painéis](builds/2026-10-10-before-session-panels.json), **36 arquivos**, SHA-256 agregado **`896778a77a828b86ed4d12e0e77c82e4154951c10e6fc718d015180901cb727c`**. O [manifesto anterior ao refinamento mobile](builds/2026-10-10-before-mobile.json) conserva o hash `f5077169474953e30c0511ac60d5e561cd5e7115d4fb2d1cfb967bf720c4e965`; escopo/algoritmo estão na [entrega](d1-delivery.md#revisão-atual--1010). Registre alterações posteriores antes do piloto. O campo inteiro ficou na primeira dobra em 390×844 e 320×900, nos três idiomas e tamanho normal; **Send pode exigir rolagem**, assim como a interface com texto a 200%. Avaliar conforto e compreensão no uso humano, separadamente dessas medidas técnicas.

## Referências anteriores e casos detalhados

Os roteiros seguintes conservam o histórico da preparação e os controles de domínio para os casos P0. Language/Options e controles simulados descritos neles pertencem às versões arquivadas. Na revisão atual, idioma fica em Settings → General → Interface language. O piloto só será retomado quando o responsável considerar a interface pronta para essa avaliação.

## Feedback e preparação em 09/10

Registro de comunicação com o responsável, sem sessão reproduzível de uso registrada:

- Ao receber o convite para o piloto, informou não ter entendido o que fazer e pediu orientações concretas. A facilitação forneceu os oito passos abaixo; isso é **orientação direta**, sem execução ou conclusão confirmada.
- Relatou que gostou da interface, mas a considerou pouco intuitiva e sentiu falta de chat. Quer uma experiência próxima à de um agente de código, percebendo o projeto como o espaço da conversa.
- Pediu preservar a interface existente, sem mudar tudo. Refinou a direção ainda em 09/10: **manter o início por projetos e o fluxo guiado; permitir várias conversas por projeto e conversas avulsas como ponto de partida**. Projeto e conversa são entidades distintas, com relação opcional. Studio, tarefas, revisão de diff e inspectors são mantidos; a [direção de projetos/conversas](d1-chat-direction.md) detalha a proposta.

São relatos reais e uma orientação de produto. A dificuldade inicial com o pedido de piloto não demonstra, por si só, um defeito da interface. Não há registro de quais ações foram tentadas, tempo, erro de aprovação ou tarefa concluída; não atribuir causa nem preencher resultados humanos a partir desta comunicação.

### Ajuda direta fornecida — oito passos da versão anterior

Este roteiro registra a preparação já enviada. Os rótulos correspondem à versão centrada em **Começar/Trabalho**, anterior ao incremento por conversa. Não é o roteiro principal da próxima avaliação.

1. Abrir o protótipo e usar o botão do projeto para entrar em **Começar**; escolher **Correção curta · 1 tarefa**.
2. Usar **Preparar projeto de exemplo** e, no diálogo pendente, **Continuar autorização simulada**.
3. Confirmar a modalidade pelo checkbox e usar **Validar registro simulado**.
4. Em OX-24, usar **Iniciar demonstração** e **Concluir trabalho simulado**.
5. Abrir **Alterações**, examinar o diff, escrever o pedido e usar **Pedir correção**.
6. Voltar a **Trabalho** e usar **Concluir correção simulada**.
7. Revisar o novo resultado, usar **Validar tarefa no Run…** e confirmar **Validar na demonstração**.
8. Usar **Revisar aplicação do Run…**, conferir resultado/destino e confirmar **Aplicar na demonstração**.

O participante deve distinguir o passo 7, que aceita a tarefa na branch interna do Run, do passo 8, que aplica o candidato no destino. Essa compreensão ainda precisa ser observada. Nenhuma dessas ações modifica Git real no protótipo.

### Preparação anterior de 09/10 — projetos e conversas

**Resultado histórico do incremento de 09/10: 36/36 testes**, incluindo 24 anteriores retestados e 12 de projetos/conversas. Identificador daquele build: **`da380dda89ea1a3aa0c8fc3fac77de19d8681d8de4ddd6e0ca41c9d9b37c6ae2`**, conforme a [entrega](d1-delivery.md). Esse hash não identifica a revisão de identidade/idiomas atual. Execução do piloto continua pendente.

Contrato disponível: **Abrir projeto** preserva início/preparação e templates guiados; **Projeto ou espaço** e **Conversa** selecionam contextos distintos, com **Nova conversa** e **Conversa avulsa**. Composer/**Enviar** pertencem à conversa ativa. O primeiro pedido prepara tarefa/Run independente, sem substituir o template nem iniciar execução. Cartões oferecem **Ver tarefa**, **Iniciar demonstração**, **Revisar resultado** e **Pedir correção**; este último prepara o comentário na revisão, cujo envio continua explícito. **Novo trabalho nesta conversa** abre outro pedido. Trocas preservam mensagens, rascunho e snapshots **em memória**, inclusive projetos de mesmo nome/IDs diferentes. Temas são globais; recarregar/resetar não é recuperação durável. A avulsa não tem repositório/destino e bloqueia aplicação. **Associação posterior não está incluída neste incremento.**

O exercício inicial usa o projeto de exemplo já disponível e uma nova conversa. O início guiado continua avaliado nos casos P0. Um convite curto foi enviado ao responsável: recarregar → **Nova conversa** → escrever uma mudança → **Enviar** → criar outra conversa e retornar à primeira pelo seletor. Essa instrução é ajuda direta, sem execução ou resultado confirmado. Para avançar, entregar um pedido de cada vez e registrar ajuda fornecida:

1. **Formular:** “Neste projeto de exemplo, crie uma conversa e peça ao agente para preservar o registro de auditoria ao revogar uma sessão. Inclua como conferiria essa mudança.” Observar distinção entre projeto, conversa, mensagem e trabalho preparado. Ajuda, se necessária: **Nova conversa → Mensagem para o projeto → Enviar**. Esse envio prepara um trabalho independente; execução ainda exige **Iniciar demonstração**. Resultados demonstrativos ficam identificados.
2. **Acompanhar e corrigir:** “Veja o trabalho preparado, acompanhe a conclusão simulada e abra as alterações. Peça uma correção pelo contexto dessa conversa e confira o novo resultado.” Ajuda, se necessária: cartão → **Ver tarefa/Iniciar demonstração** → concluir trabalho simulado → **Revisar resultado**; **Pedir correção** prepara comentário na revisão, onde o envio explícito cria a tentativa de correção. Concluir a correção simulada e revisar o novo resultado. Observar se mensagem e correção ficam vinculadas ao trabalho/artefato esperado, sem trocar silenciosamente tarefa ou conta.
3. **Aceitar e aplicar:** “Confira as evidências e aceite a tarefa. Antes de aplicar, diga o que já foi aceito e qual destino ainda será alterado. Depois confira o candidato e aplique na demonstração.” Observar separadamente aceite de tarefa e aplicação do Run, conforme P0-07/POS-10/11.
4. **Trocar e comentar:** “Crie outra conversa neste projeto, deixe um rascunho e volte à primeira. Depois comece uma conversa avulsa e veja por que ela ainda não pode aplicar uma alteração. Volte ao projeto anterior: onde estão cada mensagem e trabalho? O que ficou mais próximo do seu jeito de usar um agente?” Ajuda, se necessária: **Nova conversa**, **Conversa avulsa**, seletores **Projeto ou espaço/Conversa**. Conferir preservação das conversas, rascunho e snapshots sem mistura de tarefas/revisões. Em variação separada, preparar outro projeto de mesmo nome e voltar ao original pelos IDs próprios. Não tentar associação/conversão da avulsa: é limite posterior explícito. Registrar resposta real, sem pressupor que a alteração resolveu a dificuldade.

Perguntas essenciais após agir: “Qual projeto e qual conversa estão ativos?”, “Uma conversa avulsa já aponta para um repositório?”, “Enviar a mensagem começou execução ou preparou trabalho?”, “A qual tarefa/versão foi enviado o pedido de correção?”, “Validar a tarefa já aplicou no destino?” e “Onde abriria detalhes de conta, modelo e contexto?”. Esses passos simplificam a facilitação; não substituem os cenários POS/ACC, os casos críticos nem a avaliação externa posterior. A sessão será identificada como **piloto individual guiado**, sem alegação de uso independente ou ganho de produtividade.

## Responsáveis e decisão

| Papel | Responsabilidade | Registro da sessão |
| --- | --- | --- |
| Participante inicial | Responsável pelo produto; realizar tarefas e explicar o que entende das telas. | Nome/identificação: ______ |
| Facilitador | Apresentar tarefas sem explicar onde clicar; preparar variações; registrar ajuda e erros. | Nome/identificação: ______ |
| Implementação/design | Resolver lacunas de prontidão e problemas observados; vincular alteração e reteste ao mesmo caso. | Responsável: ______ |
| Decisão do gate | Conferir registro, casos pendentes e problemas críticos, com o responsável pelo produto. | Responsável/data: ______ |

Se o responsável fizer um piloto individual guiado por este documento, registrar **piloto individual**, manter os prompts de tarefa separados das notas de facilitação e anotar toda ajuda recebida, inclusive do agente. Não apresentar essa sessão como estudo independente. A avaliação com três a cinco desenvolvedores externos permanece necessária antes de concluir OX-017 em M2.

## Preparação reproduzível histórica — fixtures e rótulos anteriores

O bloco seguinte preserva a preparação dos casos de domínio na versão anterior, incluindo controles sintéticos e preferências antigas. Não usar Reset demo, criação de dados ou restauração de densidade confortável como instrução da abertura normal atual. Os sete passos atuais acima orientam navegação/Settings; adaptar a facilitação dos P0 à condição real disponível antes de retomar o piloto.

1. Abrir [index.html](../../prototypes/desktop/index.html) diretamente conforme o [guia local](../../prototypes/desktop/README.md#abrir), sem exigir servidor, e registrar a versão exata, endereço `file://`, navegador, viewport e escala. Não usar contas, repositórios privados ou credenciais reais.
2. Studio é o tema padrão, com sete alternativas (**oito temas**). Nenhuma mudança de aparência é necessária para o primeiro percurso. Para comparar temas no caso P0-09, usar **Customize appearance → Settings → Appearance** (no mobile, abrir **Options** primeiro), com **Restore Studio**, ou os controles equivalentes em Português. Registrar o idioma efetivo; ele é independente do tema.
3. Usar **Reset demo** antes de cada caso independente; nos casos detalhados em Português, **Reiniciar demonstração**. Recarregar também reinicia o trabalho e as conexões. Idioma, tema, densidade, tamanho do texto e largura da fila persistem quando o navegador disponibiliza armazenamento; **Reset layout / Restaurar layout** retorna densidade confortável, texto normal e largura inicial. Recolher/mostrar tarefas e modo foco são controles da sessão, sem promessa de persistência. Resetar o trabalho não substitui restaurar as preferências visuais, e o armazenamento de páginas `file://` depende do navegador.
4. Conferir os estados abaixo pelas telas. A pessoa de facilitação pode preparar uma variação por controles explícitos de demonstração, mas deve registrar o caminho utilizado. Não substituir uma interação ausente por uma explicação verbal e chamá-la de testada.
5. Consultar a [entrega técnica D1](d1-delivery.md), o [status atual](../DEVELOPMENT_STATUS.md) e conferir versão/manifesto. A passagem histórica de 09/10 teve **36 cenários aprovados** com Chrome 154.0.8037.98 headless, Playwright 1.64.0 e Node 24.19.0 no Windows. A revisão histórica de idiomas de 10/10 validou **73 casos por cobertura consolidada**, e a revisão de fluidez do site teve quatro casos focados. O refinamento mobile posterior passou **77/77 em uma única rodada**, conforme o registro atual acima. Não reutilizar o hash de 09/10 como versão atual. Esses resultados não produzem dados humanos nem fecham OX-D05.

**Seed e entrada guiada preservados:** **Reiniciar demonstração** retorna à conversa inicial e ao seed com OX-24 em revisão, OX-25 em execução, OX-26 esperando decisão e OX-27 com falha terminal confirmada; seus Runs são independentes. Botão do projeto/**Abrir projeto** permite preparar **Correção curta · 1 tarefa** ou **Feature guiada · 4 tarefas** em R-08. Preparar outro projeto conserva os espaços anteriores em memória; a primeira preparação inicializa a fixture de uma conexão se a coleção estiver intacta. Depois reutiliza registros existentes ou acrescenta um pendente, conforme o [README](../../prototypes/desktop/README.md#dados-e-limites-do-cenário). A tela não lê o path nem converte Windows/WSL. A feature conserva a aplicação final de cinco arquivos.

### Manifesto de cenários da sessão

Os caminhos abaixo documentam a versão anterior ao incremento por conversa. Servem de referência/regressão dos controles de domínio; confirmar os novos caminhos e a versão entregue antes do próximo piloto. Todos os dados são fictícios. O mesmo manifesto deve permitir repetir um reteste. Os caminhos são notas de facilitação; quando entregues ao participante como ajuda, registrar orientação direta.

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
| Endereço `file://` ou URL local; forma de iniciar/resetar | ______ |
| Idioma da interface e tema usados | ______ |
| Sistema / navegador / versão | ______ |
| Viewport CSS / tamanho da janela | ______ |
| Escala física Windows / zoom navegador / ampliação de texto | ______ |
| Dispositivos de entrada / tecnologia assistiva utilizada | ______ |
| Variações prontas / indisponíveis e motivo | ______ |
| Interrupções ou alterações do protótipo durante a sessão | ______ |

Na preparação histórica, **Tamanho do texto → Ampliado · 200%** era um controle binário da interface. Na revisão atual, **Settings → General** oferece slider 80–200%, passo 5%, padrão 100%. Nenhum deles é zoom do navegador ou escala física do Windows. Registrar cada condição usada separadamente; não generalizar um resultado para as demais. Zoom/escala e tecnologias assistivas não exercitados continuam pendentes para QA do Desktop real.

## Condução e registro

Apresentação inicial: “Este é um estudo simulado do Orchestrix. Queremos entender como as telas ajudam você a conduzir trabalho e controlar o resultado. Explique o que espera acontecer antes de ações que mudam conta, tentativa ou destino. Quando houver dificuldade, registre o que procurou. Estamos avaliando a interface.”

Antes de começar, registrar a rotina atual: agente/editor utilizados, como envia contexto, onde revisa e como aplica uma alteração. Essa descrição serve de comparação qualitativa; não fornece uma medição de produtividade.

Durante cada caso, entregar só o **prompt do participante**. O caminho e o resultado esperados ficam com a facilitação. Na parte exploratória, não ensinar os rótulos corretos antes de observar a interpretação. No piloto guiado solicitado, pode-se orientar a ação quando necessário, registrando a ajuda; conclusão guiada não deve ser descrita como descoberta independente. Uma pausa para pensar ou exploração alternativa não é automaticamente erro.

Anotar início/fim, caminho percorrido, hesitações, ajuda e ações inesperadas. Classificar ajuda como **nenhuma**, **prompt neutro** (“o que você procuraria?”) ou **orientação direta** (nomear a tela/botão). Se uma ação não existir ou a simulação quebrar, registrar **indisponível/bloqueado por protótipo**. Não registrar como erro do participante. Uma explicação do facilitador depois disso não conta como conclusão da tarefa.

Os casos podem ser divididos em sessões para preservar atenção. Registrar as partes e retomar com o mesmo manifesto. Não impor um tempo de conclusão como critério de usabilidade nesta amostra.

## Casos P0 — referência da versão anterior e regressão

Os nove grupos abaixo conservam o escopo de avaliação. Seus rótulos e caminhos antecedem a entrada por conversa; adaptar a facilitação ao incremento confirmado sem remover aceite, aplicação, identidade, contexto, estados incertos ou acessibilidade. O roteiro curto acima prepara a primeira sessão guiada, sem declarar estes casos executados.

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

**Notas de facilitação:** **Simular perda de sinal** e cancelamento sem resposta são subcasos diferentes. Depois de cancelamento incerto, a reconciliação deve confirmar o cancelamento da mesma tentativa, em vez de iniciar outra. **Desconectar para novos trabalhos** é um bloqueio de novas requisições daquela conexão, não uma suspensão global de admissões nem confirmação de término do worker. O estudo não implementa o comando real do daemon: a suspensão mínima de novas admissões está planejada em M2; políticas/escopos do scheduler ampliado seguem M3. Avaliar os controles de conexão/tentativa e registrar se a ausência global impede o fluxo do participante, sem declarar cobertura desse recurso.

**Prompt do participante:** “Pare novos trabalhos desta conexão sem perder a tentativa em andamento. Agora interrompa uma tentativa cujo sinal foi perdido. Explique o que sabe sobre o término antes de começar outra. Depois veja como retomar o controle quando não houver confirmação da revogação.”

**Observar:** bloqueio de novos trabalhos da conexão, pausa de turno simulada, cancelamento e desconexão têm escopos distintos; não generalizar para pausa global da fila. Estado incerto/reconciliando conserva tentativa/identidade e não permite duplicação automática do worker. Sair da conta ou parar novas requisições não confirma término do processo nem revogação remota. Confirmações e limites da simulação aparecem na tela.

**Perguntas após agir:** “O processo terminou ou isso ainda é desconhecido? Qual operação ficou proibida enquanto não houver confirmação? Fechar a conexão cancelou a tentativa? Qual autorização foi revogada e qual confirmação ainda falta?”

**Rastreabilidade:** POS-12; ACC-07. Prova terminal/reconciliação: OX-001/007/010/011; lifecycle de autenticação: M0/M3.

### P0-09 — Desenvolver com conforto, teclado e adaptação

**Atualização da navegação para a futura retomada:** executar o caso com Sessions à esquerda e Navigation à direita, densidade Compact inicial e slider em General. Testar docking somente entre os lados, com cancelamento do blur; Settings centralizada, fechamento externo e reabertura com rascunho preservado. Reset layout restaura o arranjo lateral; escala volta a 100% pela ação própria do slider. As instruções atuais abaixo complementam os critérios históricos sem reutilizar os defaults anteriores.

**Preparação para retomar:** adaptar V08 à versão identificada, usando Studio e um tema alternativo com os mesmos dados. Registrar janela, densidade e escala efetivas. Começar em Compact/100% e comparar janela ampla/estreita, densidade Comfortable e escala até 200% pelo slider. Outros viewports, zoom do navegador e escala física Windows podem ter QA técnica adicional.

**Prompt do participante:** “Continue a revisão usando só o teclado. Abra uma ação pela busca, mude uma aba/arquivo, escreva uma correção, abra e feche uma confirmação e volte ao ponto de trabalho. Depois reduza a janela e amplie o texto da interface a 200%. Organize os painéis para ler confortavelmente, compare a densidade e escolha outra aparência.”

**Notas de facilitação atuais:** testar mover painéis entre esquerda/direita por arraste e, separadamente, menu/teclado; conferir abas compartilhadas e blur removido ao concluir/cancelar. Abrir Settings centralizada, escrever rascunho, fechar pelo clique externo e reabrir para recuperá-lo. Comparar Compact/Comfortable e slider 80–200%, passo 5%, em General. Recarregar para conferir preferências salvas. **Reset layout** restaura o arranjo lateral; o slider tem ação própria de restaurar 100%, e **Restore Studio** restaura o tema. Não declarar escala Windows 200% testada por usar o controle de texto da interface. O antigo reset combinado de densidade/texto pertence ao histórico.

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

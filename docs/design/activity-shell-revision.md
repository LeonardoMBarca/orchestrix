# D1 — Barra de atividades e arquivos

Revisão solicitada em **12 de outubro de 2026**. Estende o [contrato do shell](../../design-system/workspace-shell.md) e prepara a implementação de M2 sem encerrar o piloto D1 nem antecipar os gates de M0. A identidade Studio, a entrada pelo chat e a simplicidade com uma única conta permanecem.

## Direção e organização

| Superfície | Contrato |
| --- | --- |
| Barra de atividades | Fixa à esquerda. Ordem inicial Chat, Files, Studio, Connections; os quatro itens podem ser reordenados. Help, Settings e Profile ficam na base. A barra não participa do docking nem do recolhimento. |
| Navigation | Painel lateral de Files, Studio ou Connections, inicialmente à esquerda com 224px quando necessário. Chat não abre esse painel. |
| Sessions | Projetos expansíveis e sessões avulsas, inicialmente à direita com 280px. Mantém o único criador principal de sessão. |
| Docks | Painéis móveis entre esquerda/direita; itens no mesmo lado compartilham abas. Nenhum destino inferior. |
| Divisórias | Ajustam largura, recolhem ao reduzir e permitem reabrir pela borda, com alternativa por teclado e restauração da largura útil. |
| Menu superior | File, Edit, View e Help, com ações pertinentes ao app e sem Run, debugger ou terminal. |
| Busca superior | Preenche a largura disponível do cabeçalho central entre os demais controles, sem deixar espaço vazio por um limite fixo de largura. |

Chat abre diretamente a conversa, sem um painel lateral próprio com atalhos redundantes de sessão, diretório ou trabalho. A barra fixa permanece visível e Sessions continua disponível como painel independente, com seu recolhimento e posição preservados. Files apresenta a estrutura autorizada e o arquivo aberto; Studio reúne trabalho, atenção, alterações e histórico; Connections oferece cadastro, recursos e diagnóstico. A troca entre essas atividades não apaga rascunhos. Help continua abrindo o navegador; Settings é centralizada, fechável e modeless, e Profile abre sua seção de perfil.

Arrastar painéis conserva os dois destinos laterais e o blur temporário já definido. Reordenar atividades muda a preferência de navegação, sem modificar contas, papéis de agentes ou políticas de execução. Reset layout restaura os lados, larguras e disposição inicial previstos pelo controle. A barra fixa continua disponível quando um dock está recolhido.

## Arquivos sem ampliar para IDE

A edição leve atende casos explícitos do responsável: ajustar um `.env`, ler documentos gerados e revisar Markdown dentro do Orchestrix. Não introduz execução de código, debugger, language server, terminal ou gestão de extensões.

- **Pasta autorizada:** usar concessão real de acesso. Onde handles de diretório e escrita estiverem disponíveis, salvar atua somente no arquivo permitido. Um caminho digitado ou salvo no projeto não é uma concessão.
- **Importação:** quando a integração de escrita não estiver disponível, arquivos selecionados podem ser lidos/editados em memória; a saída é salvar uma cópia, sem afirmar que a origem foi atualizada.
- **Estado:** diferenciar leitura, alteração não salva, gravação concluída e erro/permissão ausente. Preservar o rascunho diante de cancelamento ou falha e oferecer decisão explícita antes de descartá-lo.
- **Markdown:** alternar leitura e edição sem executar HTML/scripts embutidos. Abrir um arquivo não o envia automaticamente ao contexto de uma AI.
- **Desktop:** M2 deve integrar grants, versão/base e escrita com o Core. Alterações concorrentes precisam de tratamento de conflito e invalidam evidências da versão anterior. O protótipo do navegador não demonstra essa coordenação com workers.

Conexões e diagnósticos mantêm suas capacidades observadas e valores desconhecidos. A revisão não altera Subscription Only, não acrescenta fallback pago e não exige várias contas. Efficiency, Performance, Balanced, Custom e presets especializados continuam no plano existente.

## Refinamento de File e criação de arquivos

O menu **File** e as entradas de Files concentram **Open file, Open folder e New file**. Criar sessão ou projeto fica nas superfícies correspondentes, enquanto salvar e fechar acompanham o arquivo aberto. Abrir um arquivo avulso não obriga a abrir uma pasta.

**New file** pede nome com extensão e destino na pasta autorizada atual ou numa subpasta elegível. Sem pasta aberta, a pessoa precisa selecionar a pasta de destino para criar no disco. A criação acontece somente após confirmação explícita, respeita a permissão de escrita e não sobrescreve um arquivo existente. Quando o navegador não oferece essa integração, rascunho/cópia deve permanecer identificado, sem apresentar conteúdo em memória como arquivo já gravado.

O contrato para **M2 Desktop** inclui bolinha no nome/aba para edição não salva, **Ctrl+S** e três decisões ao fechar um arquivo alterado: **Save, Discard e Cancel**. Salvar só permite o fechamento após sucesso; cancelar preserva a edição. A revisão D1 ajusta as entradas e a lógica de destino, preservando os mecanismos browser existentes sem implementar agora esse novo ciclo completo de fechamento. Não altera marcos, gates nem o escopo de editor leve.

## Validação inicial da barra de atividades

`npm run check` passou com **1.608 chaves EN/PT-BR/ES**, incluindo sintaxe dos módulos de atividades e arquivos. A cobertura browser consolidou **173 cenários**, sem afirmar uma única rodada com todos aprovados:

- A suíte inicial passou **143/172 em 15,8 minutos**. Os testes antigos ainda usavam seletores de navegação removidos, o default anterior de Sessions e um retorno ao Studio incompatível com o painel mobile fechado; dois casos de Settings excederam o timeout durante a carga concorrente.
- Após adaptar os caminhos aos novos controles, a rodada focada passou **31/34 em 5,8 minutos**. Restaram dois timeouts de cadastro durante concorrência e uma colisão entre o nome Workspace da barra e o campo do formulário; o campo passou a usar o textbox exato.
- A rodada final, sem outra suíte browser concorrente, passou **16/16 em 2,4 minutos**: nove casos do novo shell (incluindo uma verificação nova do menu Edit) e os sete casos ainda pendentes. O caso novo eleva o total distinto a 173. As assertions de comportamento, privacidade e permissões foram preservadas.

A cobertura inclui reordenação persistente por ponteiro/teclado, recolhimento e restauração pela borda, abas e foco/rascunhos preservados, Settings/Profile, idiomas, oito temas e reflow. Os sete casos de Files incluem escrita com handles controlados, importação com download de cópia, confirmação de descarte, falha de permissão, limites de texto/binário, leitura sob demanda e Markdown com HTML/URLs ativos mantidos inertes. Os testes não autenticaram contas reais nem executaram agentes; handles e respostas de discovery foram controlados nos cenários pertinentes.

Cinco capturas do shell, três do editor e três diagramas da ajuda foram examinados e [catalogados](visual-asset-catalog.md). A entrada não rolou em 1440×900 ou 1280×720 a 100%; mobile com texto a 200% manteve reflow sem overflow horizontal. Não houve chave ausente, imagem quebrada ou erro JavaScript nessas composições.

`npm run check:file` passou **13 composições** por HTML direto: desktop, notebook e mobile em EN/PT-BR/ES, mais a jornada de sessões/rascunhos, associação a projeto, agrupamento real dos painéis à esquerda, restauração à direita, blur, Settings, escala/idioma persistidos e ajuda externa com as seis ilustrações. Não houve requisição HTTP/HTTPS, erro, chave ausente, imagem quebrada ou overflow horizontal. Em 320px, PT-BR/ES usam rolagem vertical interna; as entradas de notebook a 100% não rolaram. A jornada não inclui consulta a provedores nem acesso de escrita nativo ao disco.

O [build anterior ao Chat direto](builds/2026-10-12-before-direct-chat.json) preserva essa entrega de **70 arquivos**, SHA-256 `e391d566bd8e1ad7da0a05ec72ba3ba534fc8a7ee06532974883df174ca770c7`. O [build anterior à barra de atividades](builds/2026-10-12-before-activity-shell.json) preserva os 63 arquivos e o digest `8cdad562051d594ba8d6adf693a65226d79471ce4491fa9edc3c757c5a87d35c`. O manifesto estático exclui testes, ferramentas, host e documentação.

## Refinamento de Chat direto e busca ampla

A pedido do responsável, Chat deixa de abrir seu painel lateral redundante. A disponibilidade do painel acompanha a atividade, sem apagar posições, larguras ou preferências de recolhimento. Sessions permanece independente, inclusive quando compartilha o lado com Files/Studio/Connections. Voltar ao Chat conserva o mesmo campo, conteúdo e seleção do rascunho. A busca superior usa o espaço livre do cabeçalho, acompanhando a abertura e o recolhimento dos painéis.

`npm run check` passou com **1.608 chaves**. A regressão focada cobriu **51 cenários distintos** de atividades, docking, arquivos, entrada, sessões e ajuda: **49/51 em 4,9 minutos**, seguidos de **2/2 em 20,8 segundos**. Um caso excedeu o timeout na recarga final sob carga e passou isolado sem alterar suas assertions; o outro ainda buscava a frase antiga da ajuda e foi atualizado para conferir Chat direto e Sessions independente. Os dois casos novos verificam largura útil da busca, reflow, ambos os lados, identidade do campo, rascunho e preferências preservadas. A suíte completa anterior de 173 casos não foi repetida neste refinamento.

HTML direto passou novamente as **13 composições** e a jornada de sessões, docking, Settings, idiomas e ajuda, sem erros ou requisições externas. Três novas capturas foram examinadas em **1440×900, 1280×720 e 390×844**, além do diagrama de docking atualizado. Não houve overflow horizontal, imagens quebradas ou chaves ausentes; as entradas desktop não exigiram rolagem. A busca mediu aproximadamente 799px, 836px e 290px nessas composições, respectivamente. Capturas e aliases estão no [catálogo visual](visual-asset-catalog.md#chat-direto-e-busca-expandida--12102026).

O [build anterior às ações de arquivos](builds/2026-10-12-before-file-actions.json) preserva esse refinamento de **70 arquivos**, SHA-256 `8f6b0ea41c8328800e4c475c63e37a4b04053e32b7d89cbee02f8ea94fc59511`.

## Validação atual — ações de arquivos

`npm run check` passou com **1.631 chaves EN/PT-BR/ES**. A regressão focada cobriu **28 cenários distintos** de atividades, arquivos e ajuda: **27/28 em 1,1 minuto**, seguidos de **1/1 em 5,8 segundos**. O caso restante foi corrigido para verificar a bolinha da aba certa, pois agora há dois arquivos com o mesmo nome abertos. Não houve mudança de comportamento para fazer o teste passar.

Os seis casos novos de Files verificam arquivo avulso com mesmo nome sem perder pasta/edições, cancelamento do picker, criação explícita com extensão `.txt`, subpasta, nomes inválidos, colisão, destino obrigatório e permissão negada. Handles controlados exercitam as APIs sem alterar arquivos do repositório. Os casos anteriores preservam gravação, cópia, Markdown inerte, limites e fechamento já existentes; o ciclo Save/Discard/Cancel do Desktop continua pendente em M2.

Três capturas de menu e diálogo em desktop/mobile foram examinadas e [catalogadas](visual-asset-catalog.md#ações-de-arquivos--12102026), sem overflow horizontal ou chaves ausentes. O percurso adicional por HTML direto abriu um arquivo avulso, criou um rascunho explícito quando o picker nativo foi desabilitado e baixou sua cópia por Ctrl+S, sem requisições externas ou erros JavaScript. A troca programática de idioma manteve o nome digitado. As 13 composições gerais do refinamento anterior não foram repetidas nesta mudança focada.

O [manifesto atual](d1-build-manifest.json) identifica **70 arquivos**, SHA-256 `580bc13d13801c69aac436ab5fce923691d29abb47982bd875fc9020d85c2613`.

O [piloto](d1-pilot.md) foi atualizado para a organização atual, mas segue adiado até o responsável considerar a interface pronta. Testes automatizados e preferência estética não substituem as observações reais do piloto. O fluxo D0 → D1 → M0 → M1 → M2 e os tickets existentes são preservados.

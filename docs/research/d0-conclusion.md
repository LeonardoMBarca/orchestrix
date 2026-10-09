# D0 — Conclusão da pesquisa e passagem para D1

Consolidação em **8 de outubro de 2026**. **D0 concluído como etapa de pesquisa**, pelo critério metodológico explicitado abaixo. O resultado é uma direção de produto, requisitos de experiência e prioridades rastreáveis. Validação de uso, adapters e aplicativo de produção têm gates próprios.

## Critério de encerramento e alteração registrada

O backlog anterior pedia cinco percursos por teste/demo em pelo menos três referências. Os registros anteriores passaram a tratar isso como quinze jornadas completas, incluindo recuperação e aplicação final confirmadas. **Esse requisito integral não foi demonstrado.** O critério foi proposto pelo agente durante a elaboração do plano, não imposto pelo responsável. A revisão é uma decisão metodológica do agente sob a autonomia delegada para conduzir o plano; o pedido do responsável foi pesquisa aprofundada de solução/interface como etapa inicial.

Nesta consolidação, o aceite de OX-D01 é substituído de forma explícita: comparar pelo menos seis soluções em fontes primárias; mapear os cinco tipos de percurso em pelo menos três referências; examinar demonstrações reais em pelo menos três delas; classificar cada passo como observado, documentado ou não verificado; ligar lacunas às decisões afetadas e aos testes futuros. OX-D02 mantém seu aceite de três referências anotadas e decisões justificadas.

A alteração permite encerrar descoberta com resultados utilizáveis e limites conhecidos. Não transforma documentação em teste, não afirma que quinze jornadas passaram, não certifica concorrentes e não reduz os gates de autenticação, controle, recuperação ou integração do Orchestrix. A observação de mídia consiste em **frames extraídos de vídeos/GIFs públicos**, com tempos e limites; previews do YouTube têm classificação separada. Não houve operação de agentes concorrentes, login ou avaliação de usabilidade nesta pesquisa.

| Critério consolidado | Evidência | Resultado |
| --- | --- | --- |
| Pelo menos seis soluções, fontes/data e lacunas | [Levantamento](../PRODUCT_AND_UX_RESEARCH.md), [posicionamento](product-positioning.md), [matriz de abrangência/proveniência](landscape-and-provenance.md) | Nove famílias examinadas; superfície de aplicativo, runtime e SDK diferenciada. |
| Cinco tipos de percurso em pelo menos três referências | [Jornadas](solution-journeys.md), [Conductor](conductor-dynamic-evidence.md), [Cline/Superset](workflow-demonstrations.md) | Cobertura marcada por etapa; ausência de demonstração continua explícita. |
| Demonstrações examinadas em pelo menos três referências | Novas mídias Conductor, Cline Kanban e Superset, além de Vibe Kanban e [OpenCode](opencode-media-observation.md) | Frames de mídias efetivas, sem promover storyboards a reprodução contínua. |
| Adotar/adaptar/descartar informa escopo e spikes | [Posicionamento](product-positioning.md), [integração](integration-decisions.md), decisões nesta conclusão | Prioridades P0/P1 e responsáveis por validação definidos. |
| Três referências de interação anotadas | [Linear, Raycast, GitHub Desktop e critérios Fluent](experience-decisions.md) | Hierarquia, ações, versão, foco, adaptação, teclado e acessibilidade fundamentados. |
| Limitações levam a trabalho verificável | Tabela de lacunas abaixo; POS-01 a POS-16; ACC-01 a ACC-07 | Piloto D1 e evidência técnica M0/M1/M2 continuam pendentes. |

As capturas e vídeos publicados pelos fornecedores podem ser editados, históricos ou seletivos. O pacote não fornece baseline de produtividade, taxas de falha, satisfação de usuários ou desempenho comparativo.

## Direção de produto resultante

**Orchestrix é um workspace local para conduzir desenvolvimento assistido por agentes até um resultado revisado e aplicável, com escolhas configuráveis e rastreáveis.** A pessoa formula o trabalho, fornece contexto, acompanha, lê código/diff, pede correções e controla a entrega dentro do aplicativo.

O primeiro público proposto é o desenvolvedor que já usa um agente de código e quer reduzir a coordenação manual entre etapas e conversas. O primeiro cenário precisa funcionar com uma única conexão: implementar → verificar → revisar → corrigir → aplicar, sequencialmente quando necessário. Múltiplos runtimes/contas ampliam esse fluxo, conforme sua viabilidade; não são exigência para começar. Essa segmentação é uma hipótese a validar no piloto, sem entrevistas inventadas.

A pesquisa encontrou sobreposição direta: Superset documenta workers mistos e coordenação; Cline documenta Teams, Kanban e escolhas por fase; Antigravity tem aplicativo separado do IDE; OpenHands possui superfícies de controle além do SDK. Executar muitos agentes, usar worktrees ou selecionar thinking não sustenta uma alegação de exclusividade. O diferencial proposto precisa aparecer na compreensão, continuidade e controle do ciclo completo. [Comparação e fontes](product-positioning.md), [abrangência atual](landscape-and-provenance.md)

## Respostas às perguntas de pesquisa

| Pergunta | Conclusão e consequência |
| --- | --- |
| O que coordenar automaticamente? | Escolha compatível de execução, encaminhamento de contexto, fila, verificações e próxima etapa. Começar com regras explícitas e um fluxo pequeno; planner/replanejamento avançados ficam em M4. [Posicionamento](product-positioning.md) |
| Como organizar a experiência? | Projeto e trabalho são a navegação principal; tarefa, tentativa, artefato e resultado ficam relacionados. Agente/modelo/sessão explicam a execução no inspector. [Benchmark](experience-decisions.md) |
| Como representar atenção? | Motivo, impacto e ação na tentativa correta. Pergunta, permissão, limite, check falho e perda de sinal têm respostas distintas; atualização não rouba foco. [Atenção/revisão](attention-and-review-benchmark.md), [controle](workflow-risk-patterns.md) |
| Como encaminhar contexto e feedback? | Fontes e versão por tentativa, comentário junto ao diff e retorno à revisão. A sessão nativa permanece vinculada à conexão; troca de runtime não é transplante de conversa. [Posicionamento](product-positioning.md) |
| Que contratos investigar? | Adapter nativo primeiro; protocolo negociado quando necessário. Acrescentar comparação de OAuth oficial próprio para uso do plano ChatGPT, separada do experimento com login Codex existente. [INT-01 a INT-10](integration-decisions.md), [contas/assinatura](subscription-account-ux.md) |
| Como oferecer conforto e identidade? | Hierarquia, densidade ajustável, painéis recolhíveis/redimensionáveis, modo foco, teclado, estados legíveis e temas sem mudar a semântica. Studio é o padrão escolhido; galeria de aparência nas preferências. [Critérios visuais](experience-decisions.md) |
| O que fica fora do primeiro app? | IDE completo, debugger/test explorer, browser/frota/cloud completos, todos os adapters ao mesmo tempo e ranking adaptativo sem dados. Leitura/diff e correção assistida fazem parte do primeiro fluxo; edição leve depende de D1. [Escopo](product-positioning.md) |

## Decisões para o próximo incremento

As escolhas abaixo são recomendações de descoberta, respeitando os ADRs aceitos. P0 significa necessário para a experiência inicial; não significa que todo recurso precisa ser implementado antes de um protótipo simulado.

| ID / prioridade | Decisão | Motivo e evidência | Onde validar |
| --- | --- | --- | --- |
| D0-01 / P0 | Adotar entrada curta por objetivo e critérios; plano detalhado opcional. | Valor desde uma assinatura e uma correção pequena. [POS-01/02](product-positioning.md) | OX-D03/05; M2 tarefa manual. |
| D0-02 / P0 | Adotar trabalho + atenção + conexões + preferências, com inspector contextual. | Orientação estável e divulgação gradual. [Benchmark](experience-decisions.md) | OX-D03/04/05. |
| D0-03 / P0 | Adaptar presets para preferência automática ou escolha fixa, mostrando origem e suporte. | Capabilities variam por conta/runtime; solicitado e efetivo podem diferir. [POS-03/04/05](product-positioning.md) | OX-D03/05; OX-008. Nomes dos presets ainda provisórios. |
| D0-04 / P0 | Adotar Connection, Session e CapacityGroup distintos no domínio e na linguagem. | Mais sessões/hosts não garantem mais cota; registro de conta tem identidade própria. [Contas](subscription-account-ux.md) | ACC-01/02/03/06; OX-002 e M3. |
| D0-05 / P0 | Adotar Context Pack inspecionável com fontes, escopo e versão. | Handoff e importação têm limites; contexto útil precisa ser compreensível. [POS-07](product-positioning.md) | OX-D03/05; OX-008. |
| D0-06 / P0 | Adotar atenção contextual e preservar seleção/foco durante eventos. | Espera, falha e decisão não têm a mesma ação. [POS-08/12/13](product-positioning.md) | OX-D03/04/05; OX-011/015. |
| D0-07 / P0 | Adotar diff + critérios + checks + findings como superfície de revisão. | Comentários perto do código aparecem nas demos; nova alteração pede revisão correspondente. [Demos](workflow-demonstrations.md), [Conductor](conductor-dynamic-evidence.md) | POS-09/10/14; OX-013/015. |
| D0-08 / P0 | Separar aceite interno da tarefa de aplicação final do Run ao destino. | PR aberta ou botão pronto não comprova entrega; findings e checks mantêm papéis distintos. [Controle Git](workflow-risk-patterns.md) | POS-10/11; OX-014. |
| D0-09 / P0 | Adotar estado desconhecido e reconciliação quando faltar confirmação. | Restore de interface, resume de sessão e término de processo têm contratos distintos. [Atenção/recuperação](attention-and-review-benchmark.md) | POS-12; OX-001/007/011. |
| D0-10 / P0 | Refinar Studio e manter galeria de temas em Preferências → Aparência. | Escolha explícita do responsável; foco/alerta/diff precisam funcionar em cada tema. [Protótipo](../../prototypes/desktop/README.md) | OX-D04/05; POS-15. |
| D0-11 / P0 | Priorizar Windows, teclado, escala, leitura e layouts adaptativos. | Ambiente nativo e WSL precisam ser identificados; largura extra deve melhorar trabalho sem impor painéis. [Benchmark](experience-decisions.md) | POS-15/16; OX-D04/05 e OX-001. |
| D0-12 / P1 | Avaliar edição leve com indentação depois do fluxo de leitura/correção. | Pode ajudar pequenos ajustes; exige controle de escrita e invalidação de evidência. | OX-D03/05 decide alpha ou incremento posterior. |
| D0-13 / P1 | Ampliar concorrência, segundo runtime, pooling e regras por papel incrementalmente. | Domínio preparado não implica integração comprovada; uma execução útil vem primeiro. [Integração](integration-decisions.md) | M0/M3, conforme matriz de capabilities. |
| D0-14 / posterior | Manter API/Core independentes do editor; extensão VS Code segue o app local. | Reutiliza execução e histórico sem duplicar o scheduler no cliente. [Plano](../DEVELOPMENT_PLAN.md) | OX-010; cliente de editor em M5. |

## Lacunas preservadas, responsáveis e gates

| Evidência que falta | Consequência | Responsável / momento de resolver |
| --- | --- | --- |
| Pergunta/permissão respondida, queda/retomada e aplicação final em todas as referências | Não recomendar a interação de um concorrente como comprovadamente superior; as cenas não certificam enforcement. | Pesquisa complementar se uma decisão concreta de D1 depender desse detalhe. Os passos atuais permanecem rotulados nos relatórios. |
| Compreensão e conforto do Orchestrix em uso real | Temas aprovados e testes automáticos não equivalem a piloto. | Produto + responsável em OX-D05; desenvolvedores externos antes de OX-017. |
| Duas contas isoladas, capacidade e origem de cobrança | Nenhuma promessa de pooling/cota extra ou fallback de assinatura sem evidência. | Engenharia em OX-001/OX-002; habilitação conforme M3. |
| OAuth próprio, renovação e retomada no caminho oficial de uso do plano | Documentação não certifica a instalação nem os tokens do aplicativo. | Engenharia em OX-001/OX-002, depois do piloto inicial D1. |
| Cancelamento terminal e árvore de processos no Windows; resume após crash | Não liberar locks nem substituir escritor com estado incerto. | Engenharia em OX-001/007/011. |
| Versão/base, checks e reconciliação Git no produto | Não permitir aprovação antiga sobre resultado novo ou marcar entrega antes do efeito confirmado. | Engenharia em OX-006/009/014; simulação de compreensão em D1. |
| Licença e manutenção de dependência que venha a ser incorporada | Benchmark conceitual não é decisão de reutilizar código. | Engenharia verifica versão/arquivo antes de adicionar dependência; [proveniência](landscape-and-provenance.md). |

Nenhuma pesquisa externa reproduziu autenticação pessoal, usou assinatura de concorrente, executou merge em repositório remoto ou avaliou participantes nesta etapa. A tentativa de observar players via navegador foi encerrada pela política de Computer Use por URL indeterminável; a análise continuou por mídias públicas, sem contornar a política.

## Passagem para D1

1. **OX-D03:** completar entrada sem projeto, conexão, contexto/arquivos, tarefa curta e objetivo com dependências; desenhar sucesso e atenção essencial. Usar POS-01 a POS-14 e ACC-01 a ACC-07 como cenários, sem exigir execução real.
2. **OX-D04:** refinar Studio, tokens, componentes, resize/recolhimento, modo foco, teclado e estados de autenticação, limite, conflito e reconexão. Preservar o acesso **Personalizar aparência →**, a galeria e os nomes de temas em inglês já solicitados.
3. **OX-D05:** executar piloto inicial com o responsável, registrar ajuda/erros e corrigir problemas críticos. Decidir edição leve e nomes dos presets com evidência do piloto. Avaliação externa continua antes de concluir o alpha.
4. **Retomar M0:** comparar modalidades Codex/CLI/app-server, incluir INT-10 e validar worktree, retomada e descendentes. Depois construir M1 e integrar o Desktop em M2.

O [status](../DEVELOPMENT_STATUS.md), o [backlog](../DEVELOPMENT_BACKLOG.md) e o [handoff](discovery-handoff.md) passam a usar esse encerramento. Os relatórios de cada rodada conservam seus métodos e limites; esta consolidação é a referência atual para o aceite de D0.

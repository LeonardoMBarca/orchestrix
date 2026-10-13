# M0 — resultado do primeiro harness

**Continuação atual de supervisão — 13/10:** após a correção de ambiente e regressões 131/21/9, uma nova autorização foi executada integralmente: **cinco turnos reais passaram**. [App-server](evidence/codex-m0-job-live.json) comprovou edição/cinco checks, reinício/retomada e força do Job próprio durante turno iniciado (137, árvore vazia, streams drenados; terminal remoto ausente). [CLI](evidence/codex-m0-exec-live.json) comprovou edição/checks e retomada exata. A [comparação](cli-app-server-comparison.md) escolhe **app-server para o primeiro adapter local de uma assinatura**, liberando início de OX-003. OX-001 conserva diagnósticos completos/OAuth próprio pendentes; M0/OX-002 continuam abertos. Os dois turnos bloqueados antes da correção permanecem evidência separada. A matriz atual por versão/ambiente está na [continuação Windows](windows-process-containment.md#integração-do-transporte--1210); as tabelas abaixo permanecem históricas.

Registro de **8 de outubro de 2026**, Windows nativo. Entrega: [harness executável](../../tools/runtime-harness/README.md), [fixtures offline](../../tools/runtime-harness/fixtures/README.md) e [inspeção do schema local](codex-protocol-spike.md). Experimento em Node 24.19.0/Git, separado do protótipo e do futuro Core Rust. Não declara M0 ou adapter de produção concluídos.

**Retomada em 12/10:** o [reteste humano de D1](../design/d1-pilot.md#piloto-reportado-pelo-responsável--1210) liberou M0. O [novo incremento de OX-001](m0-resume-worktree-increment.md) prepara persistência/retomada entre app-servers e alteração em worktree com diff/checks, passou 75/75 offline, 9/9 Windows sintéticos e preflight real sem inferência em `0.162.0-alpha.17.2`. O [registro live e correções posteriores](m0-live-validation-2026-10-12.md) preserva a tentativa inicial bloqueada e registra uma segunda autorização de três turnos: MXC passou edição pelo modelo/cinco checks, reinício/retomada da mesma thread com política de escrita e cancelamento do turno em 36,27 s. O harness usado passou 95/95 offline na revisão anterior. Árvore real e comparação CLI continuam pendentes; qualquer nova inferência continua opt-in. As tabelas seguintes são o resultado histórico de 08/10, com suas versões e limites.

## Registro histórico — 08/10 e preparação de 09/10

As seções seguintes preservam o estado e os gates dessas datas; para o estado atual, usar a continuação de 12/10 acima. O percurso foi uma antecipação técnica. [D0 foi consolidado como pesquisa](d0-conclusion.md); D1 e seu piloto inicial são a próxima etapa. O incremento técnico abaixo é para a retomada posterior de M0, conforme o plano atualizado; não é aceite da modalidade OAuth própria acrescentada pela pesquisa.

**Continuação em 09/10:** [48 testes offline e correções de lifecycle](offline-lifecycle-validation.md), incluindo descendente sintético ativo depois da saída do pai. Não houve nova autenticação/inferência e o gate humano D1 continua pendente. Os dados live abaixo permanecem os de 08/10; a preparação não amplia suas capacidades reais demonstradas.

## O que foi demonstrado

| Camada | Resultado observado | Limite |
| --- | --- | --- |
| Fake | Sucesso, permissões, autenticação ausente, rate limit, JSON inválido, crash, timeout, interrupção, Unicode fragmentado, stderr extenso e notification desconhecida exercitados. | Respostas e códigos sintéticos; não representam o serviço. |
| Codex/metadados | Handshake stdio; autenticação ChatGPT; necessidade de autenticação OpenAI; disponibilidade ordinary positiva; provider/endpoint oficiais resolvidos. | Uma instalação e um armazenamento de autenticação já existente; catálogo não é entitlement. |
| Codex/live | Resposta constante correspondeu; primeiro turno `completed`; segundo turno confirmado como `interrupted`. | Dois turnos triviais, sem implementação de código ou retomada. |
| Configuração | Solicitação/resolução coincidiram: `gpt-6.1-sol`, effort `low`, provider `openai`, `readOnly`, rede de ferramentas desativada, `never`, reviewer `user`, cwd esperado, ephemeral. | Configuração resolvida não é telemetria empírica do modelo servido. |
| Ambiente | Hooks/plugins/apps desativados, notify vazio, um MCP configurado desativado por override do processo após reinício e reconfirmação. | Config pessoal não foi editada; caches normais do runtime podem mudar. |
| Encerramento | Ambos os processos próprios do percurso live encerraram com exit code 0. | Descendentes não foram inventariados; término da árvore continua desconhecido. |
| Repositório | Hashes/lista de arquivos do repositório descartável permaneceram iguais. | Não certifica sandbox Windows ou a execução posterior de ferramentas. |

**Verificação automatizada:** 23 testes offline passaram, cobrindo também respostas fora de ordem, ACK perdido, terminal antes do ACK, isolamento sintético por tentativa, limites cumulativos, estados terminais conflitantes, formato de configuração e exclusão de credenciais do ambiente. Nenhuma inferência faz parte dessa suíte.

Versão observada: **codex-cli 0.162.0-alpha.2**. O catálogo foi consultado pelo protocolo; o modelo default anunciado foi escolhido explicitamente com `low`, também anunciado. O percurso live durou **5.879 ms**, recebeu **37 mensagens** no processo de execução e produziu **28 eventos normalizados/diagnósticos**. Duração é observação de um cenário trivial, sem interpretação como benchmark de produtividade.

O [relatório sanitizado do percurso live](evidence/codex-subscription-probe.json) conserva a evidência local. Nenhum transcript, credencial, e-mail ou account ID foi salvo. A documentação e código da tag exata fundamentam os gates; as respostas locais são a evidência do resultado desta instalação. [Protocolo e fontes oficiais](codex-protocol-spike.md)

## Matriz provisória de capacidades

`supported` significa demonstrado neste cenário/versão; `unknown` permanece pendente. `runtime-managed` identifica responsabilidade do Codex, sem promessa de controle pelo Orchestrix.

| Capacidade | Estado | Evidência/pendência |
| --- | --- | --- |
| Stdio/handshake/JSONL | supported | Fake e servidor local. |
| Descobrir autenticação sem expor dados | supported | `account/read`; tipo e booleans conservados. |
| Uso pela assinatura nesta conexão | supported | Gates positivos e turno completado; sem API fallback. |
| Catálogo modelo/reasoning | supported | `model/list`; não prova todos os modelos executáveis. |
| Escolha explícita de modelo/effort | supported | Uma combinação solicitada/resolvida; outras ainda não testadas. |
| Modelo/effort empiricamente executados | unknown | Resposta não atesta telemetria do serving. |
| Interromper turno simples | supported | ACK e terminal `interrupted`. |
| Cancelar ferramenta/árvore inteira | unknown | Nenhuma ferramenta em andamento ou teste de descendentes. |
| Rejeitar aprovações | supported no fake; unknown live | Comando e permissão sintéticos; não pedir operações só para forçar aprovação nesta rodada. |
| Disponibilidade instantânea ordinary | supported | Boolean observado; percentuais, resets/créditos não registrados. |
| Independência de cotas/duas contas | unknown | Uma autenticação; dois processos fake comprovam somente correlação. |
| Conversação/compaction | runtime-managed | Não duplicar contexto interno do runtime no Core. |
| Retomada de sessão real | unknown | Thread ephemeral; resume real requer spike separado e vínculo de Connection. |
| Código em worktree/checks/revisão | unknown | Fora do cenário trivial. |
| CLI `exec --json` comparada | unknown | Ajuda/documentação inspecionadas; sem percurso equivalente real. |
| Claude/Antigravity | unknown | Não disponíveis no PATH nesta máquina; nenhum install/login automático. |

## Decisão para a próxima implementação

O **app-server é o candidato para o primeiro adapter**, por oferecer sessão, requests de aprovação e interrupção estruturados. O resultado valida seu percurso mínimo, sem fixar suporte a uma alpha como API estável. Antes de aprovar OX-001, completar comparação CLI/retomada e supervisão Windows; depois versionar o contrato de produção em OX-003. O fake atual é uma ferramenta desse trabalho, não uma implementação do scheduler.

Não instalar todos os provedores para começar um worker de um runtime. OX-002 segue a matriz oficial por modalidade e a prova independente de contas; ausência de segunda conta não autoriza copiar login ou trocar a identidade da sessão existente.

Próximo incremento técnico delimitado, depois do piloto D1: executar alteração pequena em worktree descartável, capturar diff e checks da mesma tentativa, testar retomada não ephemeral e contenção/cancelamento da árvore com descendente próprio. A observação sintética preparada em 09/10 mantém o resultado desconhecido e não substitui essa prova. O resultado orientará o supervisor Rust/Windows e os limites do adapter. Persistência/reconciliação e aplicação no repositório real entram apenas nos tickets correspondentes.

## Estado dos gates

- **OX-001 em andamento:** percurso básico real validado; comparação de superfícies, retomada e árvore ainda pendentes.
- **OX-003 experimento preparatório:** eventos, framing e fake testados; contrato final aguarda promoção de um runtime e integração no Core.
- **OX-D01 concluído como pesquisa:** conforme a [consolidação explícita do método](d0-conclusion.md), preservando lacunas dos percursos dos concorrentes e sem certificar runtimes.
- **OX-D05 pendente:** feedback de temas e automação não substituem piloto com tarefas.

Nenhum gate foi removido ou considerado concluído para iniciar Desktop de produção.

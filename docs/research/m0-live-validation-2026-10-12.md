# OX-001 — validação live e diagnóstico Windows

Registro de **12/10/2026**, com duas autorizações explícitas do responsável, cada uma limitada a três turnos Codex pela assinatura existente. O gate piloto D1 permanece liberado. O trabalho continua o [incremento de retomada/worktree](m0-resume-worktree-increment.md), sem alterar a interface, o Core proposto ou os milestones.

**Estado atual:** o segundo percurso, com MXC explícito e harness corrigido em `6f4ff95`, **passou** em 36,27 s: edição pelo modelo, cinco checks, retomada da mesma thread sob a mesma política e cancelamento do turno confirmados. A árvore Windows e a comparação CLI continuam pendentes; OX-001/M0 não estão concluídos.

## Primeira autorização — resultado parcial preservado

Na primeira autorização foram enviados **três pedidos `turn/start`**: um na tentativa original e dois na continuação diagnóstica. Não houve fallback API, troca de conta, instalação, desativação do sandbox ou escrita do runtime no Orchestrix. A thread persistida permanece no histórico do Codex, conforme o escopo informado.

| Prova | Evidência | Resultado e limite |
| --- | --- | --- |
| Edição pelo modelo | [Tentativa original](evidence/codex-m0-worktree-live-failed.json) | O turno terminou `completed`, mas o diff ficou vazio e o arquivo permaneceu defeituoso. Os cinco checks posteriores não foram executados; artefato recusado. |
| Causa da escrita bloqueada | [Diagnóstico](evidence/codex-m0-windows-diagnostic.json) | A ferramenta reportou `windows unelevated restricted-token sandbox cannot enforce split writable root sets directly; refusing to run unsandboxed`. A causa foi observada no resultado da ferramenta, além da explicação do modelo. |
| Retomada real | [Continuação em read-only](evidence/codex-m0-readonly-resume.json) | Após reinício do app-server, a mesma thread completou um turno e recuperou o marcador anterior, ausente do novo prompt. Prova de contexto interno do runtime nesse perfil; não é memória do Orchestrix. |
| Identidade e permissões | Continuação | Modelo/effort/cwd/política foram confirmados, mas o perfil mudou explicitamente de workspace-write para read-only. O binding compara somente a conexão diagnóstica e a conexão retomada; o binding original em memória não foi retido. Não prova identidade contínua desde a edição nem quotas independentes. |
| Cancelamento | Continuação + diagnóstico posterior | O RPC `turn/interrupt` foi rejeitado sem início ou terminal observado para o terceiro turno no stream. Uma leitura posterior encontrou esse turno persistido como `interrupted`; reconcilia o estado, sem provar causalidade do pedido ou cancelamento supervisionado. |
| Processos e arquivos | Relatórios acima | Os pais acompanhados encerraram com código 0. A fixture original permaneceu igual. A árvore Windows continua `unknown`; a consulta diagnóstica não certifica término de todos os processos. |

A prontidão `ready` reportada para `unelevated` não torna essa combinação de raízes graváveis compatível. Rede desativada, exclusões de temporários e limites de escrita foram preservados. A tentativa original continua falha, sem diff ou checks aprovados.

O diagnóstico consultou somente a thread própria identificada pelo cwd do fixture. Seu rollout foi lido de forma limitada para classificar o resultado da ferramenta. Não foram versionados transcript, raciocínio, marcador, IDs reais, binding, credenciais ou paths pessoais. As evidências preservam configurações sanitizadas, eventos normalizados, contagens e hashes dos arquivos próprios.

## Correções do harness

- A raiz temporária passa por `realpath` antes do Git/runtime, evitando que o alias Windows 8.3 e o caminho longo apareçam como identidades de cwd diferentes.
- `waitForTurnStart` observa somente turnos próprios, com deadline, teto de observadores e limpeza no término/erro. ACK, notificações estrangeiras, timeout e transporte fechado não confirmam início nem liberam admissão incerta.
- Os dois probes usam a mesma observação de cancelamento: início próprio + ACK do pedido + terminal interrompido. Um terminal antecipado ou RPC rejeitado permanece evidência de estado, sem fabricar ACK ou sucesso do percurso.
- O controle negativo da fixture exige falha de assertion aritmética com exit 1. Timeout, erro de spawn ou checker ausente não contam como baseline validado. Uma primeira leitura MXC teve `exitCode:null`: sua configuração foi observada, mas a fixture daquele preflight é inconclusiva. Esse achado motivou o guard e a repetição estrita somente de metadados.
- **`--windows-sandbox mxc`** é uma escolha explícita limitada ao Windows. O override vale apenas para os subprocessos próprios e deve ser confirmado por `config/read` antes de live e novamente no reinício. Sem confirmação, bloqueia. Não executa setup nem altera a configuração pessoal.

## Preparação MXC sem inferência

A [documentação oficial Windows](https://learn.chatgpt.com/docs/windows/windows-sandbox), consultada em 12/10, descreve MXC em dispositivos compatíveis e provisionamento administrativo para o fallback legado `elevated`. A compatibilidade não foi presumida a partir da página: o comando fixo de echo passou na instalação local com MXC explícito, sem instalação ou setup.

Em uma **nova fixture**, um [comando Node fixo pelo sandbox CLI](evidence/codex-m0-mxc-deterministic.json) alterou somente `src/add.mjs`, com cinco checks aprovados, diff vinculado e checkout principal intacto. Isso demonstra essa operação determinística, sem certificar isolamento adversarial, rede efetiva ou edição produzida pelo modelo. O relatório registra as restrições solicitadas; não é confirmação de política por `thread/start`.

O [preflight MXC estrito](evidence/codex-m0-mxc-metadata.json) passou: configuração `windows.sandbox=mxc`, assinatura/ordinary/endpoint, extensões desativadas e baseline com assertion falha confirmada. Não criou thread nem executou inferência. O processo pai acompanhando esse preflight encerrou com código 0; término da árvore permanece desconhecido.

Comando executado na segunda autorização:

```powershell
cd tools/runtime-harness
node worktree-probe.mjs --windows-sandbox mxc --live
```

Sem `--live`, o comando é apenas preflight. Sem a opção de sandbox, a seleção pessoal continua valendo. MXC explícito não autoriza fallback para full access, setup automático ou API paga.

## Segunda autorização — percurso live MXC aprovado

O responsável respondeu **“Autorizar os três turnos”** para uma nova tentativa. O repositório estava limpo na branch `docs/development-plan`, commit `6f4ff95`; a implementação não foi alterada durante a execução. Foram utilizados exatamente os três turnos deste novo escopo, em uma nova thread persistida. Nenhuma inferência adicional foi executada depois deles.

O [relatório sanitizado](evidence/codex-m0-mxc-live.json) registra `passed`, versão `0.162.0-alpha.17.2`, modelo `gpt-6.1-sol`, effort `low` e MXC explícito. A escolha foi confirmada por `config/read`, com os gates de assinatura/ordinary/endpoint e extensões reconfirmados pelo harness antes da execução e após reinício. Aprovação `never`, rede de ferramentas desativada e exclusões de temporários foram preservadas.

1. **Edição real:** o baseline terminou com assertion aritmética falha/exit 1. O modelo alterou somente `src/add.mjs`, de subtração para soma. O [patch de 207 bytes](evidence/codex-m0-mxc-live.patch) tem SHA-256 `f1781046ad772d7ebe30fd29c15412664df36ee9fa0855581053b25894687023`; cinco checks passaram no mesmo snapshot. Base e checkout principal temporário ficaram intactos.
2. **Retomada real com escrita:** após encerrar/reiniciar o app-server, `thread/resume` conservou a mesma thread e a identidade observável da conta comparada em memória desde a conexão inicial. As configurações solicitadas e resolvidas iniciais/retomadas são iguais, incluindo workspace-write. O turno completou e recuperou o marcador do contexto anterior, ausente do novo prompt. O diff e os arquivos permaneceram iguais. Essa continuidade pertence ao runtime; não demonstra memória ou recuperação SQLite do Orchestrix.
3. **Cancelamento do turno:** houve início próprio observado, ACK de `turn/interrupt` e terminal `interrupted`, nessa ordem. O artefato e os checks permaneceram iguais. É cancelamento cooperativo confirmado desse turno sem ferramentas, sem comprovar encerramento de subprocessos de ferramentas ou contenção da árvore inteira.

Os **117 eventos** têm sequência contínua e registram três inícios/três terminais (`completed`, `completed`, `interrupted`). Os dois pais acompanhados encerraram com código 0 e stdio fechado; ambos mantêm `treeTermination: unknown`. Não há IDs reais, paths pessoais, binding, marcador, credenciais ou conteúdo de conversa no JSON versionado. Somente o nome do patch foi ajustado para o arquivo de evidência estável; o artefato original fica em `artifacts/` local ignorado.

Essa prova sucede a falha anterior, sem reclassificá-la como sucesso. Também não certifica isolamento adversarial de MXC, quotas independentes, outros modelos/contas ou execução de produção.

## Verificação e gates

- **95/95 testes offline passaram em 31,67 s**, incluindo os 75 anteriores, caminho canônico, dezesseis casos de início/cancelamento, seleção MXC e controle negativo. Check de sintaxe passou.
- As rodadas intermediárias passaram 92/92 e 94/94; a última antecedeu o guard de baseline. Nenhum critério ou deadline foi enfraquecido para o resultado final.
- A prova **9/9 Windows em 33,97 s** é a rodada anterior do helper sintético, que não foi alterado nem reexecutado neste incremento. Não é supervisão integrada do Codex.
- A biblioteca de referências permanece fora do Git; nenhum código ou script upstream foi executado ou incorporado.
- **Nova rodada live:** os três turnos autorizados e o percurso de diff/checks/retomada/cancelamento passaram em 36,27 s. Hash do patch, estados, configurações, ordem dos eventos, privacidade e links foram conferidos. Os 95 testes offline pertencem à revisão anterior, com o mesmo código; não foram repetidos neste incremento de evidência/documentação.

**OX-001 e M0 continuam abertos.** Continuação posterior: a [bridge Windows integrada](windows-process-containment.md#integração-do-transporte--1210) passou provas sintéticas e preflights reais sem inferência. O live aprovado acima permanece anterior, sem bridge. A nova rodada usou dois dos cinco turnos autorizados, um app-server e um CLI: ambos sem diff por falha do launcher MXC; retomada/força não executadas. A omissão de `SYSTEMDRIVE` foi reproduzida e corrigida sem relaxar proteções; comando fixo/cinco checks passou nos dois caminhos. Os resultados reais bloqueados e o diagnóstico permanecem separados da correção, que ainda não tem prova live. Terminal remoto, runtime, helper, stdio e árvore permanecem separados. A [comparação CLI](cli-app-server-comparison.md) registra os resultados e as limitações; comando determinístico não a substitui. OX-003 mantém a dependência do primeiro runtime aprovado; Desktop continua simulado e avaliação externa D1 em OX-017.

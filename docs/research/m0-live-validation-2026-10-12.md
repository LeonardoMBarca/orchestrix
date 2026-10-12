# OX-001 — validação live e diagnóstico Windows

Registro de **12/10/2026**, após a autorização explícita do responsável para três turnos Codex pela assinatura existente. O gate piloto D1 permanece liberado. O trabalho continua o [incremento de retomada/worktree](m0-resume-worktree-increment.md), sem alterar a interface, o Core proposto ou os milestones.

## Percurso autorizado e resultado

Foram enviados **três pedidos `turn/start` ao todo**: um na tentativa original e dois na continuação diagnóstica. Não houve fallback API, troca de conta, instalação, desativação do sandbox ou escrita do runtime no Orchestrix. A thread persistida permanece no histórico do Codex, conforme o escopo informado.

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

Para a próxima prova, após novo opt-in:

```powershell
cd tools/runtime-harness
node worktree-probe.mjs --windows-sandbox mxc --live
```

Sem `--live`, o comando é apenas preflight. Sem a opção de sandbox, a seleção pessoal continua valendo. MXC explícito não autoriza fallback para full access, setup automático ou API paga.

## Verificação e gates

- **95/95 testes offline passaram em 31,67 s**, incluindo os 75 anteriores, caminho canônico, dezesseis casos de início/cancelamento, seleção MXC e controle negativo. Check de sintaxe passou.
- As rodadas intermediárias passaram 92/92 e 94/94; a última antecedeu o guard de baseline. Nenhum critério ou deadline foi enfraquecido para o resultado final.
- A prova **9/9 Windows em 33,97 s** é a rodada anterior do helper sintético, que não foi alterado nem reexecutado neste incremento. Não é supervisão integrada do Codex.
- A biblioteca de referências permanece fora do Git; nenhum código ou script upstream foi executado ou incorporado.

**OX-001 e M0 continuam abertos.** O próximo passo é repetir a edição/retomada/cancelamento com MXC explícito, mediante novo opt-in porque os três turnos autorizados foram usados. Depois, integrar supervisão da árvore real Windows e completar comparação CLI `exec --json`. O comando determinístico `sandbox` não substitui essa comparação. OX-003 mantém a dependência do primeiro runtime aprovado; o Desktop continua estudo funcional com execução simulada, sem integração com este harness. Avaliação externa D1 permanece em OX-017.

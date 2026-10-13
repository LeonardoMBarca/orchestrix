# OX-004 — domínio e estados em memória

Data: **13/10/2026**. Entrada: `docs/development-plan`, `d5f587f`, árvore limpa. O responsável pediu duas partes, sem reorganização ampla: orientação do repositório e, depois, OX-004. O primeiro commit **`1a9afae`** criou o guia raiz AGENTS.md e reorganizou docs/README.md; 79 links/âncora foram verificados, sem mover arquivos, alterar relatos históricos ou duplicar status no guia. A implementação começou somente após esse commit.

## Entrega e fronteiras

[domain/](../../crates/orchestrix-core/src/domain/mod.rs) usa o crate existente em módulos coesos. [Contrato do domínio](../contracts/domain-and-states.md) liga suas regras à arquitetura, plano, backlog e ADRs existentes, sem outra máquina concorrente ou novo planejamento.

Entidades implementadas: Project/Repository distintos, Connection, CapacityGroup, Run, Task, TaskAttempt, Conversation/PendingComposer, referências de sessão e de trabalho/versionamento, ActionMode e AgentProfile. IDs e estados são protegidos por tipos/campos privados. O agregado Run expõe comandos validados e acesso somente leitura às Tasks/tentativas. Um projeto aceita zero conversas; só o primeiro envio não vazio salva uma Conversation. Associar depois conserva identidade, mensagens e snapshots anteriores; links ao trabalho capturam entidades/revisões reais.

O OX-003 é consumido pelo mesmo RuntimeObserver em TaskAttempt, sem mensagens Codex ou SDK/Node/Windows/Tauri/banco. Requested/effective, desconhecidos, identidade/revisão, origem, terminal e budgets conservam as regras. Uma execução completed sem artefato produz AwaitingArtifact, não Task Completed. Código exige verificação/revisão e integração validada no Run; análise pode concluir com artefato aceito. Aplicação final e entrega são passos separados, vinculados a resultado/base exatos, com conflito explícito preservando os artefatos.

Novas tentativas selam a anterior somente após validar ownership e origem da correção. Capturam contexto/versão, definições, projeto/base/worktree e artefato anterior; mudanças de definição não alteram snapshots. Uma sessão não troca conexão/revisão/adapter nem recebe dois turnos ainda ativos/incertos. Cadastro global entre Runs continua responsabilidade do host/SessionRegistry; não há isolamento real de contas ou reservas de capacidade demonstrados.

Capacidade compartilhada é Unknown ou observação explícita; duas conexões não ganham cotas independentes por usar o mesmo adapter. Estratégia de recursos permanece no pedido OX-003, distinta do modo/perfil; não foi implementado router/resolver ou motor especializado. Subscription Only é verificado antes da criação da tentativa; dados de teste não autorizam login/API/fallback.

## Casos e correções demonstrados

| Área | Cobertura nova |
| --- | --- |
| [Workspace, conversas e definições](../../crates/orchestrix-core/tests/domain_workspace.rs) | **14 testes**: projeto vazio; composer sem persistir; primeiro envio; ordem/identidade; associação; referências de trabalho/plano/resultados; mesmos adapters com sessões distintas; capacidade desconhecida; revisões imutáveis. |
| [Lifecycle integrado](../../crates/orchestrix-core/tests/domain_lifecycle.rs) | **11 testes**: código serial → integração → dependência de análise → candidato → aprovação → aplicação → entrega; completed insuficiente; falha/rejeição; correção; bases/versões; incerteza/cancelamento; sessão única por turno; bloqueio agregado. |
| [Fronteira do runtime](../../crates/orchestrix-core/tests/domain_runtime.rs) | **4 testes**: ownership/contexto/cobrança; terminal factual sem start/ACK e configuração unknown; saída/árvore/ACK sem terminal; conflito final invalidando aprovação e preservando artefatos. |

A revisão encontrou e corrigiu bugs concretos antes do aceite: cleanup tardio não pode desfazer Ready/Cancelled; correção precisa partir da revisão do artefato rejeitado; referências diretas de sessão não podem contornar o binding no Run; trabalho independente não pode limpar Blocked de uma tarefa incerta; histórico exato de artefato não reaproveita aprovação. As regressões preservam os guards e snapshots, sem ampliar os critérios para fazer testes passarem.

## Verificações desta rodada

O responsável autorizou acrescentar **somente rustfmt e Clippy** à toolchain já instalada. Rust permanece 1.99.0; rustfmt **1.10.0-stable**, Clippy **0.1.99**. Não houve outra instalação, atualização da toolchain ou dependência nova. A primeira execução Clippy encontrou dois avisos nos arquivos novos (if colapsável e clone para criar slice), corrigidos antes do check estrito.

| Comando/checagem | Resultado |
| --- | --- |
| `cargo check --workspace --all-targets --locked --offline` | Passou |
| `cargo build --workspace --examples --locked --offline` | Passou |
| `cargo test --workspace --locked --offline` | **35/35 testes de integração**, sem falhas/ignores: 29 de domínio e seis OX-003 existentes |
| `rustfmt --check --edition 2024 --config skip_children=true <arquivos afetados>` | Passou; somente os oito módulos novos, três testes novos e lib.rs foram formatados/conferidos, sem formatação massiva dos arquivos anteriores |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | Passou, sem avisos/allow para ocultá-los |
| `npm.cmd run check:conformance` | Passou, **245 casos/935 operações**, fixture e hashes existentes sem regeneração, zero dispatches |

Fontes JS, transporte/supervisão e wire não mudaram; não repetimos suas suítes 320/11/Windows como se fossem resultados desta rodada. A conformance Rust existente foi reexecutada e todos seus checkpoints continuam passando. Os exemplos foram compilados, não promovidos a daemon. Não houve Git/projetos de usuário executados pelo domínio, provider, inferência, login, cofre ou fallback pago; os únicos Git deste trabalho foram os comandos de manutenção deste próprio repositório.

## Aceite e próximo incremento

**OX-004 concluído no recorte puro em memória.** Critérios demonstrados: transições inválidas recusadas; conta/runtime único e fluxo serial; duas conexões/sessões sem mistura; nova tentativa preserva a anterior; critérios diferentes para código/análise; distinção de integração/aplicação final; projeto/conversa/primeiro envio e versões de definição. A suíte integrada usa o contrato estabilizado, não só getters/construtores.

Isso não demonstra persistência, processos, autenticação independente, isolamento de worktrees, checks reais dos projetos ou Desktop de produção, e **não conclui M0/OX-001/OX-002**. SQLite, Git, supervisor, scheduler, daemon, UI e refresh ficaram fora desta rodada. As evidências de verificação/integração/aplicação são entradas explícitas de teste do host confiável; os serviços futuros precisam produzi-las de verdade.

**Próximo incremento permitido: OX-005 — persistência, migrações, eventos e idempotência**, dependente de OX-004. O spike deve escolher o acesso SQLite e transações sobre este domínio, sem trocar de estado/contrato nem implementar todos os tickets. OX-006 e OX-008 também conservam as dependências da fila, sem serem iniciados neste incremento.

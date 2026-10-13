# OX-003 — compatibilidade de entrada portátil e aceite

Data: **13/10/2026**. Base: `58cdf6b`, branch `docs/development-plan`, árvore limpa na entrada. Continuação do [consumo Rust](rust-runtime-contract-increment.md), sem reabrir D1/escolha do app-server, alterar interface, iniciar OX-004 ou chamar providers.

## Problema demonstrado e correção

A diferença já registrada de timestamp sem fuso não era a única: `Date.parse` aceitava rollover de calendário/24:00/horário sem segundos; UTC fora do ano de quatro dígitos não normalizava de forma idempotente. Chaves com surrogate isolado e números `1e400` em campos conhecidos porém não utilizados produziam aceitações/erros diferentes. O parser numérico Rust também arredondava `9007199254740991.1` para um inteiro diferente do JavaScript.

O [perfil de entrada revision 1](../contracts/runtime-input-profile.md) torna o domínio explícito nos dois consumidores. Gramática e calendário de timestamps são validados sem timezone do host. O guard lexical iterativo verifica Unicode/números finitos antes de parsing, inclusive campos ignorados/sobrescritos. A feature `serde_json/float_roundtrip` corrige o arredondamento; Cargo.lock não mudou e nenhuma nova dependência/componente foi instalada. IDs/tokens JavaScript agora rejeitam newline terminal, uma diferença concreta causada pelo anchor `$`. A ordem de validação Rust foi alinhada para inputs com vários campos inválidos; a bridge aceita revisão JSON `1.0` como o mesmo inteiro 1.

**Compatibilidade:** a versão semântica wire continua 1.1. O perfil, exportado por ambos, é obrigatório e identificado no corpus/bridge. Esta substituição do decoder experimental anterior à produção rejeita explicitamente inputs não portáveis; não alega compatibilidade aditiva com todas as entradas de `Date.parse`. Não há importador automático, fuso inventado, alteração das evidências históricas ou downgrade. Os **104 casos anteriores, descritores, pedidos e três hashes de origem foram comparados com `58cdf6b` e permanecem idênticos**. O corpus acrescenta casos; não redefine expectativas antigas para esconder regressões.

## Implementação e testes independentes

[runtime-input.mjs](../../tools/runtime-harness/src/runtime-input.mjs) e [runtime_input.rs](../../crates/orchestrix-core/src/runtime_input.rs) implementam o mesmo codec. Observadores/fake/adapter existentes usam a fronteira atual. [runtime_observe](../../crates/orchestrix-core/examples/runtime_observe.rs) exige `{inputProfile, descriptor, request, frames}` do host confiável; perfil ausente/incompatível falha antes de criar o observer. Não é endpoint de produção.

Os [vetores escritos explicitamente](../../tools/runtime-harness/fixtures/portable-input-v1.json) têm **37 timestamps e 23 frames**, com UTC/erro/counter esperado independente do oracle. Testam datas normalizadas/legadas, Gregorian/ano zero/centúrias, offset/frações/limites, Unicode, duplicatas, overflow/underflow e arredondamento. Três testes Rust adicionais percorrem as tabelas, incluindo evidência por conexão/modelo e cleanup após quarentena. Os testes JavaScript também executam subprocessos próprios com `TZ=UTC`, `America/Sao_Paulo` e `Asia/Tokyo`; são provas de independência da timezone, não de execução macOS/Linux.

A [conformance](../../crates/orchestrix-core/tests/fixtures/runtime-conformance-v1.json) cresce para **245 casos/935 operações**, incluindo as mesmas datas nos minors 0, 1 e 9 e erros simultâneos com prioridade explícita. Compara resultado, erro e snapshot completos: identidades, origem, configuração, terminal, budgets e cleanup. Mantém todos os sete cenários do fake e distinções entre evidências reais/sintéticas. Perfil/hash dos novos vetores são registrados; reprodução LF/CRLF continua sem modificar reports históricos.

## Verificação desta rodada

| Verificação | Resultado |
| --- | --- |
| `cargo check --workspace --all-targets --locked --offline` | Passou |
| `cargo build --workspace --examples --locked --offline` | Passou, exemplo Windows compilado/executável |
| `cargo test --workspace --locked --offline` | **6/6 testes de integração**, sem falhas/ignores; **245 casos/935 operações** na conformance |
| `npm.cmd run check:conformance` | Passou; corpus reproduzível e perfil explícito, zero dispatches |
| `npm.cmd run test:rust-core` | **11/11**, sem falhas/skips; adapter → Rust, erro idêntico sem timezone, rejeição de perfil ausente/incompatível e revisão integral JSON `1.0` |
| `npm.cmd run check` | Passou |
| `npm.cmd test` | **320/320**, sem falhas/skips; 255 existentes e 65 do novo perfil, execução final 49,254 s |
| Comparação com corpus `58cdf6b` | 104 casos/descriptors/requests/três hashes existentes idênticos |

Transporte/supervisão nativos não mudaram; não repetimos as suítes separadas Windows 21/9. A regressão default executou seu caso Job sintético. Nenhuma inferência, consulta/login de runtime, API paga, instalação ou código de concorrente foi executado/incorporado. Artefatos/target e bibliotecas locais permanecem fora do commit.

## Confronto com OX-003 e continuação

**OX-003 concluído no recorte do primeiro runtime local single-account:** os sete cenários fake exigidos estão cobertos; Core Rust recebe o mesmo contrato sem tipos de provider/OS/Node; campos desconhecidos compatíveis são projetados/descartados, e incompatibilidades geram erros fixos. A integração offline atravessa o adapter existente, em vez de somente produzir documentação/fake desconectado. O perfil fecha a diferença registrada de decoding com domínio e compatibilidade explícitos, sem prometer equivalência com todo input JavaScript legado.

Este aceite **não conclui M0/OX-001/OX-002**: diagnóstico completo/OAuth próprio, outros provedores/contas e gates de distribuição continuam pendentes. Não certifica um adapter/supervisor Rust de produção, autenticação independente, contenção macOS/Linux, quotas ou refresh periódico. Runtime mantém sua conversa; memória Orchestrix e persistência ficam nos tickets previstos. Estratégias/presets, Subscription Only e configuração manual foram preservados.

**Próximo incremento:** OX-004, domínio/estados, usando o contrato estabilizado e preservando as pendências de M0 em seus tickets. OX-004 não foi iniciado nesta rodada; SQLite/OX-005, scheduler, worker/OX-007 e Desktop continuam posteriores. Não há ação humana ou nova autorização live necessária para começar os modelos/transições puros.

# OX-003 — consumo do contrato no Core Rust

**Continuação posterior:** o [incremento de compatibilidade portátil](portable-runtime-input-increment.md) resolveu o limite descrito ao final e concluiu OX-003 no recorte local single-account. Este documento conserva os resultados e o bloqueio encontrados em `58cdf6b`, sem apresentá-los como status atual. A wire permanece 1.1; o perfil explícito substitui o decoder preproduction ambíguo sem alegar compatibilidade com toda entrada JavaScript antiga.

Data: **13/10/2026**. Base: `31f15ad`, branch `docs/development-plan`, árvore limpa na entrada. O alinhamento de produto permanece no commit documental separado. Esta rodada executa a instalação autorizada e a passagem técnica do contrato 1.1 ao Rust; não inicia OX-004, SQLite, scheduler ou outra interface.

## Ambiente preparado e compilação real

O responsável autorizou explicitamente Rust stable x64/MSVC pelo rustup para seu usuário, com Cargo/rustc/rustup e entrada padrão no PATH, reutilizando MSVC/SDK existentes e sem outros componentes ou inferência.

Foi utilizado o [instalador oficial](https://rust-lang.org/tools/install/) em `static.rust-lang.org`, SHA-256 **`6f4bef66261261fcb43131be8720bab817d403a09edec7455c371974b90bdb7e`** (registro de bytes baixados, não assinatura/atestado adicional). O primeiro lançamento com nome prefixado foi recusado como proxy desconhecido; renomear a cópia para `rustup-init.exe` permitiu concluir a instalação. Perfil `minimal`, host `x86_64-pc-windows-msvc`, canal stable: **rustc 1.99.0** (`b940084d7`, 28/09/2026), **Cargo 1.99.0** (`5f94df478`). A entrada de `%USERPROFILE%\.cargo\bin` no PATH do usuário foi confirmada; na sessão existente os comandos usaram o caminho completo.

MSVC/linker **14.50.35717** e bibliotecas SDK **10.0.26100.0** já estavam instalados. Cargo compilou e executou testes/exemplo Windows, demonstrando a disponibilidade efetiva do linker, não só descoberta de arquivos. Não houve instalação de Visual Studio, SDK, providers ou outros componentes da toolchain. Dependências de projeto foram resolvidas em Cargo.lock; checks/builds/testes posteriores passaram com `--locked --offline`. Isso não certifica macOS/Linux nem versões mínimas de Rust.

## Implementação e fronteiras

O [workspace Cargo](../../Cargo.toml) inicia somente [orchestrix-core](../../crates/orchestrix-core/README.md), com [runtime_contract](../../crates/orchestrix-core/src/runtime_contract.rs). O módulo contém normalização/validação, decisão de capacidade, preflight explícito, observador de uma tentativa, erros tipados e snapshots. Não importa Node, SDK/mensagens Codex, tipos Windows/Tauri, banco, relógio ou supervisor nativo.

Wrappers públicos validados de descritor/pedido mantêm internals privados; snapshots são cópias próprias. Conexão/revisão, sessão/tentativa/turno, versão/sequência, modelos/níveis por modelo, solicitado/observado, estados de auth/cobrança/disponibilidade e budgets conservam a semântica do recorte comparado. Preflight continua distinto de registrar uma admissão/terminal factual. Subscription Only recusa API/fallback pago; API explícita exige a referência de autorização e superfície correta, sem executar nada por essa validação.

`EventOrigin` vem do host fora do frame. A origem supervisor não pode criar resultado remoto; IDs/evidências estruturais não são prova de ownership do OS. Quarentena fatal permite cleanup independente dentro do budget; completed/interrupted exigem terminal observado, e força da raiz/Job conserva unknown quando o terminal está ausente. Streams posteriores acumulam confirmação/fault sem apagar fatos anteriores.

O parser projeta campos conhecidos e valida/descarta JSON desconhecido pelo mecanismo de [IgnoredAny](https://docs.rs/serde_core/latest/serde_core/de/struct.IgnoredAny.html), sem materializar seus payloads como árvores recursivas. Os testes cobrem 500 níveis, tipo inválido em campo conhecido e JSON malformado mesmo no trecho descartado. Bytes incluem o frame original inteiro, antes dessa projeção. A biblioteca usa [serde_json](https://docs.rs/serde_json/latest/serde_json/enum.Value.html) para valores/decodificação e [chrono](https://docs.rs/chrono/latest/chrono/struct.DateTime.html) sem clock para datas UTC; nenhum código/asset dos clones de referências foi incorporado.

## Corpus portátil e pipeline integrado

[runtime-conformance-v1.json](../../crates/orchestrix-core/tests/fixtures/runtime-conformance-v1.json) registra **104 casos/309 operações** e resultados esperados de construção/operações/snapshots. [export-conformance.mjs](../../tools/runtime-harness/export-conformance.mjs) usa a especificação JavaScript/fake/replay existentes como oracle, com nomes únicos e hashes dos dois relatórios sanitizados e da fixture Codex. A geração é explícita/offline e determinística; `--check` compara os bytes sem reescrever expectativas. Um caso por linha permite atribuir diffs de dados ao nome correspondente. Mudança no oracle exige revisão do diff, não regeneração para esconder falha.

`.gitattributes` fixa LF nas fontes Cargo/Rust e no corpus. Os hashes da nova conformance usam **`hashMode: lf-normalized-input-bytes`**, preservando a distinção dos hashes brutos registrados anteriormente no replay. Um teste transforma os inputs em CRLF/LF em memória e confirma corpus/expectativas/hashes idênticos; os relatórios históricos no disco não foram reescritos. Isso valida reprodução textual, não execução macOS/Linux.

Os casos incluem os sete cenários obrigatórios do fake, eventos normalizados dos relatórios históricos, três replays explicitamente sintéticos, ordem pré/pós-ACK, configuração factual versus preflight, interromper/forçar sem terminal inventado, auth/rate limit/timeout/crash, revisão/identidade/duplicatas, major incompatível/minor futuro, bytes UTF-8, limites/overflow, sanitização e fechamento irreversível. Cada checkpoint compara resultado/erro e snapshot completo, incluindo eventos, origens e contadores. JSON object ordering não altera a comparação.

Os [três testes Rust](../../crates/orchestrix-core/tests/conformance.rs) consomem o corpus sem Node, conferem presença/distinção das provas reais/sintéticas e verificam erros sem mensagens privadas e snapshots independentes. Não são 104 processos/probes: são 104 casos de dados dentro da suíte de conformance.

O [exemplo offline runtime_observe](../../crates/orchestrix-core/examples/runtime_observe.rs) recebe um bundle do host confiável em stdin, limitado a 4 MiB, e usa a mesma biblioteca. Não é daemon, auth gate ou endpoint para aceitar provenance do provedor. A [suíte opt-in do pipeline](../../tools/runtime-harness/tests/rust-core.spec.mjs) lança somente a fixture própria e esse consumidor: `CodexSession → CodexRuntimeAdapter → contrato → Rust`, nos cenários success/hang/rate limit/malformed, com igualdade completa ao observer JavaScript. O fake provider-neutral usa o mesmo executável. Não houve Codex real/login/inferência nesta rodada.

Os reports históricos seguem sem catálogo/ACK ou vínculo explícito de processos ausentes; o Rust não reconstrói esses campos. A prova sintética de força retém raiz 137/helper 0/árvore confirmada e terminal remoto unknown. Os resultados reais anteriores não foram reexecutados ou ampliados pelo corpus.

## Verificações desta rodada

| Execução | Resultado |
| --- | --- |
| Instalação/versões/PATH e compilação/link Windows | Confirmados |
| `cargo check --workspace --all-targets --locked --offline` | Passou |
| `cargo build --workspace --examples --locked --offline` | Passou |
| `cargo test --workspace --locked --offline` | **3/3 testes**, corpus **104 casos**, sem falhas/ignores; execução final de testes em 0,21 s, após build 1,92 s |
| `npm.cmd run check:conformance` | Corpus reproduzível, zero provider dispatches |
| `npm.cmd run test:rust-core` | **9/9**, 3,147 s, sem falhas/skips; inclui um teste que **expõe** a diferença de timestamp legado, sem aprová-la como paridade, e reprodução CRLF/LF |
| `npm.cmd run check` e `npm.cmd test` | Check passou; **255/255**, 61,124 s, sem falhas/skips |

Não houve alteração no transporte/supervisão Windows existente; as suítes separadas 21/9 não foram repetidas. A regressão de 255 inclui o caso Job com runtime sintético, executado nesta rodada. Instalação, binaries/target, transcript e artefatos temporários não entram no commit; Cargo.lock, fontes próprias e fixture sanitizada entram.

## Situação de OX-003 e próximo recorte

**Entregue neste incremento:** consumo efetivo no Core Rust, sem tipos específicos de provedor/OS, com pipeline integrado offline e conformance do corpus versionado. O impedimento de toolchain foi removido. Não há worker/adapter de produção ou daemon Rust; transportar/supervisionar runtime nativo no Rust pertence a OX-007.

**OX-003 permanece em andamento por uma diferença de parsing explicitamente encontrada:** o oracle JavaScript usa `Date.parse`, que aceita timestamps sem timezone e depende do ambiente; o consumidor Rust exige RFC3339 com timezone, compatível com as datas normalizadas emitidas pelos adapters. Um teste demonstra que o primeiro aceita `2026-10-13T00:00:00` e o segundo retorna `protocol`, sem registrar execução/valor conhecido. Não alegar paridade universal, reduzir critérios ou permitir datas ambíguas para encerrar o ticket. Também não certificar equivalência com strings JavaScript não Unicode por terem passado as fixtures usuais.

O próximo trabalho permitido é fechar essa fronteira do wire de timestamps/decodificação em OX-003: definir a gramática portátil explícita e sua compatibilidade/versionamento, testar entradas legadas/ambíguas e normalizadas nos dois consumidores e consolidar o aceite. Não requer inferência ou novo piloto; não transforma atualização periódica em pré-requisito. OX-004 não foi iniciado. Diagnóstico completo/OAuth próprio de OX-001, outros provedores/contas de OX-002 e M0 continuam com seus gates. Desktop, estratégias/presets, editor auxiliar e refresh permanecem nos marcos existentes.

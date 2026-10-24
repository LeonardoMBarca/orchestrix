# Codex 0.164: qualificação offline do adapter

Data: 2026-10-24. Recorte: Windows, `0.164.0-alpha.11.1`, comparada ao schema preservado de `0.162.0-alpha.17.2`. A autorização desta rodada cobre exportação pública e testes offline; não cobre leitura autenticada, login ou inferência. O estado do lote continua nos documentos centrais de desenvolvimento.

A [documentação oficial do App Server](https://learn.chatgpt.com/docs/app-server) descreve a exportação de JSON Schema. Foram usados os comandos oficiais `app-server generate-json-schema`, com e sem `--experimental`, em diretório e `CODEX_HOME` vazios, sem iniciar um listener. O [manifesto sanitizado](evidence/codex-0164-schema.json) fixa versões, tamanhos, hashes e recibos locais, sem caminhos pessoais ou respostas de conta.

## Origem e comparação

Os executáveis distribuídos pela extensão VS Code e pelo Desktop apresentam a mesma versão e tamanho, mas **SHA-256 diferentes**. Cada export estável contém 320 arquivos; cada export experimental, 458. Dentro de cada modalidade, os inventários e bytes exportados pelas duas builds são idênticos. Isso comprova equivalência dos schemas exportados, sem certificar equivalência de execução dos binários.

A auditoria comparou **32 schemas e 13 definições críticas**. Os campos efetivamente enviados/consumidos pelo adapter anterior continuam presentes no export estável. As 13 definições, incluindo sandbox, aprovação, turno, erro e janelas de capacidade, permanecem idênticas. Campos opcionais e métodos experimentais novos não concedem suporte ao produto: `experimentalApi:false` permanece obrigatório.

| Superfície | Diferença relevante e tratamento |
| --- | --- |
| Conta | `Account` acrescenta `gcp`; `PlanType`, `sci`/`sci_plus`. API, Bedrock e GCP continuam recusados. O perfil nativo ainda exige e-mail observado não vazio; o schema permitir `null` não altera esse gate. |
| Routing | `workspaceRouting` opcional contém `chatgptAccountId`, `backendOrigin` e `accountRoutingOverride` (`NO_CONSTRAINT`, `us`, `us_cr`). O adapter usa esses campos somente como fence opaco de vinculação em 0.164, com validação e limites. Não certificam identidade imutável, conta independente ou origem de cobrança. A projeção das versões antigas permanece intacta; consumo de cota não entra no fingerprint. |
| Capacidade/configuração | Os tipos consumidos de rate limits permanecem; `Config.apps` opcional não substitui `features.apps=false`. Uso explicitamente permitido, Subscription Only, provider/URLs, MXC, ferramentas/delegação e demais restrições continuam necessários. Schema não comprova enforcement real. |
| Modelos/esforço | `ModelListParams/Response` são byte-idênticos. `ReasoningEffort` continua string não vazia, não enum fechado. Tradução é explícita por versão, catálogo e confirmação efetiva; não há equivalência universal de Maximum/Ultra. |
| Sessão/resultado | Campos obrigatórios anteriores permanecem; surgem metadados opcionais. `outputSchema`, ramo `agentMessage` e estados de turno preservam o contrato consumido. Uma forma de resposta compatível não comprova revisão independente real. |
| Notificações | `thread/readState/changed` é informativa. O guard valida thread, revisão, aliases e posição histórica; não adota o turno antigo como atual nem concede início, término, capacidade ou evidência. Globals desconhecidos continuam recusados. |
| Requests | `currentTime/read` aparece somente no export experimental. Mantém-se a recusa genérica de requests não implementados, sem habilitar outra ferramenta. |

## Verificação e limites

Convergência aprovada sobre os oito arquivos Rust fixados no manifesto: formatação pertinente, workspace check/all-targets/all-features, Clippy estrito no mesmo escopo, build dos exemplos, conformance **245/245** sem regeneração e fronteira Rust **11/11**. Comandos, perfis, hashes de fonte/saída e tempos estão no manifesto; o recibo completo permanece local em `artifacts/codex-0164-verification-1792882083429/VERIFICATION.json`.

| Testes reexecutados | Resultado/perfil |
| --- | --- |
| Biblioteca runtime | 10/10, debug |
| Novo perfil 0.164 | 9/9, release; processos/Jobs/stdio reais, protocolo sintético |
| Runtime existente | 28/28, release; um teste autenticado ignorado |
| Conexões/daemon/SQLite | 8/8, debug; dois autenticados ignorados |
| Revisão/correção/recuperação | 4/4, debug, Git/SQLite/processos reais e runtime sintético |
| Preflight de check antes de metadata/reserva/processos | 1/1 focal de `manual_worker`, debug |
| Host Desktop | 22/22, debug |

A compilação release do novo alvo custou **9m48s**, seguida de **3,85 s** de testes; a suíte de revisão executou em **998,28 s**, além da compilação. Não são tempos de inferência. Os demais comandos têm duração registrada separadamente. A suíte ampla `manual_worker` historicamente interrompida não foi promovida a aprovada: somente o guard focal acima foi reexecutado.

Falhas intermediárias preservadas: a primeira integração sintética teve 6/9 por cenários positivos sem escrita e expectativa incorreta de contador por processo após restart. Corrigiu-se a fixture para editar e usou-se o ledger durável existente para contar os dois dispatches; os asserts de bytes finais foram mantidos. A execução focal seguinte passou 9/9 em debug. Outra convergência debug registrou timeout de conexão (500 ms), com 8/9; o caso isolado passou. A causa permanece não diagnosticada. Os dois alvos de processos passaram em release com os mesmos limites/asserts; isso não representa correção demonstrada da causa do timeout debug.

Não houve acesso à conta, turno, login, mudança de credenciais ou instalação neste recorte. Autenticação, cobrança, catálogo efetivo, retomada, permissões e resultado real em 0.164 ainda exigem a prova delimitada correspondente. O checker, a tarefa congelada e os resultados anteriores de OX-017 permanecem preservados; a autorização live anterior não se transfere automaticamente à nova versão.

Build normal congelado em `artifacts/codex-0164-normal-1792884189147/product-bin/`: Core/supervisor release, Desktop debug com frontend embarcado e identificador novo/exclusivo de QA. Sem `test-fixtures`, Vite em execução, bundle, instalação ou abertura do aplicativo. Fonte funcional **d4ce8771/234 arquivos** conferida antes/depois; sidecars staged e efetivamente copiados pelo Tauri comparados por SHA-256. Frontend **76/76**, typecheck/build e build nativo passaram. Rust Desktop compilou em **31,92 s**; comando Tauri completo **58.674 ms**. Aviso Vite de chunk **599,42 kB/500 kB** preservado, sem alterar o limite. O manifesto fixa os hashes dos três executáveis; geração de build não certifica a conexão real ou QA nativa nesta versão.

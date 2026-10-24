# OX-017 — critérios e checker da próxima prova

Auditoria local em 24/10/2026, iniciada em `d94e33e`, branch
`docs/development-plan`. Não é revisão independente executada pelo produto.
O resultado anterior permanece **NeedsWork**, sem revisão/correção/aplicação.

## Fontes conferidas

Proposta privada preservada:
`artifacts/dogfooding/direct-checker-real-proposal-1792814468416-79c5d964/`.
READY.json SHA-256 `ccbac74509a9d81a0dff74860cb639055fa896b4936ec6fe3c22a4076f46c5be`;
READY.md `ebfc1974774ff6c5163286a593efec4a5261b52713981ddfcb277cb61617d81f`.
Fonte antiga `src/lib.rs` tem hash
`b92a01769629447acfebee7762cb101ca1f14b3e4691cb7464ffeb27645aa99e`;
binário `e5a84a0fc4705513a04f1a38ba78b640a37591eec60da5db495bc90a3fe0fd8e`.
As sete fontes, proposta e evidências antigas foram conferidas sem alteração.

Os quatro critérios recuperados diretamente de READY são idênticos aos do
Context Pack persistido e à prévia nativa registrada. O worker envia
`context.rendered()`; esse corpo contém `acceptanceCriteria`, e o adapter o
encaminha no input de `turn/start`. Isso é rastreio de código, contexto e
observação local, não uma nova captura do transporte ou chamada à conta.
O [contrato canônico v2](../../tools/context-ownership-checker/proof-contract.json)
preserva C1–C4 literalmente, sem reconstruí-los de um resumo.

Classificações: **a** explícito/mecânico; **b** semântico; **c** exigência adicional
do checker antigo; **d** conclusão não sustentada pelo teste.

| Critério → origem | Avaliação antiga e evidência | Conclusão |
| --- | --- | --- |
| C1 → READY, prévia e Context Pack: prefixo original, apêndice 160–2.048 B, UTF-8/LF/final LF, uma seção H2 | Artefato tem prefixo exato de 7.170 B + 1.635 B; essas propriedades não foram reprovadas | **a**. Originalidade/English/pertinência são **b**, não comprovadas pelo tamanho |
| C1 → uma seção H2 | `heading-scope` exige palavras literais `workspace` e `context` e até 120 unidades UTF-16; título produzido foi “Context inclusion, preservation and access” | **c**: palavras/teto não foram apresentados. Não demonstram inadequação semântica (**d**) |
| C2 → inclusão, edição, preservação e leitura real separados; ausência no pack não é sandbox | Trechos distinguem “An edit allowlist separately identifies which paths may change” e arquivo excluído/preservado ainda legível pelas ferramentas | **b**, com suporte textual local nesse aspecto; tokens não substituem o significado |
| C3 → preservação não certifica segurança nem autoriza divulgação/edição | “Preserving exact bytes establishes retention, not content safety.” O texto também nega certificação de segurança e autorização de divulgação/alteração | **b**, conceito explicitamente presente nesse aspecto. O padrão antigo requer adicionalmente `sensitive|secrets?|credentials?` (**c**); sua ausência lexical não comprova ausência do conceito (**d**) |
| C3 → restrição obrigatória de leitura exige enforcement ou bloqueio | “If enforcement cannot be demonstrated, block that combination” | **b**, suporte textual compatível com o contrato; não é aprovação global do documento |
| C4 → nenhum outro arquivo/índice alterado; revisão independente | Auditoria de captura/base/índice: 583 protegidos exatos. O checker lê somente um arquivo. Não houve revisão | **a** depende do Core e da proveniência, não só do checker. Sem parecer Accepted ou aprovação de aplicação |

Outras regras editoriais não publicadas na versão anterior: início/linhas em
branco exatos, restrições a outros headings, fences, Setext e HTML. Guards de
arquivo regular, leitura limitada e NUL são de formato/segurança; a nova fonte os
documenta. A auditoria não prova que todo o documento esteja correto; explica por
que os dois códigos antigos não sustentam, sozinhos, aquela reprovação semântica.

## Substituto delimitado

[`tools/context-ownership-checker`](../../tools/context-ownership-checker/README.md)
é uma ferramenta standalone v0.2.0, com contrato/hash próprios. C1–C4 permanecem;
C5 publica antes da execução um pequeno perfil ATX visível. Sem keywords de
título, teto artificial de 120 ou regex de conceitos. Nenhum exemplo contém a
resposta produzida na prova anterior ou um parágrafo final fornecido ao executor.

O checker verifica somente o prefixo, limites/encoding e estrutura. C2/C3 e os
aspectos semânticos de C1/C4 continuam obrigatórios na rubrica S1–S4 da revisão
independente. Uma contradição sintética passa na estrutura e é anotada como
rejeitável por S1/S2; não foi criado um parecer fictício do produto. Escopo completo,
índice e protegidos continuam sob o Core opaque-base-v2.

Saída limitada, sem conteúdo/caminhos privados; exit 0 mecânico, 1 estrutural,
2 falha de leitura/execução. Mesmo evaluator, argv vazio, pin e 5 s; nenhum wrapper
Node ou aumento do teto de 64 MiB. Dois casos encontrados na auditoria adicional
(comentários fabricando H2 e referências invisíveis contadas como prosa) receberam
correções e testes negativos antes do congelamento.

## Continuação e autoridade

A nova proposta usa outra cópia integral de **b053a7f**, 584 arquivos/54.451.461 B,
somente `docs/CONTEXT_OWNERSHIP.md` como editável/contexto explícito. Reutiliza os
três binários normais congelados da fonte funcional **184df78f**; nenhum Core,
Desktop, supervisor, API ou engine mudou. O mesmo identificador/banco exclusivo
de QA conserva histórico antigo; isso é declarado como reuso, não banco vazio.
Projeto/conversa/comandos/Task/alvo serão novos. Observações antigas não ganham
validade ou consentimento e a recorrência permanece desativada.

[Revisão/correção](../contracts/manual-worker.md#independent-review-and-bounded-correction)
exige artefato Verified e sessão distinta/read-only. Correção só é permitida após
**Rejected válido**, no mesmo Run/Task e orçamento; NeedsWork por check falho não
tem essa continuação comprovada. Na proposta: check falho para; Rejected válido
pode usar uma correção autorizada e nova revisão; erro técnico, indisponibilidade,
timeout ou incerteza param sem retry. Accepted, autorização humana e confirmação
durável de aplicação são fatos distintos.

READY identifica fonte/contrato/testes/binário/pins/base/contexto/política/dados e
resultados offline. Não concede acesso à conta. Metadata e turnos exigem novo
opt-in, sem reaproveitar saldos da prova encerrada. OX-017 continua parcial.

A verificação documental encontrou 390 links válidos e nenhuma falha introduzida.
Quinze ocorrências de âncoras antigas já ausentes em status foram registradas no
recibo local, separadas dos links novos; não foram corrigidas fora deste recorte.

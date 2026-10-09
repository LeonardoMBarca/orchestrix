# Conductor: evidência dinâmica do fluxo de trabalho

Pesquisa em 08/10/2026 para OX-D01. Complementa [o benchmark anterior](attention-and-review-benchmark.md) com o arquivo integral de uma demonstração oficial. Este registro não modifica o aceite, o backlog ou as dependências do plano.

## Fonte e método

A página oficial [Workflow](https://www.conductor.build/docs/concepts/workflow) incorpora [workflow.mp4](https://melty.t3.storage.dev/workflow.mp4). O arquivo foi obtido integralmente por HTTP público: **771.097.315 bytes**, **277,567 segundos**, **3340 × 2160**. SHA-256: `3F004D213BA41FE9AC1088671738774D59895A585C745EBC41D0A8EBB1EDFA30`.

Foi feita leitura de amostras de frames do próprio MP4, extraídas com Windows MediaClip/MediaComposition e inspecionadas visualmente. Há amostragem geral a cada 30 segundos, refinamento a cada 5 segundos nas ações relevantes e a cada segundo nas transições selecionadas. Isso oferece evidência dinâmica de estados sucessivos; **não equivale a reprodução audiovisual contínua nem a teste do produto**. A pesquisa não instalou o Conductor, autenticou contas ou executou agentes.

A data de publicação e a versão da gravação não foram confirmadas. Datas e versões presentes dentro da tela não certificam a UI atual. A obtenção completa supera a limitação de tamanho registrada na rodada anterior; a antiga mídia de AskUserQuestion, cujo host falhou em TLS, não foi solicitada novamente.

Frames inspecionados, em segundos:

```text
0, 5, 10, 15, 20, 30, 35, 36, 37, 38, 39, 40, 45, 50, 55, 60,
65, 70, 75, 80, 85, 90, 110, 115, 120, 125, 130, 131, 132, 133,
135, 140, 145, 150, 170, 175, 180, 185, 186, 187, 188, 189, 190,
195, 200, 205, 210, 215, 220, 225, 230, 235, 240, 245, 250, 251,
252, 253, 254, 255, 260, 261, 262, 263, 264, 265, 270, 275
```

O cache temporário desta sessão contém `conductor-workflow.mp4`, `conductor-workflow-<segundo>.jpg` e o helper `extract-video-frames.ps1`, sob `%TEMP%\orchestrix-journey-research`. Esses arquivos não são dependências permanentes do repositório. O helper aceita `-VideoPath`, `-OutputDirectory`, `-Seconds`, `-Prefix` e `-Width`.

## Percursos observados

Todos os fatos visuais abaixo têm como fonte o [MP4 oficial](https://melty.t3.storage.dev/workflow.mp4). Os tempos identificam frames efetivamente inspecionados; intervalos não significam que cada frame intermediário foi visto.

| Percurso | Cenas vistas | Limite |
| --- | --- | --- |
| Abrir/conectar | 0–30 s: home e diálogos Clone/Open. 36–39 s: New workspace → cópia/branch → setup concluído. | Clone concluído e autenticação não vistos. |
| Iniciar trabalho/contexto | 50 s: terminal `pnpm run dev`; 55 s: prompt; 60 s: Working. | Contexto automático e conta não demonstrados. |
| Acompanhar paralelo | 80 s: segundo workspace em setup; 90 s: ambos Working. | Não comprova múltiplas contas/cotas. |
| Atenção/recuperação | 250→251 s: check pendente → Ready to merge. 275 s: outra PR com Merge conflicts. | Não se vê atendimento de pergunta/permissão, recuperação de sessão ou resolução do conflito. |
| Revisar/corrigir/aplicar | 132→135 s: Review inicia chat. 175–185 s: comentários; 186–189 s: destinatário escolhido/envio; 200–210 s: duas linhas removidas. 240–245 s: PR aberta. 261→264 s: Merge → spinner → archive/Undo. | Resultado local visto; badge remoto Merged não certificado. |

225–235 s: o revisor apresenta preocupações; não se vê tratamento delas antes da integração. Essa ressalva impede interpretar Ready to merge como revisão sem achados. A [PR #32 exibida na gravação](https://github.com/meltylabs/conductor-marketing/pull/32) retornou 404 na consulta pública desta pesquisa; não houve validação externa independente do merge.

## Onde a documentação complementa a observação

O percurso de bloqueio do agente permanece **documental**: [Troubleshooting issues](https://www.conductor.build/docs/troubleshooting/issues) orienta abrir a sessão que precisa de input e responder; para execução travada, verificar aprovação/input, cancelar e enviar uma próxima ação; para checks/conflitos, encaminhar contexto, corrigir e repetir teste/revisão. A página foi aberta integralmente em 08/10/2026; não informa uma data de atualização. Esses passos não apareceram completos no vídeo.

[Security and permissions](https://www.conductor.build/docs/reference/security-and-permissions), também consultada nessa data, documenta possíveis aprovações de ferramentas e permissões locais da conta do sistema. Isso não demonstra o desenho, a confirmação ou a persistência dessa interação.

## Decisões propostas para a experiência do Orchestrix

As propostas seguintes são decisões de projeto a validar em D1, não capacidades atribuídas ao Conductor:

1. **Distinguir atenção do agente e atenção da integração.** Uma pergunta, uma permissão, uma execução interrompida e um check pendente precisam de causas e ações distintas. Esperar CI terminar não substitui o teste de uma resposta aceita pelo runtime.
2. **Manter destino e versão junto ao feedback.** Um comentário deve guardar arquivo, linhas, versão do artefato e agente destinatário. O usuário precisa ver envio pendente, confirmação do envio, alteração proposta e verificação humana como etapas separadas.
3. **Separar disponibilidade para merge de qualidade da revisão.** O resultado de um revisor pode conter perguntas ou riscos mesmo com CI verde. A tela de integração deve mostrar achados abertos, sua decisão explícita e a versão revista.
4. **Exibir uma operação de integração até o seu resultado.** Progresso de merge, confirmação remota, falha e estado do workspace precisam ser rastreáveis. Arquivamento e Undo não devem sugerir que desfazer a organização da tela desfaz um merge remoto.
5. **Recalcular riscos entre trabalhos paralelos.** Antes de integrar, atualizar a relação com a branch alvo e identificar conflitos. Não presumir que a aprovação de um artefato permanece válida depois de outro trabalho alterar a base.

## Situação da evidência

Esta rodada fortalece abertura de workspace, início, paralelo, revisão, correção e integração com uma demonstração oficial extensa. Atenção de integração tem uma transição observada; atendimento/recuperação do agente continua sem demonstração completa nesta referência. Login, contas múltiplas, cotas, falha de runtime e confirmação remota independente permanecem fora do observado.

A evidência deve ser combinada com outras referências e com o critério vigente de OX-D01. Este relatório não declara o gate concluído e não altera sua interpretação.

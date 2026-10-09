# D1 — Matriz de cenários e verificações posteriores

Preparada em **8 de outubro de 2026** a partir da [conclusão de D0](../research/d0-conclusion.md), [POS-01 a POS-16](../research/product-positioning.md), [ACC-01 a ACC-07](../research/subscription-account-ux.md) e [backlog](../DEVELOPMENT_BACKLOG.md). **Nenhuma linha significa piloto humano executado ou gate D1 concluído.** O roteiro e o registro estão em [d1-pilot.md](d1-pilot.md).

P0 D1 identifica a interação que precisa estar representada e navegável no estudo para avaliar compreensão/controle. Não exige autenticação, execução de código, filesystem ou Git reais no protótipo. A coluna posterior conserva o aceite de engenharia; observar a interface não prova o efeito que ela apresenta.

**Estado do incremento D1:** controles de entrada, conexões, contexto, catálogo, permissão, atenção, layout e conclusão das quatro tarefas da feature foram incorporados ao estudo. **QA técnica de 09/10:** `npm.cmd run check` e **24 cenários Playwright aprovados** — dez em `d1.spec.mjs`, nove em `workspace.spec.mjs`, cinco em `d1-accessibility.spec.mjs` — com Chrome 154.0.8037.98 headless, Playwright 1.64.0 e Node 24.19.0 no Windows. A [entrega D1](d1-delivery.md) registra versão, cobertura e limites; testes automatizados não substituem resultados humanos. Preencher versão/manifesto para o piloto. **Piloto humano pendente e gate D1 aberto**; nenhum resultado de uso foi registrado nesta atualização.

## Casos P0 D1

| Caso | Interação e variações necessárias | Requisitos associados | Preparação |
| --- | --- | --- | --- |
| P0-01 | Primeira entrada, modalidade do plano, identidade/estado, nova autorização enquanto há tentativa ativa, mesmo e-mail fictício e Windows/WSL explícito. | POS-01/16; ACC-01/02/03/04 | V01/V03 |
| P0-02 | Correção curta, objetivo com dependências, ciclo sequencial com uma conexão e tarefa aberta no mesmo Run. | POS-01/02; POS-11 em P0-07 | V02 |
| P0-03 | Automático/fixo, informação parcial, incompatibilidade por conexão, mudança de preferência com snapshot preservado. | POS-03/04/05/13; ACC-06 | V03 |
| P0-04 | Manifestos de contexto de implementação/revisão, duas sessões da mesma conexão, capacidade/modelo conhecido ou desconhecido. | POS-06/07/14 | V04 |
| P0-05 | Pergunta/permissão/check/limite/autorização pendente, negada e expirada; respostas e recuperação contextual. | POS-08/13; ACC-02/05/06 | V05 |
| P0-06 | Falha de check, diff/critério/finding, correção versionada, preservação de rascunho/foco/posição durante evento. | POS-07/08/09/15 | V06 |
| P0-07 | Aceite interno com outra tarefa do mesmo Run aberta, candidato completo, candidato alterado e base alterada depois da revisão. | POS-10/11 | V02/V06 |
| P0-08 | Bloqueio de novos trabalhos da conexão, cancelamento sem confirmação, reconciliação e desconexão/revogação com trabalho pendente. Suspensão mínima pelo daemon em M2; scheduler ampliado em M3. | POS-12; ACC-07 | V07 |
| P0-09 | Fluxo por teclado, retorno de foco, janela ampla/compacta, texto da interface 200%, densidade, fila redimensionável/recolhível e comparação de aparência. Zoom navegador/escala Windows têm registro separado. | POS-15/16 | V08 |

Os casos agrupam cenários para evitar 23 sessões isoladas. Isso não elimina variações: o registro precisa permitir identificar cada critério POS/ACC e cada subcaso exercitado. Trocar a explicação do facilitador por uma ação ausente não atende a prontidão nem produz evidência de compreensão.

## Rastreabilidade POS

| ID | P0 D1 / evidência observável no piloto | Verificação posterior e ticket |
| --- | --- | --- |
| POS-01 | P0-01/02: começa com uma conexão e percorre a sequência completa; entende próxima etapa sem cadastrar outra conta. | M1/M2 OX-009/013/017: ciclo real com um runtime; uso e avaliação externos em OX-017. |
| POS-02 | P0-02: correção curta exige objetivo/critério, sem plano extenso; distingue feature dependente e template. | M2 OX-015: tarefa manual; M3 DAG e M4 planner/replanejamento, conforme backlog. |
| POS-03 | P0-03: explica Automático/fixo, motivo/alternativas; opção incompatível não parece executável. | M0 OX-003: capabilities; M1 OX-008: resolução e fallback explícito; M2 OX-016: preferência. |
| POS-04 | P0-03: distingue solicitado, resolvido e informado/desconhecido; controle de esforço não parece acesso ao reasoning privado. | M0 OX-001/003: campos reais por versão; M1 OX-007/008 e M2 OX-015/016: proveniência/telemetria. |
| POS-05 | P0-03: prevê efeito antes de salvar; tentativa ativa conserva snapshot e nova tentativa usa a nova versão. | M1 OX-004/005/008: persistência/snapshot; M2 OX-016: alteração de política. |
| POS-06 | P0-04: encontra duas sessões, mesma conexão e capacidade compartilhada/desconhecida; não entende cota multiplicada ou cobrança trocada. | M0 OX-001/002: identidade/capacidade; M1 OX-004: Connection/Session/CapacityGroup; M3 pooling/fallback. |
| POS-07 | P0-04/06: localiza fontes, versão, escopo e omissões de cada pacote; revisão recebe diff/evidências pertinentes. | M1 OX-008: manifesto/envio; M2 OX-013/015: revisor e inspeção do contexto real. |
| POS-08 | P0-05/06: evento contextual explica motivo/ação, abre tentativa correta e conserva arquivo/rascunho/foco/posição. | M1 OX-005/010: eventos/cursor/reconexão; M2 OX-015: seleção e controle sob eventos reais. |
| POS-09 | P0-06: worker concluído + check falho bloqueia aceite; evidência e correção são da versão correta. | M1 OX-009 e M2 OX-013/014: checks vinculados ao artefato, correção e aceite. |
| POS-10 | P0-07: mudança de candidato **e** mudança de base invalidam confirmação antiga e conduzem a revalidação/revisão. | M2 OX-014: aprovação sobre diff/base exatos; crash entre efeito Git e persistência reconciliado. |
| POS-11 | P0-02/07: aceita tarefa com outra tarefa aberta **no mesmo Run**; distingue interno de aplicação final e entende o que falta. | M1 OX-004/006 e M2 OX-014: dependências, integração serial e aplicação confirmada. |
| POS-12 | P0-08: perda de sinal/cancelamento não confirmado ficam desconhecidos/reconciliando; não deduz término nem dispara worker duplicado. | M0 OX-001: supervisão/Windows; M1 OX-007/010/011: término, locks, idempotência e reconciliação. |
| POS-13 | P0-03/05: limite observado oferece espera, próxima execução ou conexão autorizada; entende modalidade e ausência de fallback automático para API. | M1 OX-007/008: classificação e cobrança; M3 cooldown/grupos de cota/fallback demonstrados. |
| POS-14 | P0-04: encontra sessão revisora distinta na mesma conta/modelo; não confunde runtime com modelo nem presume diversidade desconhecida. | M2 OX-013: sessão nova e evidências; M3 diversidade/segundo adapter conforme política. |
| POS-15 | P0-06/09: termina controles essenciais por teclado em ampla/compacta e texto da interface 200%; foco/diálogos/diff/logs não escondem ações. Usa densidade, resize e recolhimento da fila. | M2 OX-012/015/017: Desktop real, zoom/escala Windows/tecnologias assistivas registrados separadamente; avaliação externa. |
| POS-16 | P0-01/09: lê path longo com espaços/acentos, ambiente/cwd e variante WSL; não presume conversão ou suporte técnico validado. | M0 OX-001/002: matriz por ambiente/path/versão; M1 OX-006/007: path/cwd; M2 OX-012: seleção real. |

## Rastreabilidade ACC

| ID | P0 D1 / evidência observável no piloto | Verificação posterior e ticket |
| --- | --- | --- |
| ACC-01 | P0-01/04: duas ChatGPT + Claude têm rótulo/provider/modalidade/estado; distingue conexão/conta, perfil e sessão sem trocar login global. | M0 OX-001/002: seleção/isolamento; M1 OX-004: identidade; M2 OX-012: cadastro; habilitação multiaccount em M3. |
| ACC-02 | P0-01/05: adicionar outra conta deixa autorização pendente separada da tentativa ativa; prevê qual será afetada. | M0 OX-001/002: auth/lifecycle e identidade antiga; M2 OX-012; multiaccount em M3. |
| ACC-03 | P0-01: mesmo e-mail fictício/workspaces diferentes têm rótulos e identidade distintos; nenhuma captura/histórico mostra token/identificador sensível. | M0 OX-001/002: identidade estável/registro; M1 OX-005/007: redaction/persistência; M2 OX-012. |
| ACC-04 | P0-01: primeira conexão confirma uma vez o uso do plano; modalidade junto ao composer/modelo e gerenciamento de uso encontrável. | M0 OX-001 INT-10: modalidade oficial/autorização e lifecycle; M2 OX-012: fluxo e identidade visual oficial da modalidade, conforme requisitos vigentes. |
| ACC-05 | P0-05: distingue limite/consentimento negado/auth expirada, preserva tarefa e entende espera/uso/conexão elegível; renovar não é retry silencioso. | M0 OX-001/002: OAuth/auth; M1 OX-007/008: classificação/política; M2 OX-012/015; fallback em M3. |
| ACC-06 | P0-03/05: troca de conexão atualiza catálogo/capabilities e incompatibilidades; catálogo não garante acesso; snapshot antigo preservado. | M0 OX-001/002/003: catálogo e autorização por conexão/versão; M1 OX-008; M2 OX-016; multiaccount em M3. |
| ACC-07 | P0-08: separa parar admissões, término de worker, logout e confirmação de revogação; rede falha não vira revogação remota confirmada. | M0 OX-001/002: auth/lifecycle; M1 OX-007/010/011: processo/reconciliação; M2 OX-012; multiaccount em M3. |

## Prontidão da versão em preparação e limites

Inventário atualizado em 09/10/2026, após a passagem técnica completa. Os testes cobriram fluxos simulados, arraste real da fila, teclado, persistência, loading e regressões de identidade/candidato. Incluíram uma tarefa em R-12 fora do candidato R-08, aceite posterior de R-12 sem reabrir R-08 aplicado, base revalidada ainda bloqueada com tarefas abertas, seleção correta dos arquivos próprios de OX-24 e snapshots/contexto do produtor durante correção. A continuação corrigiu foco após entrada/cadastro e verificou cores forçadas/movimento reduzido pelo navegador e máximos Unicode a 320px. A cobertura detalhada está na [entrega técnica](d1-delivery.md). As nove tarefas de avaliação de uso continuam **não executadas por participante**.

| Caso | Controles do incremento D1 | Limites e próximo registro |
| --- | --- | --- |
| P0-01 | Botão do projeto → **Começar**; nome, ambiente Windows/WSL, path de exemplo, template/conexão inicial → **Preparar projeto de exemplo**; autorização pendente → **Continuar autorização simulada**, confirmação da modalidade e **Validar registro simulado**. Conexões têm rótulo/workspace/identidade fictícia e gerenciamento de uso simulado. | A entrada prepara um cenário novo e substitui o seed; não lê repositório, autentica conta nem abre gerenciamento real de assinatura. Não certifica identidade visual/fluxo oficial OAuth de produção. Registrar interpretação do participante. |
| P0-02 | **Correção curta · 1 tarefa** e **Feature guiada · 4 tarefas**; feature usa R-08. OX-24 e complementares iniciam/concluem/revisam/validam; OX-26 exige resposta e OX-27 depende do aceite de OX-24/OX-25. Aplicação fica bloqueada até as quatro tarefas aceitas. | Roteiro completo em V02/P0-02; conferir na passagem técnica. Complementares são fixtures explícitas. Não é planner, scheduler global ou agregação Git real. |
| P0-03 | Preferências e snapshot; catálogo nominal por conexão (**Modelo Codex de exemplo** / **Modelo Claude de exemplo**); trocar conexão atualiza opções; Automático escolhe conexão elegível/compatível e fixa incompatível bloqueia início. **Simular descoberta em andamento / Concluir descoberta simulada** mostram loading/bloqueio de novas tentativas; catálogo incompatível tem aviso próprio. | Nomes/capabilities são fixtures, sem descoberta de modelos reais. Modelo efetivo/esforço, entitlement e limites reais desconhecidos. Avaliar compreensão de solicitado/compatível/desconhecido; nenhuma cobrança muda implicitamente. |
| P0-04 | Inspector → **Inspecionar contexto e arquivos**; fontes/path/versão/trecho; **Preparar contexto para nova tentativa** conserva o pacote atual. **Contexto da revisão** identifica sessão/pacote separados e mantém fontes/conta/modelo solicitado da tentativa produtora durante correção. Histórico conserva configuração/contexto por tentativa. Conexões expõem grupo/capacidade. | Pacotes e sessões são fixtures; envio, diversidade e isolamento reais não comprovados. Avaliar se o participante distingue origem do artefato anterior, preferência nova e fonte/versão/escopo/capacidade. |
| P0-05 | Pergunta OX-26; check falho; OX-25 em execução → **Simular pedido de permissão → Revisar permissão**, com comando/cwd/alcance; recusar mantém esperando, permitir libera a mesma solicitação. Conexões têm pendência, identidade divergente, limite/expiração/acesso negado, catálogo incompatível e reconexão. | Permissão de comando e autorização de conta são estados diferentes. Não há comando real nem variante própria de permissão de alteração de arquivos. Limites/auth são fictícios. |
| P0-06 | Dois arquivos, rascunho, falha, correção e novo artefato. `Ctrl+Shift+E` acrescenta atenção de outra tarefa quando existe, atualiza contador sem rerender e preserva foco/arquivo/scroll. **Precisa de você** abre a tarefa correspondente. | Usar seed ou feature de várias tarefas para POS-08; em cenário com uma tarefa o evento pertence à própria tarefa. Confirmar foco/posição por QA e uso humano; check/finding são exemplos, sem execução real. |
| P0-07 | Aceite interno distinto de aplicação; feature no mesmo Run bloqueia com tarefas abertas, inclusive após revalidar base. Depois das quatro aceitas, revisão final inclui cinco arquivos/quatro tarefas de R-08; uma tarefa independente de R-12 fica fora. Confirmação tem **Simular candidato alterado** e **Simular base alterada**; exigem revalidação/revisão. | Três variações e feature passaram nos testes técnicos; compreensão/aprovação humana ainda pendentes. Aprovação/versionamento são em memória; enforcement Git, checks combinados e agregação real ficam em M2. |
| P0-08 | Perda de sinal; **Cancelar → Simular cancelamento sem resposta** → desconhecido → reconciliação confirma encerramento da mesma tentativa. **Desconectar para novos trabalhos** e **Simular revogação sem resposta** separam workers/logout/revogação. | Desconectar bloqueia novos trabalhos da conexão; não é suspensão global nem prova término/revogação. Comando mínimo real de admissões em M2 e scheduler ampliado em M3, ambos ausentes no estudo; avaliar se essa ausência afeta o participante. Não existem processos/tokens reais. |
| P0-09 | **Personalizar aparência →**; densidade confortável/compacta, alvos-base 44/36px; texto **Ampliado · 200%**. Fila por arraste ou `←`/`→`/`Home`/`End`; **Recolher/Mostrar tarefas**, **Restaurar layout**, foco e sete temas. Tema/densidade/texto/largura persistem no navegador. Arraste real, teclado e persistência verificados na suíte D1. | Texto 200% não é zoom navegador nem escala Windows. Recolhimento/foco são de sessão. Leitura/conforto humano, tecnologias assistivas e escalas Windows não certificados; registrar no piloto/avaliação posterior. |

## Itens posteriores que não devem ser fingidos em D1

- OAuth, identidade validada, renovação, revogação remota, entitlement e limites reais: M0 orienta o contrato; produto e pooling são habilitados conforme M2/M3. O protótipo apenas representa esses estados como simulação.
- Execução/cancelamento terminal, reconciliação, persistência, entrega do pacote, checks e Git: OX-003 a OX-014 conservam evidência própria. Uma cor/etiqueta correta não comprova enforcement.
- Avaliação com três a cinco desenvolvedores externos, rotina real e registro de tempo/intervenções: antes de fechar OX-017. O piloto do responsável libera a decisão inicial D1 → M0, quando seu gate é satisfeito; não conclui alpha ou prova produtividade.
- Edição manual leve: P1 para decisão após P0-06/09. Se entrar no alpha, exigir proveniência de escrita e invalidação de evidências. IDE completo, debugger/test explorer e extensão VS Code não são pré-requisitos de D1.
- Planner autônomo e replanejamento: M4. Objetivo/dependências simulados avaliam linguagem e sequência sem prometer essa execução no primeiro alpha.
- Suspensão mínima real de admissões pelo daemon: M2; políticas/escopos ampliados do scheduler: M3. Este estudo distingue bloqueio da conexão e interrupção da tentativa, sem demonstrar um controle global implementado.

**Gate permanece aberto:** aplicar o checklist de [d1-pilot.md](d1-pilot.md#checklist-de-passagem-d1--m0) com resultados reais. Lacuna de interação P0 é registrada em D1; falta de prova técnica conserva o destino posterior da tabela. Nenhuma das duas desaparece por ter sido mapeada.

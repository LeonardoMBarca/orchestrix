# Modos de ação e perfis dos agentes executores

**Direção de produto definida pelo responsável em 09/10/2026.** Vários modos nativos, criação de modos especializados e configuração dos agentes executores fazem parte do escopo desejado da versão inicial. Este documento registra o requisito e sua proposta de integração; não declara essas funcionalidades implementadas. O site apresenta a direção do produto, enquanto o Core e os adapters ainda precisam demonstrar execução real.

O desenvolvimento de software continua sendo o público principal. Os mesmos contratos de contexto, execução, artefatos, revisão e controle podem coordenar trabalhos científicos, documentação e apresentações. A experiência começa pelo chat, com um modo útil por padrão; personalização detalhada permanece disponível sob demanda.

## Dois conceitos com responsabilidades distintas

| Conceito | O que configura | Exemplo |
| --- | --- | --- |
| **Action Mode** | Objetivo do trabalho, estratégia de coordenação, papéis necessários, resultados esperados e critérios de revisão. Orienta o orquestrador. | `Development` coordena implementação, verificação e revisão; `Research` coordena investigação e análise de fontes. |
| **Worker Profile** | Comportamento de um agente que recebe trabalho: especialidade, instruções, padrões de escrita/código, preferências e limites do papel. Corresponde à especialização do `AgentProfile` já previsto no domínio. | `Frontend Engineer`, `Research Analyst`, `Documentation Writer`, `Code Reviewer`. |

Um modo pode usar vários perfis; um perfil pode ser reutilizado em vários modos. Perfil, conta, runtime e modelo têm identidades distintas. Editar um implementador não redefine o orquestrador nem troca a identidade da conexão que executa uma tentativa existente.

Exemplo: a pessoa mantém `Development`, mas configura o implementador para seguir seus padrões de TypeScript, preferir alterações pequenas e explicar decisões em português. O modo conserva sua finalidade, verificação e revisão. Outra pessoa pode criar um modo próprio que coordene perfis existentes com uma sequência e resultados específicos.

A **Routing Strategy** define como distribuir recursos entre modelos e esforço de raciocínio: `Efficiency`, `Performance`, `Balanced` ou `Custom`. Não substitui o modo nem o perfil. Os [presets especializados e regras de escolha](./routing-strategies.md) seguem a política existente: um modo `Research`, por exemplo, pode usar o preset `Scientific Research` e perfis de análise, com ferramentas e capacidades elegíveis.

**Esclarecimento de 13/10:** a conversa principal é com um agente orquestrador selecionável, dentro das políticas do Core. Escolher esse agente não o confunde com Action Mode ou Worker Profile, nem altera identidade/cobrança das conexões. Modos (finalidade), perfis (comportamento/especialidade), estratégia (prioridade de recursos), presets especializados e preferências manuais são combináveis; um padrão útil não obriga configurar tudo antes de começar. Capacidades atuais/validades seguem os [diagnósticos](connection-diagnostics.md#atualização-periódica-e-orientada-a-eventos--requisito-planejado), sem mudar o snapshot de trabalho ativo.

## Catálogo nativo da versão inicial

Os nomes abaixo são propostos em inglês; descrições e conteúdos podem seguir o idioma da interface e a preferência do usuário.

| Modo | Uso principal | Resultado esperado e revisão |
| --- | --- | --- |
| **Development** | Implementar funcionalidades e corrigir código a partir da conversa e do contexto autorizado do projeto. | Alterações identificadas, critérios de aceite, verificações disponíveis e revisão do resultado antes da aplicação conforme a política. |
| **Research** | Apoiar investigação científica, localizar papers relevantes e analisar materiais já presentes no projeto. | Síntese ou rascunho com fontes rastreáveis; separação entre evidência, hipótese e interpretação; referências e afirmações verificáveis. Busca externa depende das ferramentas e permissões disponíveis. |
| **Documentation** | Criar e atualizar documentação com padrões consistentes para o projeto. | Documentos alinhados ao conteúdo observado, à estrutura e ao estilo escolhidos; revisão de referências e divergências. |
| **Presentations** | Preparar apresentações a partir de documentos, conteúdo indicado ou da conversa; útil também para estudantes e apresentações de projetos. | Estrutura, narrativa e artefatos nos formatos escolhidos, como PPTX ou PDF, quando houver ferramentas compatíveis; revisão do conteúdo e do arquivo gerado. |
| **Refactoring** | Reestruturar código com um objetivo e escopo claros, preservando o comportamento acordado. | Alterações com evidência de preservação dos requisitos; verificações adequadas e revisão das diferenças. Mudanças de comportamento precisam ser identificadas. |
| **Improvement Review** | Investigar código, projeto ou documentos e propor melhorias específicas. | Propostas com motivo, evidência, impacto e um caminho de implementação. A análise não aplica alterações por si; o usuário pode encaminhar uma proposta para um modo de execução. |

Um modo não exige famílias de modelo diferentes. Uma conta e um runtime compatível podem executar papéis em sessões independentes e em sequência, conforme o [ADR-0001](../adr/0001-single-runtime-first-class.md). Paralelismo é opcional; concorrência 1 precisa ser uma configuração útil. Seleção de modelo e raciocínio pode variar dentro da mesma conexão quando esses controles forem oficialmente expostos. Formatos, busca na web e outras ferramentas não são capacidades universais: elegibilidade precisa ser demonstrada por conexão, runtime e versão.

## Personalização sem perder o contrato

**Modos nativos:** oferecer opções declaradas de personalização, como estilo dos resultados, profundidade da análise, padrões do projeto, perfis preferidos e preferências de revisão. Essas opções conservam a finalidade e os requisitos obrigatórios de cada modo. A definição nativa permanece identificável e pode ser restaurada.

**Modos especializados:** permitir criar uma definição local com nome, finalidade, instruções, papéis, resultados esperados e critérios de aceite. Validar a definição antes de usá-la; um texto livre não concede novas ferramentas, acesso ou autoridade. Pode ser criada a partir de um modo nativo sem substituir seu contrato original.

**Perfis dos executores:** permitir configurar instruções adicionais, linguagem, convenções, especialidade e preferências de modelo/raciocínio/contexto. Campos autorizados pelo papel podem ajustar permissões dentro dos limites da política; uma instrução no perfil não amplia acesso por conta própria. Exibir os valores efetivos quando houver conflitos ou limitações do runtime, conforme o [ADR-0002](../adr/0002-model-and-reasoning-policy.md).

## Regras de execução e histórico

1. **Preferências ficam subordinadas aos requisitos obrigatórios.** Modo ou perfil não desativa restrições de conta, cobrança, ferramentas, paths, isolamento, orçamento, verificações ou aprovação. Um perfil revisor definido como leitura não passa a editar arquivos porque suas instruções pedem isso.
2. **Resolver antes de executar.** Combinar modo, perfil e política pela resolução existente, mantendo precedência explícita. Os padrões do modo entram em `mode_defaults`; os padrões do perfil em `profile_defaults`, antes dos overrides de Profile. Requisitos acumulam-se. A sequência permanece System → User → Project → Profile → CategoryRisk → Execution → Task.
3. **Configuração versionada e inspecionável.** Cada tentativa referencia a versão do modo, do perfil, o contexto enviado, a política efetiva, a conexão, a base e os artefatos. Edições alteram novas decisões; snapshots anteriores continuam preservados.
4. **Mudança de modo preserva o trabalho.** Trocar de `Improvement Review` para `Development`, por exemplo, encaminha propostas como contexto identificado para novo trabalho. Não aplica resultados anteriores nem concede aprovação. Cancelar, substituir ou criar nova tentativa segue os comandos explícitos do Core.
5. **Resultado depende de evidência.** Código continua sujeito às verificações e integração existentes. Documentos, análises e apresentações têm critérios adequados ao artefato: fontes, formato, conteúdo e revisão. Relatos dos agentes não comprovam testes, referências ou geração de arquivos.
6. **Capacidade ausente produz diagnóstico.** Ferramenta ou formato obrigatório sem suporte deixa o trabalho bloqueado ou pede alternativa explícita, conforme a política. O catálogo não transforma suporte desconhecido em capacidade disponível.
7. **Contexto autorizado e proveniência continuam valendo.** Material do projeto e fontes externas são entradas identificadas. O modo não acessa automaticamente todos os arquivos nem altera a propriedade das sessões nativas descrita no [ADR-0004](../adr/0004-context-ownership.md).
8. **Modo e estratégia têm objetivos distintos.** A estratégia pode ajustar recursos por tipo, complexidade, risco e importância, sem apagar o contrato do modo. Consulta de status Git e outras verificações mecânicas continuam usando ferramentas determinísticas; modelo é reservado ao julgamento pertinente.

Esses recursos continuam sendo configuração da orquestração. Não exigem construir uma IDE, editor de apresentações ou ambiente de testes interativos dentro do Orchestrix, em acordo com o [ADR-0003](../adr/0003-desktop-app-editor-agnostic-core.md).

## Edição local implementada — OX-016

`configuration.rs` usa registros imutáveis `Configuration` no Store existente para definições ricas e políticas User/Project. Mantém as referências mínimas Mode/Profile do domínio. Identidade, versão e revisão de edição são distintas; salvar, restaurar ou arquivar usa CAS, recibo idempotente e evento da mesma transação. Não há novo banco, localStorage autoritativo ou migração de conteúdos antigos.

Os seis modos nativos e os dois perfis de papel começam na versão **2**. A versão 1 anteriormente usada pelo worker permanece legada, com o conteúdo mínimo efetivamente registrado; não recebe retrospectivamente instruções, nome ou capacidades da versão 2. Uma derivação local fixa uma base nativa exata, sem cadeias arbitrárias. Campos e limites são validados por tipo/papel. O revisor mantém leitura, rede desabilitada e sessão independente, mesmo se renomeado ou suas instruções pedirem escrita.

O editor permite personalizar instruções, papéis válidos, saídas/critérios e preferências permitidas; finalidade, origem e requisitos nativos ficam protegidos. Perfis permitem instruções, idioma de resultado, convenções e especialidade. Políticas User e Project identificado usam o mesmo resolver. Restaurar herança remove preferências e conserva restrições obrigatórias. O idioma da interface não altera instruções salvas.

A prévia resolve os valores e suas origens sem consultar o provedor. Observações persistidas conservam datas, escopo e validade; ausência ou vencimento não comprova elegibilidade. Na preparação, o conjunto é fixado e alterações relevantes exigem nova preparação. Instruções/versionamento chegam ao Context Pack; a reserva transacional confere o fingerprint novamente, fechando a janela entre consulta externa e admissão. Tentativas admitidas conservam seus snapshots. Revisão usa o modo original e seu perfil revisor fixado; correção resolve novas contribuições sem duplicar as antigas como instruções do usuário.

Development, Documentation e Refactoring reutilizam o worker manual compatível e seus checks/revisão; isso não certifica todos os artefatos desses domínios. Research, Presentations e Improvement Review podem ser configurados, mas a execução pelo worker de código é bloqueada com `artifact-flow-unproven`. Seus fluxos representativos continuam no aceite alpha de OX-017, sem adiamento para M4. As evidências executadas e limitações atuais ficam no [status](../DEVELOPMENT_STATUS.md).

## Integração ao plano existente

Esta ampliação mantém a ordem **D0 → D1 → M0 → M1 → M2** e os gates atuais. A versão inicial pretendida deve entregar os seis modos e a configuração de perfis; a implementação pode começar pelo ciclo de código como incremento técnico, sem considerar os outros modos concluídos por estarem descritos no site.

| Etapa existente | Incremento necessário |
| --- | --- |
| **D1 / OX-D03–OX-D05** | Validar seleção simples de modo no chat, descoberta das configurações avançadas e distinção entre modo e agente executor. Evitar exigir ajustes antes da primeira conversa. |
| **M0 / OX-001–OX-003** | Registrar ferramentas, tipos de artefato e limitações relevantes nas capacidades observadas dos adapters; não supor busca científica ou exportação PPTX/PDF por existir um runtime de código. |
| **M1 / OX-004–OX-005 e OX-008** | Acrescentar definições e versões de modos/perfis, persistência e resolução de requisitos/padrões nos snapshots. Reutilizar `AgentProfile`, política e Context Pack. |
| **M2 / OX-012, OX-015 e OX-016** | Oferecer catálogo nativo, seleção por conversa/trabalho, criação de modos especializados e edição dos campos autorizados dos perfis. Mostrar configuração efetiva e restauração dos padrões. |
| **M1/M2 / OX-009, OX-013–OX-014 e OX-017** | Ampliar a demonstração e o aceite aos artefatos dos seis modos. Reutilizar isolamento, checks, revisão, correção e aplicação conforme o tipo de trabalho, com evidências reais. |
| **M4** | Ampliar planejamento e roteamento automáticos sobre os mesmos modos e perfis; não adiar a seleção e personalização básica para esse marco. |

Antes de anunciar a entrega da versão inicial, demonstrar ao menos um fluxo representativo por modo, incluindo fontes verificáveis em `Research`, arquivo exportado e revisado em `Presentations` e propostas sem aplicação automática em `Improvement Review`. Demonstrar também que editar um perfil não altera uma tentativa em andamento, que requisitos incompatíveis bloqueiam execução e que um modo especializado reutiliza os controles existentes.

O catálogo e os perfis são locais neste escopo. Publicação pública, compartilhamento hospedado e infraestrutura para distribuição de modos não fazem parte deste incremento.

## Referências do projeto

- [Visão do produto: domínio e perfis](../PROJECT_VISION.md).
- [Arquitetura: adapters, routing, verificação e contexto](../ARCHITECTURE.md).
- [Plano: política, contratos e marcos](../DEVELOPMENT_PLAN.md).
- [Backlog: domínio, políticas, revisão e Control Center](../DEVELOPMENT_BACKLOG.md).

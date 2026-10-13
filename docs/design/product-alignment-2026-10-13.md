# Alinhamento de produto — 13/10/2026

Definições explícitas do responsável pelo produto, consolidadas sobre `f898939`, branch `docs/development-plan`. Esta entrega é documental: mantém os milestones, o gate D1 liberado, o primeiro app-server local aprovado e OX-003 em andamento. Não implementa recursos nem reabre pesquisa/design.

## O que se mantém e o que muda

| Classificação | Definição | Especificação vigente e tickets |
| --- | --- | --- |
| Já previsto | Desktop open source + Core independente do editor; Windows, macOS e Linux como distribuição pretendida, com entrega progressiva. | [Visão](../PROJECT_VISION.md), [stack e aceite por sistema](../TECH_STACK.md#platform-boundaries-and-progressive-delivery); OX-003/OX-007/OX-012/OX-017, M5. |
| Já previsto | Chat, sessões/projetos, uma conta/runtime e execução serial úteis; APIs explícitas e assinaturas por mecanismos oficiais validados. | [Plano](../DEVELOPMENT_PLAN.md#1-objetivo-de-produto), ADR-0001, contratos de conexão; OX-001–OX-004/OX-012. |
| Esclarecido | A conversa principal é com um agente orquestrador selecionável, que coordena sob as políticas do Core; não exige copiar instruções entre workers ou configurar todos os parâmetros. | [Workspace](../DESKTOP_APP_AND_PROJECT_WORKSPACE.md#1-decision-summary); OX-008/OX-012/OX-016 e planner no marco vigente. A seleção do protótipo não comprova esse motor. |
| Esclarecido | O produto não é uma IDE, nem presume evoluir para uma. Editor auxiliar é requisito de apoio; execução interna de agentes/Git/checks permanece no Core. | [ADR-0005](../adr/0005-desktop-product-scope-and-platform-boundaries.md), [editor e aceite](../DESKTOP_APP_AND_PROJECT_WORKSPACE.md#7-supporting-editor-and-diff-viewer); OX-012/OX-015, com versões/aprovação em OX-013/OX-014. |
| Esclarecido | OS específico fica na infraestrutura/host/adapter. Windows primeiro não cria dependência Windows no domínio/contrato; nenhum dos três sistemas recebe suporte por associação. | ADR-0005, [matriz de plataformas](../TECH_STACK.md#platform-boundaries-and-progressive-delivery); OX-003/OX-007/OX-017. |
| Já previsto e reforçado | Action Mode, Worker Profile, Routing Strategy e presets são distintos e combináveis. Regras determinísticas, override, evidência e Subscription Only permanecem. | [Modos/perfis](action-modes-and-worker-profiles.md), [roteamento](routing-strategies.md); OX-004/OX-008/OX-016. |
| **Requisito novo** | Atualização automática periódica e orientada a eventos, além do cadastro/Recheck manual, com validade, budgets/backoff e consulta direcionada antes de executar. | [Política de atualização](connection-diagnostics.md#atualização-periódica-e-orientada-a-eventos--requisito-planejado); OX-003/OX-005/OX-007/OX-008/OX-012/OX-016, sem ticket/marco paralelo. |

A implementação atual prova somente os recortes registrados no [status](../DEVELOPMENT_STATUS.md). Multiplataforma de produção, editor Desktop, orquestrador conversacional de produção e atualização periódica continuam planejados. O cadastro/Recheck do estudo e a descoberta paginada experimental não implementam o serviço de atualização.

## Fronteiras preservadas

Provider, conta/conexão, runtime, modelo e perfil permanecem entidades distintas. Gemini API e Antigravity não compartilham automaticamente autenticação ou capacidades; login em um aplicativo não autoriza outra superfície de integração. Catálogo, suporte, disponibilidade, configuração e execução são evidências diferentes. APIs escolhidas e autorizadas explicitamente continuam possíveis fora de Subscription Only; este modo exclui execução API e fallback pago.

O [ADR-0003](../adr/0003-desktop-app-editor-agnostic-core.md) permanece como registro histórico e base de Desktop + Core. ADR-0005 substitui somente suas cláusulas de escopo da IDE/editor e esclarece as fronteiras de plataforma; a hipótese histórica de uma IDE própria deixou de ser direção vigente. ADR-0001/0002/0004 continuam aceitos.

A atualização de capacidades altera elegibilidade **para novas decisões** e preserva snapshots anteriores. Requisito crítico vencido exige revalidação/bloqueio, nunca inferência ou probe cobrado automático. A especificação única desses controles fica em diagnósticos; o router e Settings apenas a consomem.

## Continuidade técnica

O próximo incremento continua sendo consumo/conformidade do contrato 1.1 no Core Rust, reaproveitando [contrato, fixtures e resultados](../research/codex-contract-integration.md). Cargo, rustc e rustup foram procurados no PATH e em `%USERPROFILE%\.cargo\bin` nesta rodada e não foram encontrados. Não houve instalação, novo live ou início de OX-004/SQLite/scheduler. A disponibilidade da toolchain é a preparação necessária para compilar e validar o consumidor Rust; depende da autorização de instalação ou de um caminho de instalação existente fornecido pelo responsável.

`vswhere` encontrou Visual Studio Build Tools completo com o componente C++ x64/x86; a leitura de arquivos encontrou linker x64 MSVC **14.50.35717** e bibliotecas x64 Windows SDK **10.0.26100.0**. Não se presumiu que `cl.exe` precisasse estar no PATH da sessão comum. A preparação proposta é Rust stable x64/MSVC via [rustup oficial](https://rust-lang.org/tools/install/), para o usuário atual, sem instalar outros componentes automaticamente. A disponibilidade efetiva do linker/SDK será validada pela compilação, conforme os [pré-requisitos oficiais](https://rust-lang.github.io/rustup/installation/windows-msvc.html); arquivos/componentes instalados não equivalem a build Rust aprovado.

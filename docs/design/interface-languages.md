# Idiomas da interface

Decisão do responsável em **10 de outubro de 2026**: o site público do Orchestrix será em inglês. O aplicativo oferece **English**, **Português** e **Español**, com **English como padrão**. Isso complementa a identidade visual e a entrada pela conversa; não altera os milestones ou gates de D1/M0.

## Comportamento do produto

- O idioma da interface é uma preferência global da pessoa, independente do projeto, da conversa, do tema e do runtime. Uma instalação nova começa em inglês, inclusive quando o sistema/navegador usa outro idioma.
- A pessoa seleciona o idioma em **Settings → General → Interface language**; aparência fica em **Settings → Themes**. A janela é flutuante e fechável, disponível pelo ícone de engrenagem. Options e o seletor na navegação foram substituídos na revisão do shell. A mudança é imediata; não reinicia o projeto, dispara trabalho ou altera políticas. A posição da janela é estado de apresentação, separado do idioma persistido.
- Nomes fornecidos pela pessoa, mensagens, rascunhos, código/diffs, caminhos, contas, IDs e identificadores de runtime permanecem como foram escritos. Mudar a interface não traduz o histórico da conversa nem reescreve resultados.
- O idioma usado para criar conteúdo de exemplo ou novas respostas da demonstração pode acompanhar a preferência no momento da criação. Conteúdo já criado conserva esse idioma. Na implementação dos runtimes, preferência da interface e instruções de idioma para os agentes serão conceitos separados.
- A preferência persiste. Valor salvo desconhecido retorna ao inglês; armazenamento indisponível permite usar a seleção durante a sessão.
- Site, página do vídeo, controles, anúncios acessíveis e metadados públicos ficam em inglês, independentemente da preferência do app. Nomes de marca, temas e produtos são preservados.

## Implementação do estudo D1

[i18n.js](../../prototypes/desktop/i18n.js) contém a resolução de idioma e os helpers de texto da interface. [Os catálogos](../../prototypes/desktop/locales/README.md) registram os textos em inglês e espanhol; o texto fonte em português atende à opção `pt-BR`. Textos de interface usam lookup explícito; templates traduzem somente suas partes estáticas e atributos acessíveis. Valores interpolados são preservados. Não há tradução automática do DOM ou serviço externo de tradução.

Os catálogos são separados entre navegação, app, conversa/workspace e conexões. `npm.cmd run locales` compila o catálogo servido no navegador; `npm.cmd run check` detecta traduções incompletas, conflitantes ou catálogo desatualizado e verifica a sintaxe. O carregamento é local e síncrono antes dos renderizadores, sem depender de rede externa.

O estudo salva `en`, `pt-BR` ou `es` em `orchestrix-prototype-language` no armazenamento do navegador. O Desktop de produção deverá levar essa mesma preferência ao armazenamento de configurações da pessoa e preservar a separação entre interface, conteúdo e contratos dos agentes.

## Validação

O [teste de idiomas](../../prototypes/desktop/tests/localization.spec.mjs) contém 15 casos que cobrem o padrão inglês, seleção/persistência, armazenamento indisponível, conteúdo da pessoa, formulários ainda não salvos, cursor/rascunhos, código, navegação por teclado, nomes acessíveis e reflow. As jornadas técnicas anteriores continuam exercitadas explicitamente em português; os testes do site verificam o conteúdo público em inglês.

**Revisão histórica de idiomas em 10/10:** 73 casos validados por cobertura consolidada — 65 na execução completa e oito em reteste após ajustes de seletores, expectativas de apresentação e relógio dos testes — com 1.010 textos no catálogo e check aprovado. Nove capturas apresentaram idioma correto, sem overflow horizontal, erros, falhas de carregamento ou chaves faltantes.

**Refinamento mobile posterior:** catálogo compilado de **1.012 chaves** e check aprovado; os 15 casos de idiomas e 12 de entrada passaram juntos, **27/27**, incluindo primeira dobra EN/PT-BR/ES e preservação de campo/cursor/foco ao redimensionar. A suíte completa final passou **77/77 em uma única rodada de 2,2 minutos**, sem falhas ou casos ignorados. As duas tentativas anteriores e a correção de foco são preservadas no registro progressivo. O check direto de HTML passou desktop e seis composições mobile: campo inteiro na primeira dobra em tamanho normal; texto a 200% reflui com rolagem natural e sem overflow horizontal. O botão Send pode exigir rolagem. Resultados e capturas estão no [registro progressivo](d1-progressive-experience.md#refinamento-da-entrada-mobile--1010), no [status](../DEVELOPMENT_STATUS.md) e no [catálogo](visual-asset-catalog.md). Essas verificações não encerram os gates de D1/M0.

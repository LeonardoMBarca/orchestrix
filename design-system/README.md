# Orchestrix — referência de estilo

**Base aprovada pelo responsável em 10 de outubro de 2026.** Esta pasta reúne a direção visual que seguirá no site e no aplicativo. Ela orienta novas telas e a implementação do Desktop, com referências aos tokens, componentes e imagens existentes.

O Orchestrix usa azul profundo, índigo, superfícies em camadas, tipografia legível e a logo transparente da Direção 01. O site apresenta a atmosfera de um voo noturno sobre montanhas rochosas. O aplicativo Studio compartilha a identidade, com uma superfície calma para conversar, ler código e acompanhar trabalho.

## Comece por aqui

| Referência | Quando consultar |
| --- | --- |
| [Decisões aprovadas](decisions.md) | Entender a direção, sua aprovação e as diferenças intencionais entre site e app. |
| [Marca](brand/README.md) | Escolher a logo, favicon e aplicação adequada à superfície. |
| [Tokens](tokens/README.md) | Reutilizar cores, tipografia, espaçamento, bordas e densidade. |
| [Componentes](components/README.md) | Criar ou alterar botões, campos, cartões, conversa, painéis e diálogos. |
| [Movimento](motion/README.md) | Preservar a ambientação do site e o conforto de uso do app. |
| [Padrões de experiência](patterns/README.md) | Organizar apresentação, entrada pelo chat e controles progressivos. |
| [Sessões, painéis e Settings](workspace-shell.md) | Seguir a entrada simples, projetos/sessões em lista, docking com abas e configurações flutuantes. |
| [Imagens e procedência](assets/README.md) | Encontrar um ativo por descrição e registrar novas variantes. |
| [Exemplos e verificação](examples/README.md) | Comparar a implementação com as telas e evidências existentes. |

## Fontes que mantêm a referência consistente

- [identity.css](../prototypes/desktop/identity.css) é a fonte atual dos valores de marca compartilhados e já é consumida pelo site e pelo Studio.
- [styles.css](../prototypes/desktop/styles.css) define temas, aliases semânticos, componentes técnicos e reflow. [experience.css](../prototypes/desktop/experience.css) aplica a identidade ao app; [settings.css](../prototypes/desktop/settings.css) define a janela de configuração; [website.css](../prototypes/desktop/website.css) aplica a apresentação pública.
- O [catálogo textual de imagens](../docs/design/visual-asset-catalog.md) é o índice único de ativos, candidatos e capturas. Esta pasta aponta para ele e para os arquivos originais; não cria outra coleção de binários.
- O [contrato de idiomas](../docs/design/interface-languages.md) define site em inglês e app em English, Português e Español, com inglês como padrão.

Os valores das tabelas são referências da implementação atual. Ao alterar um token, editar sua fonte de código e atualizar os guias afetados no mesmo incremento. Evitar uma segunda paleta manual em cada tela. Na migração para React/TypeScript/Tauri, preservar esses papéis e contratos; esta pasta não escolhe outra stack nem fornece uma biblioteca de componentes de produção.

## Como desenvolver seguindo esta base

1. Localizar o padrão de experiência e o componente existente que atendem à tela.
2. Reutilizar seus tokens semânticos e o ativo indicado no catálogo. Adaptar a organização ao objetivo da tela, ao tema e à largura disponível.
3. Escrever texto de interface pelos catálogos de idiomas. Manter mensagens, código, caminhos e nomes fornecidos pela pessoa como foram escritos.
4. Conferir os estados relevantes, teclado, reflow, texto ampliado e movimento reduzido. Comparar as superfícies afetadas com os exemplos.
5. Quando a direção evoluir, registrar o motivo em [decisions.md](decisions.md), atualizar a fonte dos valores, os guias e o catálogo correspondente. Uma nova imagem disponível não se torna automaticamente a marca do produto.

## Situação no desenvolvimento

A direção estética está aprovada e é a base dos próximos incrementos. A [revisão de sessões e configurações](../docs/design/d1-shell-revision.md) implementa a organização solicitada, cuja aprovação de uso ainda está pendente. **O piloto D1 foi adiado pelo responsável até que a interface esteja de acordo com o que deseja.** Gostar do visual não equivale a concluir as tarefas de uso. Persistência durável, execução de runtimes, instaladores e publicação seguem o [plano](../docs/DEVELOPMENT_PLAN.md), o [backlog](../docs/DEVELOPMENT_BACKLOG.md) e os gates existentes. O [status](../docs/DEVELOPMENT_STATUS.md) separa o que foi implementado, verificado e planejado.

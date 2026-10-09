# Orchestrix — protótipo D1 do Desktop

Incremento de OX-D03/OX-D04 preparado para o piloto de OX-D05, em **8 de outubro de 2026**, fundamentada na [pesquisa de experiência](../../docs/research/experience-decisions.md) e nos [percursos dos similares](../../docs/research/solution-journeys.md).

Este protótipo usa HTML/CSS/JavaScript sem dependências de execução. Permite avaliar a estrutura e os fluxos antes de implementar o Desktop Tauri/React. Não determina a stack do aplicativo final. [Entrega e verificações D1](../../docs/design/d1-delivery.md) · [sistema visual](../../docs/design/d1-design-system.md) · [roteiro do piloto](../../docs/design/d1-pilot.md).

## Abrir

Abra [index.html](./index.html) no navegador, ou execute a partir desta pasta:

```powershell
npm.cmd start
```

Acesse [http://127.0.0.1:4173](http://127.0.0.1:4173). O servidor disponibiliza somente os arquivos estáticos do estudo, em loopback. Não precisa instalar pacotes para usar o protótipo. Para outra porta, configure `ORCHESTRIX_PROTOTYPE_PORT`.

## Temas disponíveis

| Tema | Proposta | Captura |
| --- | --- | --- |
| Studio — padrão | Grafite, acento violeta, tipografia de interface e superfícies discretas | [Workspace](../../docs/research/previews/studio.png) |
| Atelier | Tons claros de papel, acento terracota, títulos editoriais e cantos mais contidos | [Workspace](../../docs/research/previews/atelier.png) |
| Horizon | Azul profundo, acento azul claro, mais espaço e superfícies arredondadas | [Workspace](../../docs/research/previews/horizon.png) |
| Deep Black | Preto profundo, acento neutro e contraste contido | [Workspace](../../docs/research/previews/deep-black.png) |
| Medieval | Madeira escura, ouro antigo, títulos clássicos e bordas contidas | [Workspace](../../docs/research/previews/medieval.png) |
| Forest | Verde profundo e tons de menta | [Workspace](../../docs/research/previews/forest.png) |
| Dawn | Luz quente, coral e superfícies suaves | [Workspace](../../docs/research/previews/dawn.png) |

Os sete temas usam os mesmos dados e controles, permitindo comparar organização e leitura. Studio foi definido como padrão pelo responsável, que pediu alternativas com nomes em inglês. O botão **Personalizar aparência →** na navegação abre a seleção de temas em **Preferências → Aparência**; **Restaurar Studio** retorna ao padrão. São explorações de tokens e tratamento visual, ainda sujeitas a refinamento. Tema, densidade, ampliação de texto e largura da fila persistem no armazenamento do navegador. Recolhimento e foco são de sessão. Trabalho, conexões e preferências da simulação reiniciam ao recarregar.

[Tela de revisão](../../docs/research/previews/review.png) · [Janela compacta de revisão](../../docs/research/previews/compact.png) · [Galeria em Aparência](../../docs/research/previews/appearance.png)

## Percursos disponíveis

O [roteiro D1](../../docs/design/d1-pilot.md) contém os nove casos de avaliação, variações e registro de ajuda/dúvidas/erros. Nenhum piloto humano foi realizado nesta entrega.

- **Primeira entrada:** botão do projeto → Começar. Prepare Correção curta ou Feature guiada, informe caminho de exemplo e Windows/WSL, escolha uma conexão e confirme a modalidade na autorização simulada.
- **Correção curta:** inspecione contexto, inicie e conclua trabalho, examine diff/evidências e peça correção. Valide a tarefa no Run; depois revise e aplique o candidato no destino.
- **Feature:** OX-24/25/26/27 pertencem a R-08. OX-26 exige decisão; OX-27 espera OX-24/OX-25 aceitas. Resultados complementares têm revisão própria. O candidato final contém cinco arquivos; uma tarefa independente não entra nesse candidato.
- **Atenção:** use o seed inicial para check falho, pergunta, falha terminal, perda de sinal, permissão e cancelamento sem confirmação. `Ctrl+Shift+E` registra evidência de outra tarefa sem tirar o foco do campo/diff; abra Precisa de você.
- **Conexões:** adicione registro pendente, confirme modalidade/identidade e compare catálogo, descoberta, limite, expiração, acesso negado e reconexão. Desconectar não confirma término de worker ou revogação.
- **Preferências:** escolha conta, modelo de exemplo, raciocínio e contexto para novas tentativas. Pacotes e configurações anteriores continuam preservados.
- **Aparência:** Personalizar aparência → galeria, densidade e texto ampliado. Redimensione a fila com arraste ou setas/Home/End; compare recolhimento, modo foco e janela compacta.

`Ctrl+K` abre ações; `N` cria tarefa fora de campos; `?` abre ajuda; `Tab` e setas navegam controles/abas; `Esc` fecha diálogos. A simulação reinicia por Reiniciar demonstração ou reload; Restaurar layout retorna as preferências de layout ao padrão.

## Dados e limites do cenário

Todos os agentes, contas, modelos, estados, arquivos e evidências são simulados. Não há login, credenciais, chamada de inferência, leitura de pastas ou operação Git. O exemplo de código não pertence à implementação do Orchestrix e não é código de autenticação pronto para uso.

No seed inicial, R-08 contém somente OX-24 e as demais tarefas pertencem a Runs independentes. O template Feature guiada usa quatro tarefas no mesmo R-08, com revisão de resultados complementares e candidato de cinco arquivos. A simulação distingue tarefa validada na branch interna e Run aplicado no destino; a agregação Git real e integração serial pertencem ao Core futuro.

Os controles de pausa, retomada e reconciliação mostram intenções de interface. O produto final só oferecerá operações comprovadas por cada adapter. Revalidar a base versiona o candidato de integração e conserva a tentativa do agente; pedir correção cria uma tentativa nova. Durante uma correção, o artefato anterior conserva a tentativa que o produziu.

O estudo tem navegação, leitura de diff e correção assistida. Contexto/arquivos, entrada, estados de autorização/limite, snapshots e painéis ajustáveis estão representados. Comentários por linha, comparação aprofundada entre tentativas, políticas avançadas ainda serão ampliados. Tarefas manuais têm resultado ilustrativo, correção, aceite e revisão/aplicação em Runs próprios. Edição manual leve será priorizada após o piloto.

## Verificar

```powershell
npm.cmd ci
npm.cmd run check
npm.cmd test
```

Os 24 cenários automatizados (dez D1, nove anteriores e cinco de acessibilidade/regressão) passaram em 09/10/2026 usando Node 24.19.0 e Chrome 154.0.8037.98 headless no Windows. O runner de teste é Playwright 1.64.0, fixado no lockfile. Caso o canal Chrome não esteja instalado, instale-o ou ajuste `playwright.config.mjs` para o navegador de teste disponível.

Os testes cobrem transições, identidade/proveniência do artefato, confirmação vencida, preferências, persistência/restauração de tema, conteúdo de usuário, teclado, reflow e quatro pares principais de contraste de texto em cada um dos sete temas. Foram usados viewports de 1920×1080, 1280×800, 1024×768, 720×480, 390×844 e 320×720 pixels CSS. Capturas próprias foram renderizadas e examinadas; os arquivos gerados ficam em `artifacts/`, ignorado pelo Git.

Os cinco [testes adicionais](tests/d1-accessibility.spec.mjs) verificam destino de foco após preparar projeto/adicionar conexão, movimento reduzido e cores forçadas emulados pelo Chrome e textos/paths Unicode nos limites dos campos a 320px. A correção do foco mantém um controle válido ou o conteúdo atual, sem mover a rolagem.

Isso não certifica acessibilidade completa, escala física do Windows ou conforto de uso. Escala 125/150/200%, zoom físico do navegador, configuração de contraste do Windows, tecnologias assistivas, logs/diffs extensos e avaliação com desenvolvedores permanecem pendentes. O incremento D1 verificou texto ampliado a 200%, arraste/teclado e persistência de layout; isso é distinto da escala/zoom físico. Os testes do protótipo também não são testes dos futuros adapters/Core.

Referência para a configuração do runner e canal do navegador: [Playwright configuration](https://playwright.dev/docs/test-configuration) e [browsers](https://playwright.dev/docs/browsers#google-chrome--microsoft-edge).

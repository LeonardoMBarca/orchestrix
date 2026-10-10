# Movimento e ambientação

Referência aprovada em **10 de outubro de 2026**. A direção do site é um **voo noturno sobre montanhas rochosas**, com luz índigo e estrelas que respondem suavemente à pessoa. O aplicativo usa a mesma identidade com uma superfície tranquila para trabalhar. Montanhas e partículas ficam no site.

## Efeitos implementados no site

| Camada | Comportamento atual | Fonte viva |
| --- | --- | --- |
| Montanhas | Imagem fixa cobrindo a viewport, com transparência e overlays navy para preservar a leitura. A sensação de voo vem da composição e das estrelas; não há movimento de câmera ou parallax da montanha. | `.night-landscape` em [website.css](../../prototypes/desktop/website.css), [website.html](../../prototypes/desktop/website.html) |
| Estrelas em repouso | Drift lento, leve oscilação e variação discreta de brilho em canvas. Os pontos atravessam os limites e reaparecem no lado oposto. | `drawField`, `advanceParticles` em [website.js](../../prototypes/desktop/website.js) |
| Scroll | O deslocamento da página adiciona um impulso limitado à velocidade das estrelas. O impulso decai gradualmente quando a pessoa para. O listener é passivo; o efeito não controla a rolagem da página. | `scroll` e `frame` em [website.js](../../prototypes/desktop/website.js) |
| Ponteiro | Glow índigo, brilho/tamanho local dos pontos, leve afastamento e deslocamento conforme profundidade. Posição e intensidade são suavizadas; os pontos mantêm velocidade e retornam com amortecimento, evitando saltos e travamento. | `pointermove`, `drawField`, `advanceParticles` em [website.js](../../prototypes/desktop/website.js) |
| Coordenação no hero | Blocos oscilam verticalmente. Curvas e bolinhas são recalculadas até a lateral vertical de cada bloco, acompanhando sua posição e mudanças de tamanho/fontes. A logo permanece transparente sobre o cenário. | `drawOrbit`, `alignOrbit`, `ResizeObserver` em [website.js](../../prototypes/desktop/website.js) |
| Demonstração do fluxo | Cenários e etapas respondem a ações explícitas. “Play workflow” é opt-in, avança até a revisão e termina; navegação manual conserva o resultado legível. Pulsos e destaque acompanham a etapa apresentada. | `setPlaying`, `schedulePlay`, `renderStep` em [website.js](../../prototypes/desktop/website.js), [website.css](../../prototypes/desktop/website.css) |

Canvas e cenário têm `pointer-events:none`: o fundo não intercepta links, seleção de texto ou controles. A resposta ao ponteiro requer hover e ponteiro preciso; toque não ativa esse efeito. Ao sair da janela ou perder foco, a intensidade desaparece gradualmente.

### Parâmetros atuais, não novos requisitos

Os valores abaixo documentam o código em 10/10; futuras mudanças devem atualizar a fonte e este registro. São escolhas de implementação, não uma promessa de desempenho em todo equipamento.

- **22–72 partículas**, dimensionadas pela área; densidade de pixels limitada a **1,5**.
- Campo em aproximadamente **30 fps em repouso / 60 fps durante interação**; atualização dos conectores em aproximadamente **30 fps**.
- Suavização da posição do ponteiro com constante de **70ms**; entrada/saída de intensidade em **180/400ms**. Impulso de scroll limitado a **2,5**, com decaimento de **540ms**.
- Oscilação dos blocos do hero com amplitude de **3px**; alinhamento acompanha as posições reais, não coordenadas de borda fixas.
- Reprodução da demonstração com intervalo de **4,3 segundos**, encerrando na revisão.

Os registros de versões anteriores em [experiência progressiva](../../docs/design/d1-progressive-experience.md) incluem parâmetros históricos diferentes. Para novas alterações, use o código atual e os valores acima.

## Visibilidade e preferências do sistema

O site suspende animação quando a aba está oculta ou recebe `pagehide`. `IntersectionObserver` acompanha canvas, hero e demonstrador: o loop só continua quando existe camada animável visível; o timer da demonstração espera a volta da sua seção. `pageshow` reativa a sincronização sem transformar um retorno à página em reprodução automática não solicitada.

Com `prefers-reduced-motion: reduce`, partículas e blocos ficam estáticos, transições/animações CSS são removidas e a reprodução temporizada do fluxo fica desabilitada. Etapas manuais e a comparação Chat/Studio continuam operáveis. Alterar a preferência do sistema atualiza esse estado. Em cores forçadas, o site oculta camadas decorativas e preserva contornos de controles.

**Não há botão global “Pause animations”**: foi removido por decisão do responsável. “Play workflow / Pause workflow” controla somente a demonstração solicitada, não a ambientação. A [página do vídeo](../../prototypes/desktop/watch.html) usa player sem autoplay e mantém uma abertura útil enquanto o filme não foi publicado.

## Movimento no aplicativo

O app usa transições curtas de hover/borda/cor e indicação de expansão para comunicar interação. Não recebe o cenário de montanhas, canvas de estrelas ou efeitos de ponteiro do site. Ao mudar estado, idioma ou painel, a prioridade é continuar lendo, escrevendo e revisando com estabilidade.

Eventos passivos preservam foco, cursor, rascunho e posição. Ações explícitas podem focar o composer ou o destino apropriado; abrir/fechar um diálogo devolve foco útil. Não animar código, trocar seu conteúdo por decoração ou deslocar uma ação enquanto a pessoa a aciona. Texto de estado e revisão devem permanecer legíveis independentemente da animação.

[styles.css](../../prototypes/desktop/styles.css) remove animações, transições e rolagem suave com movimento reduzido; [experience.css](../../prototypes/desktop/experience.css) conserva a linguagem discreta dos controles. Os [testes de acessibilidade D1](../../prototypes/desktop/tests/d1-accessibility.spec.mjs) e [testes de conversa](../../prototypes/desktop/tests/project-chat.spec.mjs) cobrem movimento reduzido e preservação de foco/cursor. Preferência visual não muda política de execução.

## Verificar uma alteração de movimento

Use os [testes do site](../../prototypes/desktop/tests/site-modernization.spec.mjs) para etapas manuais, reprodução opt-in, suspensão fora da tela, movimento reduzido, teclado e fixação dos conectores. Examine o efeito de ponteiro/scroll em navegador real, além das capturas estáticas: screenshots mostram composição e legibilidade, mas não demonstram naturalidade do movimento.

Confira desktop e celular, resize, saída/retorno da aba e movimento reduzido nos efeitos alterados. Texto e ações continuam acessíveis sem depender do movimento. Sem JavaScript, o site conserva marca, explicação, plataformas e acesso à página do vídeo. Registre capturas ou evidência nova no [catálogo visual](../../docs/design/visual-asset-catalog.md), sem atribuir à alteração os resultados de uma rodada anterior.

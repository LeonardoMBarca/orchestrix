# Biblioteca de imagens e recursos visuais

O [catálogo textual de imagens](../../docs/design/visual-asset-catalog.md) é a fonte para localizar arquivos, entender sua aparência e identificar uso, origem e estado sem abrir cada imagem. Esta pasta documenta a manutenção do acervo; não duplica o catálogo nem os binários. A revisão de **10 de outubro de 2026** mapeia **53 imagens versionáveis** e **73 capturas locais de pesquisa e QA**.

Pesquisar o catálogo por assunto ou aliases em português e inglês, por exemplo `montanhas / mountains`, `voo noturno / night flight`, `sem fundo / transparent`, `Indigo`, `chat`, `review` ou `Medieval`. Os IDs `B`, `V`, `I`, `D` e `S` identificam ativos, conceitos e evidências versionáveis já catalogados.

## Onde guardar e encontrar

| Local | Conteúdo e uso |
| --- | --- |
| [Marca do protótipo](../../prototypes/desktop/assets/brand/README.md) | Rasters selecionados e fontes originais preservadas. Os lockups com fundo e o favicon anterior são referências disponíveis, não a aplicação atual da logo. |
| [Símbolos transparentes](../../prototypes/desktop/assets/brand/transparent/README.md) | Variantes Indigo, White e Graphite, galeria e prompts. Seguir o [guia de marca](../brand/README.md) para escolher a aplicação. |
| [Ambientações](../../prototypes/desktop/assets/visuals/README.md) | Imagem de voo noturno usada no site, vídeo e poster padrão. A paisagem é exclusiva da apresentação pública; não compõe o fundo do aplicativo. |
| [Ícones externos](../../prototypes/desktop/assets/icons/README.md) | Ícone oficial GitHub e sua procedência. Preservar o arquivo e a identidade do proprietário. |
| [Conceitos da Direção 01](../../docs/design/brand/direction-01-review/README.md) | Biblioteca de 23 conceitos raster, galeria e prompts; inclui opções sem uso atual. Aprovação da direção visual não promove todos os conceitos à interface. |
| `docs/design/visual-experiments/<collection>/` | Convenção para futuras coleções candidatas de fundos, texturas ou aplicações. Criar apenas quando houver conteúdo, com README, `images/` e prompts; não representa uma coleção já existente. |
| [Evidências de design](../../docs/design/) e [prévias de pesquisa](../../docs/research/previews/) | Capturas versionáveis de etapas específicas, vinculadas às decisões e entregas. Não usar screenshots como componentes, logos ou fundos do produto. |
| `prototypes/desktop/artifacts/` | Capturas e relatórios locais regeneráveis, ignorados pelo Git. Podem não existir em outro checkout; o catálogo registra os nomes e assuntos. Não é a biblioteca de estilização permanente. |

Estrelas, brilho, conectores e formas das interfaces atuais são desenhados por CSS, SVG embutido e canvas. Não procurar um PNG para cada efeito. O [README de vídeo](../../prototypes/desktop/assets/video/README.md) explica a futura mídia e sua configuração; a pasta não implica que o filme já exista.

## Registro de novas imagens

Toda nova imagem, variante ou coleção deve ser encontrável textualmente **no mesmo incremento e antes de seu uso na interface**, inclusive quando ainda for candidata. Atualizar o catálogo e o README da coleção com:

- nome descritivo e caminho real; assunto, aparência e aliases em português e inglês;
- finalidade, estado — candidato, selecionado, histórico ou evidência — e pontos de uso;
- origem, data, referência utilizada e prompt quando gerada ou editada; manter a relação fonte → variante → aplicação;
- dimensões, transparência e enquadramento relevantes, distinguindo observações medidas de regras pretendidas;
- procedência e condições de uso para fontes externas; fluxo, data/build e contexto para screenshots;
- hash quando for usado para comprovar cópia ou integridade. Preservar as fontes e não afirmar equivalência entre arquivos sem verificar.

Os sete PNGs originais copiados dos conceitos tiveram igualdade SHA-256 registrada no catálogo. Essa verificação não se estende automaticamente a novas derivações. Nomes como `image-final-2.png` e descrições apenas visuais dificultam encontrar um recurso depois; usar nomes de assunto e variante, com metadados próximos aos arquivos.

## Da opção disponível à aplicação

Confirmar coerência com a Direção 01 e com a superfície de destino; verificar transparência, recorte, contraste e legibilidade no tamanho real. Preservar a fonte conceitual, selecionar apenas a variante necessária para a pasta de ativos e atualizar o catálogo com os usos. Uma variante para favicon não redefine a logo principal; uma alternativa disponível não é um novo padrão aprovado.

Se uma imagem deixar de ser usada, registrar a mudança de estado antes de removê-la. Se uma captura local precisar acompanhar uma entrega, selecionar a evidência, registrar seu contexto e manter uma cópia versionável apropriada. Referências de sites de terceiros permanecem evidências de pesquisa, sem promoção a recursos visuais do Orchestrix.

# Estratégias de escolha de modelo e raciocínio

**Consolidação de produto em 09/10/2026.** O Orchestrix deve ser útil com uma única conta, um único runtime e execução serial. A seleção automática de modelo e esforço de raciocínio considera complexidade, risco, importância e tipo de tarefa, dentro das capacidades disponíveis e da política do usuário. Os novos nomes de estratégias e os presets especializados abaixo tornam essa proposta mais fácil de configurar; não representam um router já implementado.

## O que já está definido e o que este complemento acrescenta

O suporte a um runtime é uma decisão aceita no [ADR-0001](../adr/0001-single-runtime-first-class.md). Modelo e raciocínio configuráveis, tradução pelo adapter e registro solicitado/efetivo estão aceitos no [ADR-0002](../adr/0002-model-and-reasoning-policy.md) e detalhados em [Model and Reasoning Policy](../REASONING_AND_MODEL_POLICY.md). O [plano, seção 5](../DEVELOPMENT_PLAN.md#5-escolha-de-conta-modelo-e-thinking), já define o router inicial determinístico, a precedência e os limites obrigatórios.

Este complemento acrescenta a nomenclatura **Efficiency, Performance, Balanced e Custom**, presets especializados, a importância como dimensão explícita e critérios para demonstrar a experiência com uma conta. Mantém os contratos existentes; não cria outro schema de política ou exige aprendizado de máquina. O [Control Center](../ORCHESTRATION_CONTROL_CENTER.md) continua sendo a superfície avançada de configuração. Seus presets anteriores são referências de intenção; os novos nomes devem ser reconciliados durante o schema e a implementação, sem prometer equivalência de campos ainda não definidos.

## Uma conta, trabalho coordenado

Uma conta compatível pode realizar uma sequência de planejamento, implementação, verificação e revisão, com papéis em sessões independentes quando exigido. Paralelismo é opcional e condicionado ao suporte do runtime, à política e à capacidade observada; não constitui pré-requisito de valor.

Dentro da mesma conexão, o Orchestrix pode escolher modelos diferentes e ajustar raciocínio quando esses controles forem oficialmente expostos. Se houver apenas um modelo elegível, usa esse modelo. Se o runtime gerenciar um parâmetro sem expô-lo, registra `runtime-managed`; quando não houver confirmação, registra `unknown`. A interface não deve transformar essas situações em uma escolha fictícia.

Mais contas ou runtimes ampliam as alternativas. Não são necessários para as estratégias básicas, e novas sessões não criam novas cotas. Connections que compartilham capacidade seguem o grupo correspondente.

## Estratégias gerais

| Estratégia | Intenção | Comportamento esperado |
| --- | --- | --- |
| **Efficiency** | Evitar trabalho de modelo e consumo desnecessários, preservando os requisitos da tarefa. | Preferir ferramentas determinísticas para operações mecânicas, contexto focado e configuração mais leve elegível. Aumentar modelo/raciocínio quando complexidade, risco ou critérios exigirem. Não retirar revisão ou checks obrigatórios para economizar. |
| **Performance** | Priorizar capacidade cognitiva e qualidade onde julgamento tem valor. | Preferir os modelos declarados mais capazes e esforço elevado para tarefas exigentes, com revisão adequada. Não implica máxima concorrência, uso ilimitado, nem raciocínio máximo para consultar fatos mecânicos. |
| **Balanced** | Distribuir esforço conforme a necessidade do trabalho. | Usar configuração moderada no trabalho rotineiro e elevar recursos para alta complexidade, risco ou importância. Evitar duplicação opcional sem diminuir as exigências de validação. |
| **Custom** | Permitir escolhas explícitas e regras do usuário. | Definir modelos, esforços, prioridades, limites, fallbacks e condições por escopo. Toda escolha continua sujeita a capacidades e restrições obrigatórias. |

`Efficiency` não promete porcentagem de economia de assinatura; `Performance` não promete superioridade medida. “Mais capaz” e “mais rápido” usam ordenação declarada pelo adapter ou pelo usuário, com origem visível. Nomes de modelos não estabelecem um ranking universal entre provedores.

## Presets especializados

Esses presets preenchem preferências de estratégia, modelo/raciocínio, contexto e revisão para uma classe de trabalho. Não são novos modos de ação: `Research` descreve o trabalho; `Scientific Research` descreve uma preferência de execução para esse trabalho. Os nomes de exibição usam espaços; IDs e schema serão definidos na implementação.

| Preset | Preferências que deve tornar configuráveis |
| --- | --- |
| **Scientific Research** | Análise criteriosa de fontes, contexto com referências rastreáveis e raciocínio elevado em síntese ou avaliação crítica. Busca e ferramentas científicas dependem de suporte e permissão. |
| **Software Architecture** | Raciocínio elevado para contratos, dependências e tradeoffs; contexto arquitetural pertinente e revisão de decisões de maior impacto. |
| **Complex Planning** | Maior esforço para decomposição, dependências, critérios de aceite e limites; validação determinística do plano antes da execução. |
| **Debugging** | Ajustar esforço à dificuldade da falha, preservando reprodução, logs e evidências; elevar recursos para problemas complexos sem substituir testes por opiniões. |
| **Security Review** | Raciocínio elevado para análise de riscos, contexto de ameaças e revisão independente, obedecendo permissões do papel e verificações obrigatórias. |

Os presets usam a mesma resolução da estratégia geral. Não concedem acesso a ferramentas, dados, shell ou rede; não exigem outra família de modelo salvo quando a política declarar isso como obrigação.

## Entradas e escopos

O router considera classificações explícitas com origem registrada:

- **Tipo:** natureza da tarefa e capacidades necessárias, como implementação, pesquisa, documentação, planejamento ou revisão.
- **Complexidade:** dificuldade e dependências do problema; não equivale ao tamanho do prompt.
- **Risco:** consequências possíveis de erro, como mudança de autenticação ou alteração destrutiva.
- **Importância:** impacto e prioridade definidos para o objetivo; não elimina a avaliação de risco.

Esses valores podem vir do usuário ou de um plano validado. Uma estimativa produzida por um agente permanece identificada como estimativa e pode ser corrigida. Valores ausentes não se tornam automaticamente baixos; a política define um padrão explícito ou exige esclarecimento para casos sensíveis.

Manter a precedência existente para preferências: **sistema → usuário/global → projeto → AgentProfile → categoria/risco → execução → tarefa**. Restrições obrigatórias são limites acumulados, não preferências que um escopo mais específico pode apagar. Estratégia, preset, modo e perfil devem aparecer na configuração resolvida com sua origem; a integração de seus valores padrão ao schema pertence a OX-008.

## Router V1: regras determinísticas

1. **Distinguir fato mecânico de julgamento.** Consultar status/diff do Git, existência de arquivos, exit codes, resultado de testes e estado de processo usando ferramentas determinísticas. Não consumir uma chamada de modelo apenas para descobrir esses fatos. Análise e interpretação de um resultado podem justificar trabalho de modelo.
2. **Resolver requisitos e preferências.** Combinar tarefa, classificações, estratégia, preset, modo/perfil e política, preservando a origem dos valores e restrições obrigatórias.
3. **Eliminar candidatos incompatíveis.** Verificar conexão/autenticação, modalidade de cobrança, capacidades, permissões, contexto, ferramentas, revisão exigida e capacidade de execução. Modelo ou raciocínio obrigatório não suportado não passa por uma preferência de velocidade ou economia.
4. **Aplicar regras e ordem declaradas.** Mapear as classificações para a intenção de modelo/raciocínio e selecionar entre candidatos elegíveis por preferências ordenadas e disponibilidade. Usar desempate estável; mesmas entradas, versões de política e estado observado produzem a mesma decisão.
5. **Traduzir pelo adapter.** Converter a intenção para os controles oficialmente expostos naquela combinação de conexão, runtime, versão e modelo. Registrar solicitado, confirmado como efetivo, `unknown` ou `runtime-managed`, conforme a evidência.
6. **Persistir a decisão e o snapshot.** Gravar regras aplicadas, candidatos excluídos, escolha e fallback junto da tentativa antes da execução pelo supervisor, respeitando os contratos de admissão e persistência do Core.

Uma matriz inicial pode usar configuração mais leve para tarefa simples de baixo risco, moderada para trabalho rotineiro e elevada para alta complexidade, risco ou importância. Esses valores são regras configuráveis e deverão ser validados em OX-008; não presumem níveis equivalentes entre provedores. `maximum` é o máximo oficialmente suportado pelo modelo/runtime selecionado.

O router V1 não depende de histórico de sucesso, ranking aprendido ou estimativas inventadas de custo/cota. O exemplo YAML existente menciona scoring histórico como capacidade futura; não é o algoritmo obrigatório do primeiro router. Classificações podem orientar regras estáticas antes de existir planejamento automático completo de M4.

## Explicação, intervenção e fallback

A pessoa deve conseguir inspecionar por que a configuração foi escolhida: classificações da tarefa, regras do preset, alternativas elegíveis, exclusões, conexão, modelo/raciocínio solicitado e efetivo, revisão exigida e motivo de fallback. A explicação usa dados registrados da decisão, sem depender de raciocínio oculto do modelo.

O usuário pode fixar conexão, modelo ou esforço onde houver suporte, editar regras e corrigir classificações. Um override incompatível produz diagnóstico; não ignora permissões ou requisitos obrigatórios. Edições afetam novas decisões e conservam os snapshots das tentativas anteriores. Reatribuir trabalho ainda não iniciado é distinto de interromper uma tentativa ativa e criar outra.

Se faltar uma preferência, aplicar apenas uma alternativa permitida e registrar o motivo. Se faltar um requisito obrigatório, bloquear com explicação. Esgotamento de tentativas, timeout e limites definidos pelo usuário continuam exigindo a ação prevista na política; estratégia não cria retries ilimitados nem transforma erro de autenticação em autorização para outra conta.

## Assinatura, cota e limites observáveis

**Subscription Only** conserva a regra existente: usar apenas a modalidade oficial de assinatura validada pelo adapter, sem fallback silencioso para API cobrada. Confirmar origem da autenticação e comportamento de cobrança; remover variáveis de API sozinho não prova esse isolamento. Uma política estrita não inicia execução quando a modalidade não puder ser confirmada.

Escolha automática não autoriza acesso a um modelo ou nível de raciocínio que a assinatura/runtime não exponha oficialmente. Múltiplas contas próprias não autorizam contornar limites ou restrições do provedor.

Capacidade concorrente configurada, número de tentativas e timeouts podem ser limites locais determinísticos. Saldo da assinatura, reset, tokens disponíveis e custos financeiros só podem orientar decisões como informações reais quando forem observáveis e identificados por fonte/instante. Na ausência de suporte, mostrar `unknown`; um limite local não representa saldo de cota. Sinal real de rate limit pode suspender admissões e acionar um fallback permitido, sem inventar capacidade restante.

## Encaixe no desenvolvimento e aceite

| Etapa | Entrega necessária |
| --- | --- |
| **D1** | Validar escolha simples da estratégia e acesso à explicação/configuração avançada, inclusive em uma conta e execução serial. |
| **M0 / OX-001–OX-003** | Demonstrar capacidades de modelo/raciocínio por conexão e versão, autenticação/cobrança e classificação de limitações do runtime. |
| **M1 / OX-008** | Implementar resolução, regras por classificações, elegibilidade, fallback explícito e snapshot usando o contrato validado. Não aguardar ML ou histórico de desempenho. |
| **M2 / OX-016–OX-017** | Entregar estratégias e presets no Control Center, overrides e explicações; demonstrar execução com um runtime e concorrência 1. |
| **M4 e etapas posteriores** | Ampliar classificação/planejamento automático e avaliar ajustes apoiados por dados reais, preservando as mesmas regras e auditoria. |

Critérios de aceite: uma conta percorre trabalho e revisão de forma serial; tarefas simples e complexas produzem decisões justificadas quando houver controles compatíveis; parâmetros não suportados têm fallback ou bloqueio explícitos; alterações de preferência preservam tentativas antigas; consulta de status Git não requer modelo; cota sem telemetria permanece desconhecida; nenhum fallback viola Subscription Only.

## Referências

- [Modos de ação e perfis executores](./action-modes-and-worker-profiles.md).
- [Model and Reasoning Policy](../REASONING_AND_MODEL_POLICY.md).
- [Orchestration Control Center](../ORCHESTRATION_CONTROL_CENTER.md).
- [Política ilustrativa, sem schema público estável](../examples/orchestration-policy.example.yaml).
- [Plano de desenvolvimento](../DEVELOPMENT_PLAN.md) e [backlog](../DEVELOPMENT_BACKLOG.md).

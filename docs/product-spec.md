# Construct — Especificação de Produto

> Documento vivo que define o comportamento esperado do produto. Decisões de implementação devem preservar estes requisitos ou atualizar explicitamente esta especificação.

| Campo | Valor |
| --- | --- |
| Status | Preview funcional em fase de hardening |
| Versão | 0.16 |
| Data | 29 de julho de 2026 |
| Plataforma principal | macOS |
| Preview adicional | Windows x64 com índice local e MCP |
| Plataforma futura | Linux |
| Nome do produto | **Construct** |

## 1. Resumo

Construct é um aplicativo desktop para localizar, acompanhar, visualizar e editar arquivos de contexto produzidos por coding agents em diferentes projetos.

O produto atende principalmente pessoas que trabalham com agentes pelo terminal e não têm acesso conveniente aos arquivos do projeto durante a sessão. O aplicativo permite cadastrar pastas monitoradas, navegar recursivamente pelos arquivos Markdown, abrir múltiplos documentos em abas e painéis, visualizar Markdown renderizado, editar o conteúdo e acompanhar mudanças feitas externamente pelos agentes.

O aplicativo é um companion para o fluxo de trabalho com agentes. Ele não pretende substituir uma IDE, um terminal, um cliente Git ou um gerenciador de arquivos completo.

## 2. Problema

Coding agents frequentemente produzem planos, relatórios, memórias, especificações e outros artefatos em Markdown. Quando a interação com esses agentes acontece em terminais — especialmente dentro de multiplexadores como o Herdr — esses arquivos ficam pouco visíveis.

O usuário precisa interromper o fluxo, abrir outra ferramenta, encontrar o projeto e navegar até o arquivo. Quando existem vários agentes e vários projetos ativos, também é difícil saber o que foi criado ou alterado recentemente.

Os principais problemas são:

- falta de uma visão unificada das pastas usadas com agentes;
- dificuldade para descobrir arquivos criados ou alterados durante as sessões;
- atrito para ler Markdown renderizado;
- necessidade de abrir uma IDE apenas para fazer pequenas edições;
- dificuldade para comparar arquivos ou consultar dois documentos simultaneamente;
- falta de um histórico simples das mudanças observadas em vários projetos.

## 3. Visão do produto

Ser o lugar mais rápido e agradável para acompanhar os arquivos de contexto gerados por agentes, independentemente do terminal, agente ou projeto usado.

O produto deve transmitir a sensação de uma mesa de leitura dedicada: projetos à esquerda, documentos organizados e a área de trabalho livre para ler, editar e comparar conteúdos.

## 4. Objetivos

### 4.1 Objetivos do produto atual

- Permitir que o usuário cadastre uma ou mais pastas locais.
- Descobrir recursivamente arquivos Markdown dentro dessas pastas.
- Mostrar arquivos ocultos relevantes ao trabalho com agentes.
- Atualizar a interface conforme arquivos forem criados, alterados, renomeados ou removidos externamente.
- Oferecer leitura em Preview e edição em Edit ou Source.
- Permitir edição e salvamento explícito dos arquivos.
- Permitir vários documentos em abas e vários painéis lado a lado.
- Manter um histórico persistente das mudanças observadas.
- Mostrar diff somente para arquivos dentro de repositórios Git.
- Restaurar o workspace entre execuções.
- Manter uma arquitetura que não impeça suporte futuro a Windows e Linux.

### 4.2 Indicadores de sucesso iniciais

Os indicadores abaixo orientam validação qualitativa e telemetria futura; não
exigem coleta de dados no preview.

- O usuário consegue adicionar um projeto e abrir um arquivo Markdown em menos de um minuto.
- Um arquivo criado por um agente aparece no aplicativo sem ação manual de atualização.
- O usuário consegue encontrar um arquivo recente sem saber seu caminho exato.
- O usuário consegue consultar dois documentos lado a lado sem abrir outra ferramenta.
- O aplicativo pode ser reaberto e retomar o workspace anterior.
- Nenhuma edição local é perdida silenciosamente por causa de uma mudança externa.

## 5. Não objetivos do produto atual

- Substituir uma IDE ou editor de código completo.
- Executar coding agents automaticamente ou hospedar terminais dentro do
  Construct.
- Fazer commits, staging, checkout, merge ou outras operações de escrita no Git.
- Sincronizar arquivos entre dispositivos.
- Colaborar em tempo real com outras pessoas.
- Guardar versões históricas completas dos arquivos.
- Visualizar ou editar formatos além de Markdown.
- Gerenciar arquivos de maneira completa, incluindo criar pastas, copiar, mover e excluir.
- Instalar plugins ou executar extensões de terceiros.
- Executar reparos OKF automáticos ou reescrever frontmatter.
- Usar LLMs, embeddings remotos ou serviços hospedados para busca.

## 6. Princípios de produto

1. **Local first:** o conteúdo permanece no computador do usuário e não depende de uma conta ou serviço remoto.
2. **Arquivos são a fonte da verdade:** o aplicativo lê e escreve os arquivos reais; não cria um formato proprietário para o conteúdo.
3. **Mudanças externas são esperadas:** coding agents e outras ferramentas podem modificar um arquivo a qualquer momento.
4. **Nunca perder trabalho silenciosamente:** conflitos e falhas de salvamento devem ser explícitos.
5. **Leitura em primeiro lugar:** Markdown renderizado deve ser agradável, rápido e fiel.
6. **Complexidade progressiva:** a interface inicial deve ser simples, enquanto abas, painéis, Git e histórico aparecem quando necessários.
7. **Portabilidade deliberada:** comportamentos centrais não devem depender de APIs exclusivas do macOS quando houver uma abstração multiplataforma razoável.

## 7. Terminologia

| Termo | Definição |
| --- | --- |
| Local | Pasta raiz cadastrada e monitorada pelo usuário. Pode representar um projeto inteiro ou uma pasta específica de contexto. |
| Arquivo | Documento Markdown descoberto dentro de um Local. |
| Aba | Representação de um arquivo aberto dentro de um Painel. |
| Painel | Área de leitura ou edição que contém uma ou mais abas. Também chamado de pane. |
| Workspace | Conjunto de Locais, abas, painéis, seleções e preferências restauráveis. |
| Preview | Markdown renderizado. |
| Edit | Edição visual do corpo Markdown, com comandos e formatação contextual. |
| Review | Leitura renderizada com comentários persistentes destinados ao próximo ciclo com um agente. |
| Source | Conteúdo Markdown bruto e editável. |
| Mudança externa | Alteração no sistema de arquivos feita fora do aplicativo. |
| Histórico | Linha do tempo local de eventos observados pelo aplicativo. |
| Diff Git | Comparação somente leitura entre o arquivo atual e a referência Git definida nesta especificação. |

## 8. Público e cenários principais

### 8.1 Público inicial

Desenvolvedores que:

- usam coding agents por terminal;
- trabalham com múltiplas sessões ou projetos;
- recebem artefatos Markdown produzidos por agentes;
- querem acompanhar esses artefatos sem abrir uma IDE completa.

### 8.2 Cenários principais

#### Acompanhar um agente

1. O usuário cadastra a pasta do projeto.
2. O agente cria `implementation-plan.md`.
3. O arquivo aparece na árvore e no Histórico.
4. O usuário abre o arquivo em Preview.
5. Quando o agente atualiza o plano, o Preview é atualizado automaticamente.

#### Editar uma especificação

1. O usuário abre um arquivo em Edit ou Source.
2. Faz alterações; a aba passa a indicar estado não salvo.
3. Pressiona `⌘S`.
4. O arquivo é salvo no disco e o indicador desaparece.
5. A mudança passa a constar no Histórico e, se aplicável, no Diff Git.

#### Comparar contexto

1. O usuário abre dois arquivos em abas.
2. Divide a área de trabalho vertical ou horizontalmente.
3. Move ou abre um dos arquivos no segundo painel.
4. Mantém um documento em Edit ou Source e outro em Preview.

#### Revisar mudanças recentes

1. O usuário abre o aplicativo depois de trabalhar em vários projetos.
2. O Histórico mostra arquivos modificados e eventos detectados desde a última sessão.
3. O usuário seleciona um evento para abrir o arquivo e seu Local.
4. Se o arquivo estiver em Git, pode abrir o Diff Git.

## 9. Arquitetura da informação

A janela principal possui duas regiões:

```text
┌──────────────────────┬─────────────────────────────────────────────┐
│ LOCAIS               │ Painel 1                     │ Painel 2     │
│  Projeto A           │ plano.md | notas.md          │ AGENTS.md    │
│  Projeto B           ├──────────────────────────────┼──────────────┤
├──────────────────────┤                              │              │
│ ARQUIVOS             │ Preview, Edit, Review ou Source │ Preview   │
│  docs/               │                              │              │
│    plano.md          │                              │              │
│  AGENTS.md           │                              │              │
├──────────────────────┤                              │              │
│ HISTÓRICO            │                              │              │
│  plano.md        2m  │                              │              │
│  AGENTS.md      15m  │                              │              │
└──────────────────────┴──────────────────────────────┴──────────────┘
```

### 9.1 Sidebar

A sidebar é redimensionável e contém três seções também redimensionáveis:

1. **Locais:** lista das pastas cadastradas.
2. **Arquivos:** árvore recursiva do Local selecionado.
3. **Histórico:** eventos recentes agregados de todos os Locais.

Cada seção pode ser recolhida. O aplicativo deve preservar seus tamanhos e estados de expansão.
Recolher uma seção reduz sua altura ao cabeçalho e devolve imediatamente o
espaço às demais. Cada corpo possui scroll independente e mantém o cabeçalho
visível. Controles globais da sidebar, como tema, visibilidade e conexão de
agentes, permanecem fora das seções e não rolam com Locais.

### 9.2 Área principal

A área principal contém um ou mais Painéis. Cada Painel possui:

- barra de abas;
- indicação de arquivo ativo;
- indicação de edição não salva;
- seletor ou ação para alternar entre Preview, Edit, Review e Source;
- conteúdo do arquivo;
- estados de carregamento, erro, conflito e arquivo indisponível.

Os temas claro e escuro devem manter contraste legível em texto primário,
texto secundário, dicas de teclado, campos e resultados de superfícies
flutuantes. Essas superfícies acompanham o tema ativo em vez de misturar
cores fixas de temas diferentes.

O chrome da aplicação — sidebar, abas, barras, busca, metadados e superfícies
flutuantes — usa JetBrains Mono empacotada com o aplicativo. Preview e edição
visual usam Source Serif 4, também empacotada, para dar ao documento uma
superfície editorial confortável para leitura longa; código, caminhos e Source
continuam usando a fonte monoespaçada. Cores de texto, cursor, linha ativa e
seleção do Source devem ser definidas por tema e manter contraste em ambos.

## 10. Requisitos funcionais

### 10.1 Locais

- **LOC-001:** O usuário pode adicionar um Local usando o seletor nativo de pastas.
- **LOC-002:** O aplicativo deve solicitar apenas as permissões necessárias para ler e editar o Local escolhido.
- **LOC-003:** O mesmo caminho não pode ser cadastrado duas vezes.
- **LOC-004:** Cada Local exibe por padrão o nome da pasta raiz.
- **LOC-005:** O usuário pode remover um Local do aplicativo sem apagar ou alterar a pasta no disco.
- **LOC-006:** O usuário pode revelar um Local no gerenciador de arquivos do sistema.
- **LOC-007:** O aplicativo deve preservar a ordem dos Locais.
- **LOC-008:** O usuário pode reordenar os Locais por arrastar e soltar.
- **LOC-009:** Quando um Local estiver indisponível, ele permanece cadastrado e recebe um estado visual de indisponibilidade.
- **LOC-010:** Quando um Local voltar a ficar disponível, o aplicativo deve retomar a varredura e o monitoramento automaticamente.
- **LOC-011:** Um Local pode ser uma raiz de projeto ou qualquer subdiretório escolhido pelo usuário.
- **LOC-012:** Se Locais cadastrados se sobrepuserem, o aplicativo deve evitar duplicar eventos internamente, mas pode exibir o arquivo no contexto de cada Local.

### 10.2 Descoberta e árvore de arquivos

- **FILE-001:** A descoberta deve ser recursiva a partir da raiz do Local.
- **FILE-002:** O preview reconhece extensões `.md` e `.markdown`, sem diferenciar maiúsculas de minúsculas.
- **FILE-003:** Arquivos e diretórios ocultos devem aparecer normalmente, exceto quando estiverem na lista de exclusão padrão.
- **FILE-004:** A árvore deve representar a hierarquia relativa ao Local.
- **FILE-005:** Diretórios vazios, considerando os filtros ativos, não precisam aparecer.
- **FILE-006:** Diretórios podem ser expandidos e recolhidos.
- **FILE-007:** O estado de expansão deve ser preservado entre execuções quando for razoável.
- **FILE-008:** A árvore deve ter atualização incremental, evitando reconstruções visuais completas a cada evento.
- **FILE-009:** Arquivos com o mesmo nome devem ser distinguíveis pelo caminho relativo.
- **FILE-010:** O menu de contexto do arquivo deve oferecer ao menos Abrir, Abrir à direita, Revelar no Finder e Copiar caminho.
- **FILE-011:** Links simbólicos não devem ser percorridos recursivamente no preview, evitando ciclos e fuga acidental do Local.
- **FILE-012:** Arquivos acessados por link simbólico podem ser marcados como não suportados até uma decisão futura.

### 10.3 Exclusões padrão

A varredura e o monitoramento devem ignorar diretórios de controle de versão, dependências, ambientes, caches e artefatos de build. A comparação do nome deve respeitar as regras de sensibilidade a maiúsculas do sistema de arquivos.

Lista inicial:

```text
.git
.hg
.svn
node_modules
vendor
.venv
venv
__pycache__
.pytest_cache
.mypy_cache
.ruff_cache
target
dist
build
out
.next
.nuxt
.svelte-kit
.gradle
.idea
Pods
DerivedData
bin
obj
.terraform
.dart_tool
.pub-cache
coverage
.coverage
```

- **IGNORE-001:** As exclusões são aplicadas em qualquer profundidade.
- **IGNORE-002:** O diretório `.git` pode ser consultado indiretamente por operações Git, mas nunca aparece na árvore.
- **IGNORE-003:** O preview não oferece interface para editar a lista.
- **IGNORE-004:** A arquitetura deve permitir exclusões globais e por Local em uma versão futura.
- **IGNORE-005:** O aplicativo não precisa respeitar `.gitignore`, pois arquivos de contexto ignorados pelo Git ainda podem ser relevantes.

### 10.4 Abertura e navegação

- **NAV-001:** Um clique em um arquivo o abre no Painel ativo.
- **NAV-002:** Se o arquivo já estiver aberto nesse Painel, sua aba deve receber foco.
- **NAV-003:** Se o arquivo estiver aberto em outro Painel, o comportamento padrão é focar a instância existente.
- **NAV-004:** Deve existir uma ação explícita para abrir outra visualização do mesmo arquivo em outro Painel.
- **NAV-005:** O aplicativo deve preservar a posição de rolagem separadamente para Preview, Edit, Review e Source enquanto a aba estiver aberta.
- **NAV-008:** Ao alternar explicitamente entre modos, o aplicativo deve tentar
  manter o mesmo bloco semântico visível e usar uma posição proporcional apenas
  quando esse bloco não puder ser reencontrado.
- **NAV-006:** Ao selecionar um evento no Histórico, o Local correspondente é selecionado, o arquivo é revelado na árvore e sua aba recebe foco.
- **NAV-007:** O caminho relativo completo deve estar disponível na interface, mesmo que não permaneça visível o tempo todo.

### 10.5 Abas

- **TAB-001:** Cada Painel pode conter múltiplas abas.
- **TAB-002:** Abas podem ser reordenadas por arrastar e soltar.
- **TAB-003:** Abas podem ser movidas entre Painéis.
- **TAB-004:** Uma aba modificada deve exibir um indicador inequívoco de conteúdo não salvo.
- **TAB-005:** Fechar uma aba modificada abre uma confirmação com as opções Salvar, Não salvar e Cancelar.
- **TAB-006:** `⌘W` fecha a aba ativa; se for a última aba, o Painel pode permanecer vazio.
- **TAB-007:** O título da aba usa o nome do arquivo e oferece o caminho relativo em tooltip ou elemento equivalente.
- **TAB-008:** O aplicativo deve indicar quando duas abas possuem nomes iguais e pertencem a caminhos diferentes.
- **TAB-009:** O menu de contexto de uma aba deve oferecer ao menos Recarregar do disco, Copiar caminho e Revelar no Finder.

### 10.6 Painéis

- **PANE-001:** O usuário pode dividir um Painel verticalmente ou horizontalmente.
- **PANE-002:** A divisão cria um novo Painel ao lado do atual.
- **PANE-003:** Os Painéis são redimensionáveis.
- **PANE-004:** Cada Painel mantém suas próprias abas e aba ativa.
- **PANE-005:** Cada aba mantém seu próprio modo Preview, Edit, Review, Source ou Diff.
- **PANE-006:** O usuário pode fechar um Painel; suas abas devem ser movidas ou fechadas de maneira segura.
- **PANE-007:** Não há limite funcional rígido de Painéis, embora a interface possa impedir divisões que resultem em áreas inutilizáveis.
- **PANE-008:** Arrastar uma aba para uma borda pode criar uma divisão como aprimoramento, mas ações explícitas devem existir no preview.
- **PANE-009:** O layout dos Painéis deve ser restaurado entre execuções.

### 10.7 Edição visual e Source

- **EDIT-001:** Source exibe o conteúdo Markdown bruto em um editor de texto monoespaçado.
- **EDIT-002:** O editor deve oferecer numeração de linhas e destaque de sintaxe Markdown.
- **EDIT-003:** O produto usa salvamento explícito; não há autosave.
- **EDIT-004:** `⌘S` salva o arquivo ativo.
- **EDIT-005:** O salvamento deve minimizar risco de corrupção e evitar deixar arquivos parcialmente escritos.
- **EDIT-006:** Depois do salvamento bem-sucedido, a aba deixa o estado não salvo.
- **EDIT-007:** Uma falha de salvamento preserva o buffer e mostra uma mensagem acionável.
- **EDIT-008:** O aplicativo deve preservar o tipo de quebra de linha existente quando possível.
- **EDIT-009:** UTF-8 é a codificação suportada no preview. Arquivos inválidos devem abrir em estado de erro sem tentativa silenciosa de conversão.
- **EDIT-010:** Desfazer e refazer devem funcionar enquanto a aba permanecer aberta.
- **EDIT-011:** O aplicativo deve fornecer operações básicas de seleção, copiar, recortar, colar e localizar dentro do arquivo aberto.
- **EDIT-012:** A alternância para Preview não salva automaticamente o arquivo.
- **EDIT-013:** O Preview de uma aba modificada deve renderizar o buffer local, não a versão antiga do disco.
- **EDIT-014:** Edit oferece edição visual do corpo do documento com títulos, parágrafos, listas, checklists, citações, links, tabelas e blocos de código.
- **EDIT-015:** Preview, Edit e Source compartilham o mesmo buffer local e o mesmo estado de salvamento explícito.
- **EDIT-016:** Abrir Edit ou alternar entre modos sem fazer alterações não pode marcar a aba como modificada nem normalizar o arquivo no disco.
- **EDIT-017:** Ao desfazer todas as alterações visuais, o buffer deve voltar ao conteúdo fonte exato que existia antes da edição, incluindo quebras de linha e serialização Markdown.
- **EDIT-018:** Frontmatter YAML deve ser preservado byte a byte por Edit. A edição de metadados continua disponível em Source.
- **EDIT-019:** Frontmatter não fechado ou estruturalmente inválido impede Edit de iniciar e oferece uma ação para abrir Source, sem ocultar ou alterar o conteúdo.
- **EDIT-020:** Edit não deve oferecer upload ou colagem persistente de imagens enquanto não existir uma estratégia local que produza referências de arquivo estáveis.
- **EDIT-021:** Diagramas Mermaid permanecem editáveis como blocos de código em Edit e são renderizados em Preview.
- **EDIT-022:** A primeira edição visual real pode serializar o corpo segundo a forma canônica do editor; o frontmatter permanece intacto e o Diff Git torna essa alteração explícita.

### 10.7.1 Review

- **REVIEW-001:** Review permite selecionar um trecho do documento renderizado e associar uma observação textual a essa seleção.
- **REVIEW-002:** Comentários são armazenados no próprio Markdown em um bloco HTML `construct-review:v1` imediatamente depois do frontmatter, ou no início do documento quando não houver frontmatter.
- **REVIEW-003:** O bloco de review é invisível em Preview, mas permanece legível em Source e para agentes que leem o arquivo.
- **REVIEW-004:** O bloco contém identificador, trecho citado, comentário e instante de criação de cada observação aberta.
- **REVIEW-005:** Adicionar, remover ou limpar comentários modifica apenas o buffer local e respeita o mesmo salvamento explícito de Edit e Source.
- **REVIEW-006:** O usuário pode remover uma observação individual ou limpar toda a rodada. Limpar todas restaura exatamente o conteúdo que existia antes da criação do bloco.
- **REVIEW-007:** Review oferece uma ação para copiar um prompt autocontido com o caminho relativo, trechos e comentários abertos, sem remover o tracking mantido no arquivo.
- **REVIEW-008:** O prompt copiado orienta o agente a atualizar o documento, remover comentários resolvidos e preservar observações ainda não tratadas.
- **REVIEW-009:** Edit preserva o bloco de review sem exibi-lo dentro do corpo visual.
- **REVIEW-010:** Links Markdown citados dentro de comentários não participam da navegação, backlinks ou Graph do bundle OKF.
- **REVIEW-011:** Um bloco de review malformado nunca deve ser reescrito automaticamente; o aplicativo mantém o documento acessível em Source e apresenta erro acionável.
- **REVIEW-012:** Comentários guardam um snapshot textual da seleção. Alterações posteriores no trecho não apagam a observação silenciosamente.
- **REVIEW-013:** Novos comentários podem guardar uma âncora opcional com
  offsets normalizados e contexto anterior e posterior sem alterar o trecho
  visível do Markdown.
- **REVIEW-014:** Comentários antigos sem âncora continuam legíveis e só são
  associados visualmente quando seu trecho possui uma ocorrência inequívoca.
- **REVIEW-015:** Review destaca passagens resolvidas em tempo de renderização;
  os destaques não são inseridos no corpo Markdown.
- **REVIEW-016:** Selecionar um destaque revela seu comentário, e selecionar um
  comentário navega para o destaque correspondente.
- **REVIEW-017:** Quando uma passagem não puder ser localizada com segurança, o
  comentário permanece visível com estado `Passage changed`; o aplicativo não
  escolhe uma ocorrência ambígua.
- **REVIEW-018:** Adicionar, remover ou limpar comentários não deve recriar a
  superfície renderizada nem alterar a posição de leitura do documento.

### 10.8 Preview Markdown

- **PREVIEW-001:** O Preview deve suportar CommonMark e extensões amplamente usadas no GitHub Flavored Markdown.
- **PREVIEW-002:** Deve renderizar títulos, listas, citações, links, imagens, tabelas, separadores e blocos de código.
- **PREVIEW-003:** Checklists devem ser renderizadas; sua interação pode ser apenas visual no preview.
- **PREVIEW-004:** Blocos de código devem ter syntax highlighting quando a linguagem for reconhecida.
- **PREVIEW-005:** Diagramas Mermaid devem ser renderizados.
- **PREVIEW-006:** Erros em um diagrama Mermaid devem ser isolados e não impedir a renderização do restante do documento.
- **PREVIEW-007:** HTML embutido deve ser sanitizado. Scripts e execução arbitrária de conteúdo não são permitidos.
- **PREVIEW-008:** Imagens relativas devem ser resolvidas a partir do diretório do arquivo Markdown.
- **PREVIEW-009:** Recursos remotos podem ser carregados apenas conforme a política de privacidade e segurança definida para a implementação.
- **PREVIEW-010:** O Preview deve oferecer tipografia legível e largura de texto confortável, sem modificar o arquivo.

### Exemplo de diagrama Mermaid

O Preview deve renderizar este fluxo, usado também como caso de validação visual:

```mermaid
flowchart LR
    A[Coding agent] --> B[Cria ou atualiza Markdown]
    B --> C[Watcher local]
    C --> D[Construct]
    D --> E[Preview, Edit, Review, Source ou Diff Git]
```

### 10.9 Links

- **LINK-001:** Links relativos para arquivos Markdown existentes dentro de um Local devem abrir dentro do aplicativo.
- **LINK-002:** Links com fragmentos devem tentar navegar até o título ou âncora correspondente.
- **LINK-003:** Links externos `http` e `https` devem abrir no navegador padrão.
- **LINK-004:** Links para arquivos locais não suportados podem ser revelados no Finder ou abertos no aplicativo padrão, após uma ação explícita do usuário.
- **LINK-005:** Links quebrados devem produzir feedback discreto e acionável.
- **LINK-006:** O aplicativo não deve executar esquemas de URL desconhecidos sem confirmação.

### 10.10 Monitoramento do sistema de arquivos

- **WATCH-001:** Cada Local disponível deve ser monitorado para criação, alteração, renomeação e remoção.
- **WATCH-002:** Eventos em rajada devem ser agrupados para evitar duplicidade e instabilidade visual.
- **WATCH-003:** Um arquivo novo suportado aparece na árvore e no Histórico sem atualização manual.
- **WATCH-004:** Um arquivo removido desaparece da árvore e gera evento no Histórico.
- **WATCH-005:** Um arquivo renomeado deve ser tratado preferencialmente como renomeação; quando o sistema não oferecer evidência suficiente, pode ser registrado como remoção e criação.
- **WATCH-006:** Se um arquivo aberto e sem mudanças locais for alterado externamente, seu conteúdo e Preview são atualizados automaticamente.
- **WATCH-007:** A atualização automática deve preservar a posição de leitura tanto quanto possível.
- **WATCH-008:** Se um arquivo aberto tiver mudanças locais não salvas, uma alteração externa não pode substituir o buffer.
- **WATCH-009:** No conflito acima, a aba exibe um aviso persistente com as ações Recarregar versão externa e Manter minhas alterações.
- **WATCH-010:** Recarregar descarta o buffer somente após confirmação explícita.
- **WATCH-011:** Manter minhas alterações conserva o buffer; um salvamento posterior deve exigir confirmação de que substituirá a versão externa.
- **WATCH-012:** Comparação visual de conflito fora do Git não faz parte do preview.
- **WATCH-013:** Se um arquivo aberto e limpo for removido, a aba é fechada automaticamente e o usuário recebe feedback discreto.
- **WATCH-014:** Se um arquivo aberto e modificado for removido, a aba permanece aberta e sinaliza que o caminho deixou de existir.
- **WATCH-015:** O usuário pode salvar novamente um arquivo removido no mesmo caminho, desde que o Local esteja disponível e permita escrita.

### 10.11 Histórico

- **HIST-001:** O Histórico agrega eventos de todos os Locais cadastrados.
- **HIST-002:** Os tipos iniciais são criado, modificado, renomeado e removido.
- **HIST-003:** Cada evento contém Local, caminho relativo, tipo e horário conhecido ou detectado.
- **HIST-004:** Eventos são agrupados ou ordenados cronologicamente, com os mais recentes primeiro.
- **HIST-005:** O Histórico persiste entre execuções.
- **HIST-006:** O período padrão de retenção é de 30 dias.
- **HIST-007:** Eventos mais antigos podem ser removidos automaticamente sem afetar arquivos.
- **HIST-008:** Deve existir uma ação para limpar o Histórico, com confirmação.
- **HIST-009:** O Histórico deve manter uma única entrada por arquivo durante a janela de retenção, substituindo seu estado, tipo e horário pelos do evento mais recente.
- **HIST-010:** Um evento de arquivo existente pode ser selecionado para navegar até o documento.
- **HIST-011:** Um evento de arquivo removido permanece visível até expirar, mas indica que o arquivo não está mais disponível.
- **HIST-012:** O Histórico guarda metadados de eventos, não snapshots ou versões do conteúdo.
- **HIST-013:** Na inicialização, o aplicativo deve comparar o estado conhecido com o estado atual e registrar mudanças detectáveis ocorridas enquanto estava fechado.
- **HIST-014:** Eventos detectados após tempo offline podem usar o horário de detecção quando o horário exato não estiver disponível, deixando isso claro na interface quando necessário.
- **HIST-015:** Ações realizadas pelo próprio aplicativo também devem aparecer no Histórico, sem duplicidade causada pelo watcher.
- **HIST-016:** Renomes devem preservar a identidade histórica do arquivo, evitando que o caminho anterior e o atual apareçam como duas entradas independentes.

### 10.12 Integração Git e diff

- **GIT-001:** O aplicativo deve descobrir se o arquivo pertence a um worktree Git.
- **GIT-002:** A integração é estritamente somente leitura.
- **GIT-003:** O aplicativo nunca deve executar automaticamente `add`, `commit`, `checkout`, `reset`, `clean`, `stash` ou equivalentes.
- **GIT-004:** O Diff Git compara o conteúdo atual do arquivo com `HEAD`, incluindo mudanças staged e unstaged.
- **GIT-005:** Se a aba possuir mudanças locais não salvas, o Diff Git deve comparar o buffer atual com `HEAD` quando tecnicamente viável e indicar que inclui conteúdo não salvo.
- **GIT-006:** Arquivos não rastreados devem aparecer como adição integral.
- **GIT-007:** Em um repositório sem `HEAD`, um arquivo deve aparecer como adição integral quando possível.
- **GIT-008:** Arquivos fora de Git não exibem ação de diff.
- **GIT-009:** Ausência do executável Git, repositório inválido ou falha de leitura deve resultar em estado explicativo, sem bloquear leitura e edição.
- **GIT-010:** O diff deve distinguir adições e remoções e oferecer numeração de linhas suficiente para revisão.
- **GIT-011:** A interface pode oferecer diff unificado ou lado a lado; a decisão visual final será tomada no design detalhado.
- **GIT-012:** O status Git pode ser exibido na árvore e nas abas por indicadores para modificado, adicionado, renomeado e não rastreado.
- **GIT-013:** O aplicativo deve atualizar status e diff após mudanças no arquivo ou no estado relevante do repositório.
- **GIT-014:** Submódulos e worktrees devem ser tratados como repositórios próprios quando o Git assim os reconhecer.

### 10.13 Localização de arquivos e busca de conhecimento

- **SEARCH-001:** `⌘P` abre o localizador de arquivos.
- **SEARCH-002:** A busca considera nome e caminho relativo.
- **SEARCH-003:** A correspondência deve tolerar digitação parcial e, preferencialmente, usar fuzzy matching.
- **SEARCH-004:** Por padrão, a busca considera todos os Locais disponíveis.
- **SEARCH-005:** Cada resultado exibe nome, caminho relativo e Local.
- **SEARCH-006:** `↑` e `↓` movem a seleção no localizador; `Enter` abre o resultado selecionado no Painel ativo.
- **SEARCH-007:** `⌘⇧F` abre ou foca um workspace dedicado de busca de conhecimento, sem substituir `⌘P`.
- **SEARCH-008:** A busca de conhecimento consulta corpo Markdown salvo, título, descrição, tipo, tags, headings, caminho relativo e demais valores de frontmatter indexáveis.
- **SEARCH-009:** Quando aberta a partir de um Local ativo, a busca usa inicialmente apenas esse Local.
- **SEARCH-010:** O escopo é um multiselect visível e pode conter um, vários ou todos os Locais disponíveis.
- **SEARCH-011:** Consultas em vários Locais executam fan-out sobre índices fisicamente isolados e combinam rankings locais; não existe índice físico global.
- **SEARCH-012:** Resultados identificam Local e caminho relativo, sem expor caminhos absolutos no contrato normal.
- **SEARCH-013:** Cada resultado contém título, snippet destacado, razão do match, metadados relevantes, geração do índice e findings quando houver.
- **SEARCH-014:** Os filtros visíveis iniciais são Locais, types e tags.
- **SEARCH-015:** Path, papel técnico, findings, status, trust e freshness ficam disponíveis em `More filters`.
- **SEARCH-016:** Múltiplos valores dentro da mesma categoria usam OR; categorias diferentes usam AND.
- **SEARCH-017:** `status`, trust derivado de `verified` e freshness derivado de `stale_after` seguem a semântica oficial do OKF v0.2.
- **SEARCH-018:** A ausência de `status` em um conceito OKF significa stable; ausência de `stale_after` significa freshness não especificada.
- **SEARCH-019:** Documentos sem type continuam pesquisáveis quando nenhum filtro de type estiver aplicado.
- **SEARCH-020:** A busca permanece local; query, snippets e interações não podem ser enviados para autocomplete, analytics ou serviços remotos.
- **SEARCH-021:** O aplicativo pode guardar localmente as 20 buscas submetidas mais recentes, com query, escopo e filtros.
- **SEARCH-022:** A retenção de buscas recentes pode ser limpa e desativada; desativá-la remove as entradas retidas.
- **SEARCH-023:** Resultados podem ser selecionados manualmente durante a sessão de Search.
- **SEARCH-024:** `Copy references` copia título, Local, caminho relativo e razão do match, sem conteúdo ou caminho absoluto.
- **SEARCH-025:** A seleção de Search é efêmera e pode alimentar `Copy context`; coleções persistentes continuam fora deste corte.
- **SEARCH-026:** Cada resultado pode abrir uma lista limitada de links de saída e backlinks diretos, com razão estrutural em texto.
- **SEARCH-027:** Se a execução full-text embedded falhar, ou se o schema necessário estiver comprovadamente ausente ou inconsistente, a busca deve degradar para uma varredura lexical local e paginada da geração ativa, preservando escopo, filtros, ranking, privacidade e os mesmos contratos de resultado. Uma consulta válida com zero candidatos não deve acionar o fallback.

#### 10.13.1 Grafo de retrieval e context packs

- **GRAPH-001:** Links Markdown internos resolvidos devem ser persistidos no índice derivado do próprio Local, incluindo origem, fragmento e range quando disponíveis.
- **GRAPH-002:** Links de saída e backlinks devem continuar consultáveis independentemente dos limites de renderização do Graph visual.
- **GRAPH-003:** A primeira navegação estrutural usa um hop e nunca atravessa automaticamente para outro Local.
- **GRAPH-004:** Cada documento relacionado deve explicar se foi ligado pela origem, se liga de volta à origem ou se a relação é mútua.
- **GRAPH-005:** Links externos, links que escapam da raiz e links encontrados dentro de `construct-review:v1` não entram no grafo de retrieval.
- **CONTEXT-001:** O usuário pode adicionar resultados de Search e documentos relacionados a uma seleção efêmera de contexto.
- **CONTEXT-002:** `Copy context` monta o pacote no núcleo nativo usando apenas corpos salvos presentes nos índices autorizados.
- **CONTEXT-003:** O pacote preserva limites visíveis entre documentos e inclui Local, caminho relativo, título, papel técnico e razão de inclusão.
- **CONTEXT-004:** A primeira implementação usa um orçamento selecionável de caracteres, limitado pelo servidor, e nunca excede esse teto.
- **CONTEXT-005:** Documentos truncados ou omitidos por orçamento, indisponibilidade ou limite de quantidade devem ser reportados explicitamente.
- **CONTEXT-006:** A mesma seleção, corpus salvo, orçamento e versão do indexador devem produzir ordenação estável.
- **CONTEXT-007:** O context pack não gera resposta por LLM, não salva arquivos e não inclui comentários de Review até a entrega explícita da RFC 07.
- **CONTEXT-008:** O orçamento deve primeiro reservar um trecho útil para o maior número possível de documentos selecionados e depois distribuir o saldo proporcionalmente ao conteúdo restante; um documento grande não pode monopolizar o pacote quando outros documentos ainda puderem contribuir.

### 10.14 Persistência do workspace

- **STATE-001:** O aplicativo deve persistir Locais cadastrados e sua ordem.
- **STATE-002:** Deve persistir o Local selecionado.
- **STATE-003:** Deve persistir o layout e as dimensões da janela principal.
- **STATE-004:** Deve persistir dimensões e recolhimento das seções da sidebar.
- **STATE-005:** Deve persistir o layout dos Painéis.
- **STATE-006:** Deve persistir abas abertas, sua ordem, Painel e modo Preview, Edit, Review, Source ou Diff.
- **STATE-007:** Deve persistir a aba ativa de cada Painel.
- **STATE-008:** Deve restaurar apenas referências a arquivos; conteúdo não salvo não precisa sobreviver ao encerramento normal no preview.
- **STATE-009:** Ao encerrar com alterações não salvas, o aplicativo deve pedir que o usuário salve, descarte ou cancele.
- **STATE-010:** Arquivos ausentes durante a restauração devem ser ignorados ou apresentados como indisponíveis sem impedir a abertura do aplicativo.
- **STATE-011:** Dados de estado corrompidos devem ser recuperados com defaults seguros, preservando os arquivos do usuário.
- **STATE-012:** A inicialização deve liberar o workspace após restaurar estado, watchers e abas; reconciliação, inspeção OKF e indexação de todos os Locais continuam progressivamente em segundo plano.

### 10.15 Atalhos iniciais

Os atalhos abaixo usam a notação do macOS. A implementação futura deve mapear equivalentes nas demais plataformas.

| Ação | Atalho |
| --- | --- |
| Localizar arquivo | `⌘P` |
| Buscar conhecimento | `⌘⇧F` |
| Salvar arquivo | `⌘S` |
| Fechar aba | `⌘W` |
| Localizar dentro do arquivo | `⌘F` |
| Alternar Preview/Edit/Review/Source | A definir |
| Dividir verticalmente | A definir |
| Dividir horizontalmente | A definir |
| Focar próximo Painel | A definir |
| Reabrir aba fechada | Futuro |

### 10.16 Open Knowledge Format (OKF)

O aplicativo deve oferecer consumo não destrutivo e tolerante do [Open Knowledge Format v0.1 e v0.2](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md), uma especificação aberta baseada em arquivos Markdown com frontmatter YAML. Versões futuras permanecem legíveis em modo de compatibilidade.

- **OKF-001:** Um Local cujo `index.md` da raiz declare `okf_version`, ou que combine um `index.md` na raiz com conceitos contendo `type`, deve ser reconhecido automaticamente como bundle OKF. Conceitos de exemplo aninhados, sem esse sinal estrutural, não bastam para classificar o Local inteiro. A marcação explícita do usuário tem precedência sobre a detecção.
- **OKF-002:** Em um Local marcado, o aplicativo deve reconhecer `index.md` como índice de diretório e `log.md` como histórico de atualizações, inclusive em subdiretórios.
- **OKF-003:** Conceitos OKF devem expor, quando presentes, `type`, `title`, `description`, `resource`, `tags`, `timestamp`, `okf_version`, `sources`, `generated`, `verified`, `status` e `stale_after`.
- **OKF-004:** O Preview deve resolver links internos iniciados com `/` em relação à raiz do bundle OKF. Links relativos mantêm o comportamento Markdown usual.
- **OKF-005:** O aplicativo deve indicar de forma não bloqueante se um conceito não contém frontmatter, não possui o campo obrigatório `type` ou contém frontmatter incompleto.
- **OKF-006:** Tipos desconhecidos, campos adicionais, links quebrados e ausência de arquivos de índice não devem impedir a abertura ou leitura do bundle.
- **OKF-007:** A interface deve oferecer um inspector recolhível com os metadados e o status de conformidade do documento aberto.
- **OKF-008:** O suporte inicial é apenas de leitura e validação; não deve criar, reescrever ou completar índices, logs ou metadados automaticamente.
- **OKF-009:** Para cada bundle OKF aberto, o aplicativo deve construir localmente um índice derivado de conceitos, types, tags e links Markdown entre conceitos.
- **OKF-010:** A ação Explore deve permitir navegar pelos types e tags presentes no bundle e abrir a coleção filtrada de conceitos correspondente.
- **OKF-011:** O inspector de um conceito deve apresentar links de saída e referências de entrada conhecidos no bundle.
- **OKF-012:** O índice semântico é um cache derivado, não uma fonte de verdade; deve ser atualizado quando arquivos do bundle forem alterados e nunca deve alterar conteúdo por conta própria.
- **OKF-013:** Explore deve oferecer visões List e Graph sobre o mesmo conjunto filtrado de conceitos.
- **OKF-014:** Graph deve representar conceitos como nós e links Markdown internos conhecidos como arestas, usando `type` como agrupamento visual sem impor uma taxonomia fechada.
- **OKF-015:** O usuário deve poder navegar, aplicar zoom, selecionar um nó para consultar seus metadados essenciais e abrir o documento correspondente.
- **OKF-016:** Em bundles grandes, Graph pode limitar a renderização aos conceitos mais conectados, desde que informe claramente quantos conceitos foram omitidos e preserve o índice completo nas demais visões.
- **OKF-017:** O filtro por `type` deve aceitar seleção múltipla. Cada clique liga ou desliga apenas o `type` correspondente; os conceitos visíveis pertencem a qualquer um dos types selecionados e continuam sujeitos ao filtro de tag quando ele estiver ativo.
- **OKF-018:** Cada `type` presente em um bundle deve receber uma cor visual distinta e estável enquanto o bundle estiver aberto. A mesma cor deve identificar o `type` nos filtros, na lista, nos nós e na legenda do Graph, sem atribuir significado taxonômico à paleta.
- **OKF-019:** Uma única implementação nativa deve produzir a inspeção consumida pelo inspector, pela detecção do bundle, por Explore e por Graph. O frontend não deve manter um segundo parser de YAML.
- **OKF-020:** O parser deve preservar valores YAML desconhecidos como valores tipados — null, booleano, número, string, sequência ou mapa — sem convertê-los silenciosamente em texto.
- **OKF-021:** Em OKF v0.2, `generated.at` é a data preferida para apresentação e ordenação; `timestamp` permanece como fallback compatível, e ambos os valores originais são preservados.
- **OKF-022:** Diagnósticos devem ter códigos estáveis, severidade, mensagem em inglês, identidade relativa do documento e range de origem quando disponível. Erros de conformidade não podem impedir que o Markdown legível seja aberto ou editado.
- **OKF-023:** Links Markdown inline e de referência e campos OKF com contrato de path conhecido devem compartilhar resolução segura. Links quebrados geram findings; links que escapam da raiz registrada nunca entram no grafo.
- **OKF-024:** A inspeção deve impor limites locais de tamanho e profundidade, excluir o bloco `construct-review:v1` de links e nunca reserializar ou salvar um documento.
- **OKF-025:** O executável deve oferecer `construct okf lint [PATH]` para validar qualquer diretório explicitamente indicado sem exigir Local registrado, desktop, serviço, MCP, workspace ou índice.
- **OKF-026:** O linter deve reutilizar o parser Rust compartilhado, operar somente em memória, nunca alterar arquivos e nunca criar estado persistente do Construct.
- **OKF-027:** A saída de texto deve usar severidade, código estável, caminho relativo, range quando disponível, mensagem e resumo determinístico. A saída JSON deve ser um único objeto versionado em stdout.
- **OKF-028:** Os códigos de saída públicos são `0` para scan concluído sem finding no threshold, `1` para lint failure e `2` para erro de invocação ou runtime. O threshold inicial pode ser `error`, `warning` ou `never`.
- **OKF-029:** O linter deve aplicar exclusões de traversal para dependências e builds, aceitar `--exclude` repetível como regra de conformidade e não seguir symlinks. Um Markdown omitido da conformidade permanece resolvível como destino de links internos.
- **OKF-030:** `--max-findings` limita apenas findings materializados na saída; contagens, scan completo e exit code continuam considerando todos os findings. O default inicial é 1.000, com máximo configurável de 100.000.
- **OKF-031:** O primeiro corte não inclui profile, SARIF, cache, watch ou reparo automático. Links quebrados e convenções de `index.md`/`log.md` permanecem findings não bloqueantes com o threshold padrão.
- **OKF-032:** Explore deve oferecer uma visão Health ao lado de List e Graph, reutilizando os findings produzidos pelo mesmo parser Rust do inspector e do CLI, sem executar um segundo parser ou subprocesso.
- **OKF-033:** Health deve resumir errors, warnings, infos e documentos inspecionados, permitir filtro por severidade, path, código ou mensagem e agrupar findings por código estável.
- **OKF-034:** O escopo padrão `Repository policy` deve aplicar apenas exclusões declaradas pelo próprio repositório em `.constructignore`, informar documentos/findings omitidos e permitir recuperar a inspeção estrita em `All Markdown`. `AGENTS.md`, `CLAUDE.md`, `SKILL.md` ou qualquer outro nome não são exceções universais implícitas da OKF.
- **OKF-035:** O usuário deve poder executar novamente o lint sobre os arquivos salvos, abrir o documento afetado em Source e copiar um handoff autocontido para um agente. Essas ações não podem reparar, salvar ou reserializar arquivos automaticamente.
- **OKF-036:** Um bundle pode declarar em `.constructignore` padrões versionáveis para arquivos Markdown que não são conceitos OKF. Linhas vazias e comentários são permitidos, padrões são avaliados em ordem e `!` pode reintroduzir um caminho.
- **OKF-037:** `.constructignore` e `--exclude` compõem a mesma política de conformidade: documentos correspondentes não produzem findings próprios nem aparecem como conceitos OKF, mas continuam disponíveis no filesystem, na busca Markdown geral e na resolução de links. `--no-ignore-file` deve oferecer auditoria estrita no CLI.
- **OKF-038:** O linter deve possuir um build `okf-cli` independente das dependências de desktop, índice e MCP, reutilizando os mesmos módulos de parser, política, relatório e códigos de saída. Releases devem oferecer esse build para Linux x64.
- **OKF-039:** A integração oficial com GitHub Actions deve ser apenas uma embalagem versionada sobre o CLI: selecionar o artefato da plataforma, validar `SHA256SUMS`, usar cache, executar JSON, apresentar summary/annotations e propagar o exit code sem duplicar regras de conformidade.
- **OKF-040:** Explore deve exibir inicialmente as 20 tags mais frequentes, oferecer expansão e recolhimento da lista completa e manter visível uma tag ativa mesmo quando ela estiver fora desse recorte.
- **OKF-041:** Os filtros de `type` e tag no Explore devem ser ordenados por frequência decrescente e usar o nome em ordem alfabética como desempate determinístico.
- **OKF-042:** A área visual do Graph deve crescer para ocupar o espaço vertical disponível no Explore e acompanhar o redimensionamento da janela, preservando uma altura mínima utilizável e scroll quando a janela não comportar todo o conteúdo.

### 10.17 Índice local derivado

O aplicativo mantém uma fundação local de retrieval para todo Markdown salvo,
independentemente de o Local ser OKF. Este índice ainda não substitui o
localizador por nome e caminho; ele alimenta a busca de conhecimento dedicada,
relações diretas, context packs e o acesso MCP.

- **INDEX-001:** Cada Local deve possuir um banco embedded fisicamente separado, identificado pelo ID estável do Local e armazenado no diretório privado de dados do aplicativo.
- **INDEX-002:** Arquivos Markdown continuam sendo a única fonte de verdade. O índice é descartável, reconstruível e nunca pode salvar, reserializar ou alterar um documento.
- **INDEX-003:** O índice deve armazenar corpo Markdown visível completo, headings, frontmatter tipado completo, metadados normalizados e uma projeção limpa para busca.
- **INDEX-004:** Blocos `construct-review:v1`, HTML de Preview, saída renderizada de Mermaid e buffers ainda não salvos não entram na projeção normal de busca.
- **INDEX-005:** Criação, alteração e remoção de arquivos devem atualizar apenas os registros afetados durante a reconciliação comum. Rebuilds completos usam uma nova geração e só a ativam depois de concluídos.
- **INDEX-006:** O último índice saudável deve permanecer utilizável quando um Local estiver temporariamente indisponível ou durante um rebuild.
- **INDEX-007:** Estados públicos do índice são `notIndexed`, `indexing`, `ready`, `degraded` e `failed`; falhas do índice nunca podem impedir leitura ou edição direta dos arquivos.
- **INDEX-008:** A sidebar deve comunicar o estado do índice por Local e permitir rebuild explícito, esclarecendo que nenhum arquivo do projeto será alterado.
- **INDEX-009:** Remover um Local deve apagar seu índice derivado por padrão sem apagar seus arquivos.
- **INDEX-010:** Um único `IndexService` nativo deve possuir todas as conexões embedded. React e futuros adaptadores CLI/MCP acessam contratos tipados e nunca abrem o banco diretamente.
- **INDEX-011:** Consultas estruturadas normais identificam resultados por ID do Local e caminho relativo; caminhos absolutos permanecem no limite nativo.
- **INDEX-012:** Nenhum conteúdo, caminho, query ou métrica do índice pode ser enviado a serviços remotos.
- **INDEX-013:** O índice de cada Local deve manter seus próprios links derivados; nenhuma tabela ou consulta pode misturar relações de Locais diferentes.
- **INDEX-014:** Reconciliações incrementais comuns devem manter o último índice saudável publicamente `ready` ou `degraded`; `indexing` fica reservado para builds sem geração ativa e rebuilds explícitos com `buildingGeneration`.
- **INDEX-015:** O índice deve permitir enumeração completa e determinística de documentos por metadados estruturados e prefixo de caminho, sem consulta textual, com paginação por cursor opaco vinculado à geração ativa.

### 10.18 Acesso local para agentes

- **AGENT-001:** O desktop e adaptadores de agentes devem usar um único serviço local como proprietário exclusivo das conexões embedded por Local.
- **AGENT-002:** O mesmo executável pode operar nos modos desktop, serviço local e MCP stdio; o serviço deve continuar disponível quando a janela desktop estiver fechada.
- **AGENT-003:** O MCP inicial é somente leitura para arquivos fonte e não expõe create, edit, save, delete, move, rename, shell, Git, SQL, banco bruto ou leitura arbitrária do filesystem.
- **AGENT-004:** Cada execução MCP exige allowlist explícita de IDs de Locais registrados. Respostas normais usam ID do Local e caminho relativo, nunca caminho absoluto.
- **AGENT-005:** O transporte entre adapters e serviço usa IPC local autenticado e protegido pelas permissões do usuário. Nenhum listener de rede deve ser aberto e nenhum conteúdo deve ser enviado a serviços remotos.
- **AGENT-006:** O contrato MCP expõe listagem de Locais, overview, activity, enumeração determinística de documentos, busca, leitura de documento, documentos relacionados, context pack e status do índice.
- **AGENT-007:** Cada Local mantém um cache derivado diário de 15 dias com contadores separados para mudanças reais, leituras servidas e inclusão em context packs. Hits de busca e rebuilds não contam como atividade.
- **AGENT-008:** O overview deve combinar contagens por type, tag e role, saúde de links, findings, atividade recente e entradas recentes dos `log.md` reservados pelo OKF, inclusive em scopes aninhados.
- **AGENT-009:** Review comments permanecem fora do contrato MCP até RFC 07 e nunca são misturados silenciosamente ao conteúdo fonte.
- **AGENT-010:** A interface deve oferecer um ponto global para copiar uma configuração MCP pronta, exigindo escolha explícita entre o Local atual, um conjunto de Locais ou todos os Locais, e explicar que o cliente externo controla o destino do conteúdo recuperado.
- **AGENT-011:** Falhas de tools MCP devem manter `isError`, texto legível e um erro estruturado com código estável e mensagem, incluindo a rejeição de Locais fora da allowlist.
- **AGENT-012:** Reconciliações periódicas solicitadas por múltiplos clientes MCP devem ser coalescidas pelo serviço local por Local para evitar varreduras duplicadas, preservando uma única autoridade sobre o índice.
- **AGENT-013:** O serviço usa socket Unix no macOS/Unix e named pipe no Windows, sempre com token por profile, sem listener de rede.
- **AGENT-014:** No Windows, uma invocação desktop deve se desacoplar do console
  antes de iniciar a interface, enquanto `okf`, `service` e `mcp serve`
  permanecem anexados ao console com stdin, stdout, stderr e códigos de saída
  preservados.
- **AGENT-015:** `construct_list_documents` deve enumerar um Local sem query textual, filtrar por role, type, status, tags e prefixo de caminho, ordenar por caminho relativo e rejeitar cursores incompatíveis com filtros ou geração ativa.

### 10.19 Handoff para terminal externo

- **TERM-001:** O usuário pode abrir explicitamente um terminal suportado na
  raiz do Local selecionado.
- **TERM-002:** O usuário pode abrir o terminal na pasta que contém o documento
  ativo por toolbar ou menu de contexto.
- **TERM-003:** Quando houver mais de um terminal suportado instalado, a
  interface permite escolher um aplicativo e preserva essa preferência.
- **TERM-004:** O frontend envia apenas ID do Local, diretório relativo e ID de
  um adaptador conhecido; não pode enviar executável ou comando arbitrário.
- **TERM-005:** O núcleo nativo resolve e canonicaliza o diretório e rejeita
  caminhos absolutos, `..`, links simbólicos que escapem e qualquer destino
  fora do Local registrado.
- **TERM-006:** O handoff não envia conteúdo do documento, não inicia um coding
  agent e não observa comandos, histórico, output ou processos do terminal.
- **TERM-007:** A operação não é exposta por MCP, Markdown renderizado, Review,
  busca ou context packs.
- **TERM-008:** macOS suporta Apple Terminal, iTerm2, Ghostty, WezTerm e Warp
  quando instalados. Windows oferece o host de console padrão do sistema e
  suporta Windows Terminal e Warp quando disponíveis. Linux desktop permanece
  fora do escopo atual.
- **TERM-009:** Falha ou remoção do aplicativo preferido produz erro recuperável
  em inglês e permite escolher outro aplicativo instalado.
- **TERM-010:** Abrir o terminal não modifica documentos, Histórico, índice,
  Review, Git ou estado de salvamento.

### 10.20 Abertura do Construct pelo terminal

- **OPEN-001:** `construct .` e `construct <diretório>` abrem o desktop,
  cadastram o diretório canônico como Local quando necessário e o selecionam.
- **OPEN-002:** `construct <arquivo.md>` abre o arquivo em Edit. O aplicativo
  reutiliza o Local registrado mais específico que contém o arquivo ou
  cadastra seu diretório pai como Local quando nenhum existente o contém.
- **OPEN-003:** Se o arquivo já estiver aberto, inclusive em outro Painel, a
  invocação ativa a aba existente e preserva seu buffer e mudanças não salvas.
- **OPEN-004:** A invocação desktop aceita no máximo um caminho existente e
  somente diretórios ou arquivos `.md`/`.markdown`. Caminhos relativos são
  resolvidos a partir do diretório de trabalho do processo e falhas terminam
  com mensagem em inglês e código 2.
- **OPEN-005:** Construct mantém uma única instância desktop. Invocações
  posteriores mostram e focam a janela existente e entregam a solicitação por
  uma fila nativa drenada somente após a restauração do workspace.
- **OPEN-006:** A ausência de caminho mantém a abertura normal do workspace.
  Os namespaces `okf`, `service` e `mcp serve` continuam com seus contratos de
  console e não são interpretados como caminhos desktop.
- **OPEN-007:** Settings permite instalar de forma idempotente o launcher fixo
  `construct` em um diretório padrão. O aplicativo não aceita destino ou
  executável arbitrário, não substitui arquivos conflitantes e informa quando
  o fallback `~/.local/bin` exige configuração de `PATH`. A instalação
  automática é oferecida somente em macOS/Unix; no Windows, Settings orienta o
  usuário a colocar `construct.exe` no `PATH` manualmente.

## 11. Estados e tratamento de erros

### 11.1 Estado vazio inicial

Quando não houver Locais, a interface deve explicar a proposta em uma frase e destacar a ação **Adicionar pasta**. Não deve parecer um erro.

### 11.2 Local vazio

Se um Local não contiver Markdown suportado, a árvore deve informar que nenhum arquivo foi encontrado, sem esconder o Local.

### 11.3 Local indisponível

Possíveis causas incluem disco externo desconectado, pasta movida, volume de rede ausente ou permissão revogada. A interface deve:

- manter o cadastro;
- mostrar a causa conhecida;
- permitir localizar novamente a pasta;
- tentar retomar automaticamente quando apropriado;
- nunca remover o Histórico associado apenas por indisponibilidade.

### 11.4 Falta de permissão

O aplicativo deve explicar qual pasta não pôde ser lida ou escrita e oferecer uma maneira de conceder novamente a permissão. Falhas em um Local não podem inutilizar os demais.

### 11.5 Arquivo muito grande

O produto deve tentar abrir Markdown grande sem bloquear a interface. Acima de um limite definido durante a implementação, pode avisar que Preview, Mermaid ou highlighting serão reduzidos para preservar desempenho.

### 11.6 Conteúdo inválido

Markdown malformado deve ser exibido da melhor forma possível. Falhas em extensões específicas, como Mermaid, não podem impedir acesso ao Source.

### 11.7 Falha do watcher

Se o monitoramento ficar indisponível, o aplicativo deve informar que atualizações automáticas podem estar atrasadas e oferecer nova varredura, mantendo leitura e edição quando possível.

## 12. Modelo conceitual de dados

Este modelo orienta o comportamento e não determina banco de dados ou tecnologia.

### 12.1 Local

- identificador interno estável;
- caminho ou referência persistente autorizada pelo sistema;
- nome de exibição;
- ordem;
- disponibilidade e última verificação;
- estado conhecido dos arquivos para reconciliação.

### 12.2 Documento aberto

- Local e caminho relativo;
- Painel e posição da aba;
- modo Preview, Edit, Review, Source ou Diff;
- buffer local;
- estado limpo, modificado, em conflito ou removido;
- assinatura conhecida da versão no disco;
- posição de rolagem e seleção relevantes.

### 12.3 Evento de Histórico

- identificador;
- identificador do Local;
- caminho relativo atual e, quando aplicável, anterior;
- tipo de evento;
- instante observado;
- origem conhecida: externa, aplicativo ou reconciliação;
- disponibilidade atual do arquivo.

O Histórico não armazena o conteúdo do arquivo.

### 12.4 Layout

- árvore de divisões horizontais e verticais;
- proporções das divisões;
- Painéis e abas;
- foco atual;
- dimensões da janela e sidebar.

### 12.5 Preferência de terminal

- ID estável de um adaptador de terminal conhecido;
- nenhuma linha de comando, argumento customizado, conteúdo ou caminho;
- ausência de preferência quando o aplicativo escolhido deixa de estar
  disponível.

## 13. Privacidade e segurança

- Todo processamento de arquivos deve acontecer localmente.
- Nenhum conteúdo deve ser enviado a serviços remotos pelo aplicativo.
- Telemetria, se adicionada futuramente, deve ser documentada e nunca incluir conteúdo, nomes de arquivos ou caminhos sem consentimento explícito.
- O Preview deve sanitizar HTML e impedir execução arbitrária de JavaScript.
- Mermaid e highlighting devem operar em ambiente controlado.
- Links externos devem abrir fora do contexto privilegiado do aplicativo.
- Caminhos relativos devem ser normalizados para evitar acesso indevido fora do Local.
- O aplicativo deve seguir o modelo de permissões e sandbox da plataforma quando aplicável.
- Operações Git devem ser somente leitura e limitar seu escopo ao repositório do arquivo.
- Logs de diagnóstico não devem registrar conteúdo de documentos por padrão.
- Logs de diagnóstico devem permanecer locais, usar rotação limitada e poder ser apagados sem afetar arquivos, estado ou índices.
- Logs de busca podem registrar versão, plataforma, geração, contagens agregadas, duração, estado de schema/analyzer, probes sem conteúdo e erros sanitizados.
- Logs de busca não podem registrar texto da consulta, conteúdo, nomes de arquivo, caminhos locais ou valores de frontmatter.
- O launcher de terminal deve aceitar somente adaptadores conhecidos e
  diretórios derivados de Locais registrados.
- O terminal externo recebe apenas o diretório inicial e mantém sua autoridade
  normal de usuário; Construct não o monitora nem o apresenta como sandbox.
- Nenhum documento, agente ou cliente MCP pode acionar o launcher.
- O instalador do comando `construct` pode criar apenas um link simbólico com
  nome e destino definidos pelo núcleo nativo em diretórios de comandos
  revisados; conflitos nunca são sobrescritos.

## 14. Desempenho e confiabilidade

Metas iniciais, sujeitas a validação com protótipos:

- A interface deve permanecer responsiva durante varreduras.
- A primeira lista útil deve aparecer progressivamente, sem esperar a conclusão de toda a varredura.
- Mudanças comuns devem aparecer em até aproximadamente dois segundos após estabilização da escrita.
- A busca por nome deve responder de maneira perceptivelmente instantânea em workspaces com dezenas de milhares de arquivos elegíveis.
- Árvores e listas extensas devem usar renderização virtualizada quando necessário.
- Parsing de Markdown, Mermaid e Git não deve bloquear a thread de interface.
- Escritas devem ser seguras e falhas nunca devem apagar o buffer do usuário.
- O watcher deve tolerar salvamentos atômicos, nos quais editores substituem o arquivo em vez de alterá-lo no lugar.
- A reconciliação na inicialização deve corrigir eventos perdidos sem duplicar excessivamente o Histórico.

Para testes de escala, considerar ao menos:

- 20 Locais cadastrados;
- 50 mil caminhos examinados após exclusões;
- 10 mil arquivos Markdown elegíveis;
- rajadas de centenas de eventos produzidas por operações Git ou agentes;
- arquivos Markdown entre poucos bytes e dezenas de megabytes.

## 15. Acessibilidade

- Todas as ações essenciais devem ser operáveis por teclado.
- O foco ativo deve ser visível em sidebar, abas, Painéis, editor e diálogos.
- Controles devem possuir nomes acessíveis.
- Estados não podem depender exclusivamente de cor.
- Preview e Source devem respeitar escalonamento de texto.
- Contraste deve atender pelo menos WCAG AA quando aplicável.
- A ordem de navegação deve acompanhar a estrutura visual.
- Animações devem respeitar preferências de redução de movimento.

## 16. Portabilidade

O preview principal pode usar integrações nativas do macOS, mas o núcleo deve separar:

- seleção e persistência de acesso a pastas;
- monitoramento do sistema de arquivos;
- abertura de links e revelação no gerenciador de arquivos;
- atalhos e convenções de menu;
- armazenamento de estado;
- execução somente leitura do Git;
- renderização e edição de Markdown.

Convenções específicas devem ser adaptadas por plataforma:

- macOS: Finder, `⌘`, menus nativos e permissões apropriadas;
- Windows: Explorer, `Ctrl`, caminhos e volumes Windows;
- Linux: gerenciador padrão, `Ctrl` e variedade de ambientes desktop.

O produto não deve armazenar caminhos usando suposições exclusivas de separadores ou case sensitivity.

### 16.1 Distribuição

- GitHub Releases é o canal canônico inicial para builds versionados.
- Uma tag semântica `vX.Y.Z` deve corresponder às versões do frontend, lockfile,
  crate Rust e configuração Tauri antes de qualquer bundle ser produzido.
- A mesma tag e commit devem gerar a app e a CLI standalone para macOS Apple
  Silicon, macOS Intel e Windows x64, além da CLI standalone para Linux x64.
- A release deve permanecer em draft até que todos os targets e checksums sejam
  verificados e os smoke tests disponíveis ao mantenedor passem.
- Um preview não assinado que dependa de teste externo pode ser publicado
  somente como pre-release, com limitações explícitas; promoção a release
  estável exige smoke test em máquina limpa e identidade de assinatura
  confiável.
- Instaladores e artefatos CLI devem possuir um manifesto SHA-256; exemplos de
  CI futuros devem fixar uma versão imutável e validar o checksum.
- Um preview não assinado ou ad-hoc signed deve ser identificado explicitamente.
  Uma release pública confiável exige notarização macOS e assinatura Windows.
- Package managers e app stores podem espelhar artefatos futuramente, mas não
  substituem GitHub Releases como origem versionada inicial.

## 17. Onboarding

### Primeira execução

1. Exibir uma breve explicação do produto.
2. Oferecer a ação principal **Adicionar pasta**.
3. Abrir o seletor nativo.
4. Após a escolha, mostrar o Local imediatamente e iniciar descoberta progressiva.
5. Quando houver arquivos, selecionar o Local e permitir que o usuário escolha o primeiro documento.

Não é necessário tutorial em múltiplas etapas no preview. A própria interface
deve ensinar o fluxo, apoiada pelo guia de usuário.

## 18. Escopo atual do preview

### Incluído

- aplicativo macOS;
- aplicativo Windows x64 com workspace Markdown, índice local, linter stateless e MCP;
- cadastro e persistência de múltiplos Locais;
- descoberta recursiva de Markdown;
- exclusões padrão;
- árvore de arquivos com pastas ocultas relevantes;
- busca por nome e caminho;
- abas;
- divisões verticais e horizontais;
- Edit visual com comandos contextuais e salvamento explícito;
- Review com comentários persistentes, cópia para agente e limpeza da rodada;
- Source editável com salvamento explícito;
- Preview com GFM, highlighting e Mermaid;
- links internos;
- monitoramento e resolução segura de conflitos externos;
- Histórico persistente de 30 dias;
- Diff Git somente leitura;
- restauração de workspace;
- atalhos essenciais;
- estados vazios e erros recuperáveis;
- índice local derivado e fisicamente isolado por Local no macOS, Windows e Unix;
- busca de conhecimento em conteúdo e frontmatter, com filtros e fan-out entre
  Locais;
- links diretos, backlinks e context packs manuais com orçamento;
- consumo tolerante de OKF v0.1 e v0.2, inspector, List, Graph e Health;
- `.constructignore` versionável para política de conformidade OKF;
- linter OKF CLI stateless com texto, JSON e thresholds de CI;
- MCP stdio local, read-only e allowlisted no macOS, Windows e Unix;
- overview e atividade local de 15 dias para orientar agentes.
- handoff explícito para um terminal externo suportado na raiz de um Local ou na
  pasta de um documento.

### Adiado

- Linux;
- YAML, JSON, texto, imagens e PDF;
- configuração visual de exclusões globais ou por Local;
- diff e snapshots fora do Git;
- histórico de versões;
- autosave;
- terminal integrado;
- mutação de arquivos por agentes;
- sincronização e colaboração;
- extensões e plugins;
- gerenciamento completo de arquivos;
- múltiplas janelas independentes;
- busca vetorial e embeddings locais opcionais;
- assinatura confiável, notarização e atualização automática.

## 19. Critérios de aceite por jornada

### 19.1 Adicionar e restaurar um Local

- O usuário adiciona uma pasta pelo seletor nativo.
- Os arquivos Markdown aparecem recursivamente.
- Diretórios excluídos não aparecem nem degradam a descoberta.
- Ao reiniciar, o Local continua cadastrado e selecionável.
- Remover o Local do aplicativo não altera a pasta.

### 19.2 Acompanhar mudanças de um agente

- Criar um `.md` externamente faz o arquivo aparecer na árvore e no Histórico.
- Alterar um arquivo aberto e limpo atualiza Preview, Edit e Source.
- Renomear ou remover o arquivo atualiza a navegação sem travar.
- Cada arquivo aparece uma única vez no Histórico, com o tipo e o horário da alteração mais recente.
- Renomear um arquivo preserva uma única entrada para sua identidade atual.

### 19.3 Editar com segurança

- Digitar em Edit ou Source marca a aba como modificada.
- Apenas abrir Edit e retornar a Preview ou Source não modifica o buffer.
- Desfazer uma edição visual até o estado inicial restaura o conteúdo fonte exato e limpa o indicador de modificação.
- Frontmatter YAML permanece idêntico após editar apenas o corpo em Edit.
- Um frontmatter não fechado mantém o conteúdo acessível em Source e não inicia Edit.
- Selecionar um trecho em Review permite registrar uma observação sem alterar o texto citado.
- Comentários continuam presentes ao alternar entre Preview, Edit, Review e Source.
- Copiar para o agente inclui todas as observações abertas e não altera o arquivo.
- Remover a última observação elimina o bloco de review e restaura o conteúdo original.
- `⌘S` grava o conteúdo e limpa o indicador.
- Fechar uma aba modificada pede confirmação.
- Uma falha de escrita mantém o conteúdo no editor.
- Uma mudança externa concorrente nunca substitui silenciosamente o buffer.

### 19.4 Trabalhar com abas e Painéis

- O usuário abre vários arquivos em abas.
- Pode dividir a área vertical e horizontalmente.
- Pode mover uma aba para outro Painel.
- Pode exibir dois documentos lado a lado em modos independentes.
- O layout é restaurado após reiniciar.

### 19.5 Ler Markdown

- Tabelas, checklists, blocos de código e Mermaid são renderizados.
- Um Mermaid inválido não quebra o restante do Preview.
- Links Markdown relativos abrem o arquivo correto no aplicativo.
- Links externos abrem no navegador padrão.
- Conteúdo HTML não pode executar scripts.

### 19.6 Consultar Histórico e Git

- Eventos de todos os Locais aparecem em ordem recente.
- Um arquivo editado repetidamente ocupa apenas uma entrada, atualizada para o evento mais recente.
- Selecionar um evento existente abre o arquivo correto.
- Eventos persistem entre execuções e expiram depois de 30 dias.
- Um arquivo em Git oferece comparação com `HEAD`.
- Um arquivo fora de Git não oferece diff.
- Nenhuma ação Git de escrita é executada pelo aplicativo.

### 19.7 Buscar e montar contexto

- `⌘⇧F` abre a busca de conhecimento sem substituir o localizador `⌘P`.
- O usuário escolhe um ou vários Locais e pode filtrar resultados por metadados
  OKF e caminho.
- Uma consulta entre Locais combina resultados sem criar um índice físico
  global.
- Cada resultado identifica Local, caminho relativo, snippet e razão do match.
- Documentos relacionados mostram links diretos de saída e backlinks.
- `Copy references` não inclui conteúdo ou caminho absoluto.
- `Copy context` usa apenas documentos explicitamente selecionados, preserva
  fronteiras e respeita o orçamento configurado.
- Um documento grande não monopoliza o context pack quando outros selecionados
  ainda podem contribuir.

### 19.8 Inspecionar e validar OKF

- O mesmo bundle produz metadados e findings coerentes no inspector, Explore,
  Health e CLI.
- Health aplica `.constructignore` em `Repository policy` e o ignora em
  `All Markdown`.
- Executar novamente o lint atualiza findings de arquivos salvos sem alterar
  documentos.
- O linter stateless funciona sem desktop, Location, índice ou MCP.
- Texto e JSON usam caminhos relativos e findings determinísticos.
- Os exit codes públicos distinguem sucesso, lint failure e erro de execução.

### 19.9 Conectar um agente local

- A configuração copiada para o MCP aponta ao executável e profile atuais.
- O usuário escolhe explicitamente o Local atual, um conjunto de Locais ou
  todos os Locais; a interface nunca amplia o acesso silenciosamente.
- O servidor recusa iniciar sem allowlist explícita ou `--allow-all`.
- O agente pode consultar Locations, overview, activity, enumeração
  determinística de documentos, busca, documento, relações, context pack e
  status do índice.
- Respostas normais não expõem caminhos absolutos.
- O MCP não oferece mutação de arquivos, Git write, shell, SQL ou leitura
  arbitrária do filesystem.
- Mudanças, leituras servidas e inclusão em context packs permanecem contadores
  separados na atividade local.

### 19.10 Continuar o trabalho em um terminal

- A ação do cabeçalho abre o Local selecionado no terminal preferido.
- A toolbar e os menus de contexto abrem a pasta do documento, não o arquivo.
- Na primeira ação com múltiplos terminais instalados, o usuário escolhe qual
  usar e a escolha é restaurada após reiniciar.
- Trocar a preferência não executa um comando nem abre um terminal
  silenciosamente.
- Caminhos com espaços, acentos e Unicode chegam intactos ao aplicativo.
- Um caminho fora do Local é rejeitado antes de iniciar qualquer processo.
- O terminal não recebe conteúdo do documento nem um comando de agente.
- O MCP continua sem capacidade de shell.

## 20. Estratégia de validação

Antes de considerar o preview estável, validar:

- fluxo real com Codex e outros agentes escrevendo arquivos;
- pastas locais, repositórios Git, worktrees e submódulos;
- pastas ocultas como `.agents` e `.codex`;
- salvamentos feitos por substituição atômica;
- conflitos entre edição local e agente externo;
- remoção e retorno de discos ou pastas;
- projetos pequenos e workspaces grandes;
- Markdown complexo e não confiável;
- navegação integral por teclado;
- reinício e recuperação de estado interrompido.

Automação recomendada:

- testes unitários para filtros, paths, agrupamento de eventos e modelo de layout;
- testes de integração para watcher, persistência, escrita segura e Git;
- testes de renderização para Markdown e sanitização;
- testes end-to-end para as jornadas dos critérios de aceite;
- testes de desempenho com árvores e rajadas sintéticas;
- testes manuais de acessibilidade e integrações nativas no macOS.

## 21. Evolução posterior

Possíveis etapas após o preview atual, sem ordem definitiva:

1. Assinatura confiável, notarização e atualização segura.
2. Suporte a Linux.
3. Visualização e edição de YAML, JSON e texto.
4. Exclusões globais e por Local configuráveis na interface.
5. Quick Look para imagens e PDFs.
6. Histórico de snapshots opcional fora do Git.
7. Diff de conflitos locais.
8. Criação, renomeação e exclusão de arquivos.
9. Múltiplas janelas e workspaces nomeados.
10. Temas e customização avançada do Preview e Edit.
11. Imagens locais em Edit, com inserção e cópia para uma pasta estável do projeto.
12. Busca vetorial local opcional, apenas se avaliação demonstrar ganho sobre
    texto e grafo.
13. Ações contextuais, como copiar Markdown renderizado.

## 22. Decisões ainda abertas

Estas decisões não impedem o preview atual, mas devem ser resolvidas antes de uma distribuição pública estável:

- mecanismo multiplataforma de watcher;
- atalhos para Preview/Edit/Review/Source e divisões;
- política exata para carregamento de imagens remotas;
- estratégia local para inserir, copiar e resolver imagens dentro de Edit;
- limite e degradação controlada para arquivos muito grandes;
- comportamento de sobreposição entre Locais;
- política de assinatura, atualizações automáticas e distribuição no macOS;
- critérios e backend de busca semântica local opcional.

## 23. Registro de decisões

| Data | Decisão |
| --- | --- |
| 2026-07-16 | Começar no macOS sem impedir evolução multiplataforma. |
| 2026-07-16 | Descoberta recursiva; inicialmente apenas Markdown. |
| 2026-07-16 | Source editável e Preview rico, incluindo Mermaid. |
| 2026-07-16 | Estado do aplicativo deve persistir entre execuções. |
| 2026-07-17 | Usar abas e múltiplos Painéis verticais ou horizontais. |
| 2026-07-17 | Não exigir Preview lado a lado com Source; dois arquivos lado a lado são prioritários. |
| 2026-07-17 | Atualizar automaticamente arquivos limpos modificados externamente. |
| 2026-07-17 | Manter Histórico persistente de eventos por 30 dias, sem snapshots. |
| 2026-07-17 | Exibir diff apenas quando o arquivo estiver em Git. |
| 2026-07-17 | Mostrar arquivos ocultos relevantes. |
| 2026-07-17 | Buscar apenas por nome e caminho no MVP. |
| 2026-07-17 | Usar salvamento explícito com `⌘S`; sem autosave. |
| 2026-07-17 | Ignorar diretórios comuns de dependências, caches e builds. |
| 2026-07-18 | Adotar Tauri 2, React 19 e Rust para o aplicativo desktop. |
| 2026-07-18 | Usar CodeMirror como editor Source e React Markdown com Mermaid no Preview. |
| 2026-07-18 | Persistir workspace e Histórico localmente; arquivos continuam sendo a fonte da verdade. |
| 2026-07-18 | Exibir Diff Git unificado e somente leitura. |
| 2026-07-25 | Adotar **Construct** como nome e identidade do produto. |
| 2026-07-25 | Tratar bundles OKF como espaços navegáveis por tipos, tags, links, backlinks, lista e grafo. |
| 2026-07-25 | Manter uma única entrada por arquivo no Histórico, atualizada pelo evento mais recente. |
| 2026-07-25 | Preparar o projeto para colaboração pública com validação automatizada e CI no macOS. |
| 2026-07-26 | Adotar edição visual local com Milkdown/Crepe, mantendo Source como escape hatch e frontmatter YAML byte a byte. |
| 2026-07-26 | Compartilhar um único buffer entre Preview, Edit e Source, preservar salvamento explícito e não oferecer upload de imagens no primeiro corte. |
| 2026-07-26 | Armazenar comentários de Review no próprio Markdown em um bloco invisível `construct-review:v1`, com remoção exata e prompt copiável para novas sessões de agente. |
| 2026-07-26 | Consumir OKF v0.1/v0.2 e versões futuras de forma tolerante por um parser Rust compartilhado, preservando YAML aberto e usando findings estáveis. |
| 2026-07-26 | Adotar um SurrealDB/SurrealKV embedded por Local, possuído por um `IndexService` nativo, com corpo e frontmatter completos, projeção limpa de busca e arquivos como fonte da verdade. |
| 2026-07-26 | Manter a inicialização interativa fora do caminho crítico da reconciliação e indexação completas. |
| 2026-07-26 | Persistir links por Local, oferecer relações diretas explicáveis e montar context packs manuais com orçamento de caracteres antes de qualquer expansão automática ou LLM. |
| 2026-07-26 | Oferecer um linter OKF CLI stateless, determinístico e somente leitura, reutilizando o parser nativo sem depender de Location, índice, serviço ou desktop. |
| 2026-07-27 | Distribuir app e CLI pela mesma tag em GitHub Releases, com DMG macOS, NSIS Windows, arquivos CLI standalone, checksums e publicação inicial em draft. |
| 2026-07-27 | Tornar drafts privados em pre-releases públicas somente depois de verificar targets e checksums; usar o estado pre-release para previews não assinados e smoke tests externos, sem apresentá-los como distribuição estável. |
| 2026-07-27 | No preview Windows, oferecer workspace desktop e linter OKF stateless; manter índice local e MCP como macOS/Unix até existir um transporte IPC Windows autenticado. |
| 2026-07-27 | Habilitar índice local e MCP no Windows com named pipe autenticado por token, sem listener de rede; normalizar identidades canônicas `\\?\` no frontend. |
| 2026-07-27 | Organizar a documentação pública pelas jornadas de usuário, CLI, MCP, desenvolvimento, produto e arquitetura, mantendo README como entrada curta. |
| 2026-07-28 | Isolar o linter em um build CLI-only, distribuí-lo para Linux x64 e oferecer uma GitHub Action fina que verifica o artefato versionado e preserva o contrato do CLI. |
| 2026-07-29 | Oferecer handoff explícito para terminais externos conhecidos a partir de Locais e documentos, sem comandos arbitrários, conteúdo, monitoramento ou acesso por MCP; terminal embutido permanece adiado. |
| 2026-07-30 | Preservar a posição semântica ao alternar modos e tornar comentários de Review bidirecionalmente navegáveis por âncoras opcionais compatíveis com `construct-review:v1`. |
| 2026-07-30 | Tornar as três seções da sidebar redimensionáveis, com recolhimento que libera espaço e scroll independente; mover tema e conexão de agentes para controles globais e exigir escopo MCP explícito. |
| 2026-08-01 | Limitar a visualização inicial de tags no Explore às 20 mais frequentes, com expansão explícita e preservação do filtro ativo. |
| 2026-08-01 | Ordenar os filtros de types e tags no Explore pela quantidade de conceitos, com desempate alfabético. |
| 2026-08-01 | Permitir abrir Locais e Markdown pelo comando `construct <caminho>`, reutilizando uma única instância desktop e oferecendo instalação segura do launcher em Settings. |
| 2026-08-04 | Ampliar o handoff externo com Warp no macOS e Windows e usar o host de console padrão como fallback nativo no Windows. |

## 24. Histórico do documento

| Versão | Data | Alteração |
| --- | --- | --- |
| 0.1 | 2026-07-17 | Primeira especificação consolidada do MVP. |
| 0.2 | 2026-07-25 | Estado do preview, identidade Construct, suporte OKF e decisões técnicas consolidados. |
| 0.3 | 2026-07-26 | Edição visual Markdown, contrato de preservação de frontmatter e critérios de segurança do buffer. |
| 0.4 | 2026-07-26 | Review persistente no documento, ciclo de comentários e handoff copiável para agentes. |
| 0.5 | 2026-07-30 | Continuidade de posição entre modos, âncoras de Review, destaques e navegação bidirecional. |
| 0.5 | 2026-07-26 | Compatibilidade OKF v0.1/v0.2, parser nativo compartilhado, metadados tipados e contrato de findings. |
| 0.6 | 2026-07-26 | Índice local persistente por Local, ownership nativo, gerações, retenção e controles de rebuild. |
| 0.7 | 2026-07-26 | Search dedicado, filtros OKF, federação entre Locais e seleção efêmera de referências. |
| 0.8 | 2026-07-26 | Startup progressivo, links persistidos, related documents e context packs manuais com orçamento. |
| 0.9 | 2026-07-26 | Linter OKF stateless com texto/JSON, thresholds de CI, exclusões e exit codes públicos. |
| 0.10 | 2026-07-26 | MCP local read-only, allowlist por Location, hot memory, overview, activity e tools compartilhadas com o índice. |
| 0.11 | 2026-07-27 | Escopo atual reconciliado com busca, Health, CLI, MCP e preview Windows; documentação pública orientada por jornada. |
| 0.12 | 2026-07-27 | Ciclo de distribuição distinguindo draft privado, pre-release pública não assinada e release estável confiável. |
| 0.13 | 2026-07-27 | Índice local e MCP no Windows via named pipe autenticado, incluindo compatibilidade com caminhos canônicos Windows. |
| 0.14 | 2026-07-28 | Build CLI-only para Linux x64 e GitHub Action versionada sobre o contrato JSON do linter. |
| 0.15 | 2026-07-29 | Handoff seguro para terminal externo por adaptadores conhecidos e diretório relativo validado dentro de um Local. |
| 0.16 | 2026-07-29 | Inicialização desktop no Windows desacoplada do console, preservando console e stdio para CLI, serviço e MCP no executável compartilhado. |
| 0.17 | 2026-08-01 | Abertura de Locais e arquivos pelo terminal, instância desktop única, fila de cold start e instalação segura do launcher. |
| 0.18 | 2026-08-01 | Documentação pública e RFCs reconciliados com a enumeração MCP de documentos, abertura desktop pelo terminal e distribuição preview atual. |
| 0.19 | 2026-08-04 | Detecção de Warp no macOS e Windows, fallback para o host de console padrão no Windows e seletor de terminal consistente entre temas. |

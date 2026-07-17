# Agent Context

Aplicativo desktop local para acompanhar e editar arquivos Markdown produzidos por coding agents.

## O que já funciona

- cadastro persistente de pastas (Locais);
- descoberta recursiva de `.md` e `.markdown`, incluindo diretórios ocultos relevantes;
- exclusão de diretórios de dependências, cache e build;
- monitoramento de mudanças no sistema de arquivos;
- Histórico local de eventos por 30 dias;
- Source com editor Markdown e salvamento explícito;
- Preview CommonMark/GFM com tabelas, checklists, highlighting, Mermaid, links e imagens locais;
- abas, drag-and-drop entre painéis e divisões horizontal/vertical;
- busca por nome/caminho com `⌘P`;
- integração Git somente leitura e diff contra `HEAD`;
- restauração de Locais, histórico, layout, abas e modos de visualização.

## Requisitos de desenvolvimento

- macOS com Xcode Command Line Tools;
- Node.js 22 ou posterior;
- Rust estável, via `rustup`.

## Desenvolvimento

```bash
npm install
npm run dev
```

## Verificações

```bash
npm run check
npm run build:web
cd src-tauri && cargo check
```

## Bundle macOS

```bash
npm run build
```

O bundle `.app` é gerado em `src-tauri/target/release/bundle/macos/Agent Context.app`.

## Documento de produto

A especificação detalhada do MVP está em [docs/product-spec.md](docs/product-spec.md).

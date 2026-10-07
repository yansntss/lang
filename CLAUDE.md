# Tradutor por atalho

App desktop Tauri v2 (Rust + React/TS) para Windows, macOS e Linux.
Objetivo: o usuário está aprendendo inglês (PC em inglês) e quer traduzir rápido.

## Fluxo principal
Atalho global → caixa flutuante perto do cursor → tradução EN↔PT-BR.
- Se houver texto selecionado, capturar e preencher automaticamente.
- Detecção automática: origem EN → PT-BR; qualquer outra → EN-US.
- Esc fecha; Enter copia a tradução.
- App fica na bandeja; janela pré-carregada e oculta.

## Arquitetura
- Tradução: DeepL API, chamada no Rust (reqwest).
- Botão "Explicar": Claude Haiku (gramática, exemplos, uso).
- Chaves de API SOMENTE no backend Rust, guardadas no keychain do SO (crate keyring). Nunca no JS, nunca em log, nunca commitadas.
- Front só chama comandos Tauri via invoke().
- Capabilities do Tauri: permissões mínimas.

## Ambiente (IMPORTANTE)
- Claude Code roda no WSL; o projeto fica em /mnt/c/www/pessoal/lang.
- NUNCA rodar npm, npx ou cargo direto no WSL neste projeto.
- Sempre: powershell.exe -c "cd C:\www\pessoal\lang; <comando>"
  - dev: npm run tauri dev
  - testes Rust: cd src-tauri; cargo test
  - lint Rust: cd src-tauri; cargo clippy -- -D warnings; cargo fmt --check

## Convenções
- Rust: sem unwrap()/expect() fora de testes; erros tipados (thiserror).
- Commits pequenos, um por funcionalidade (conventional commits).
- Plano e progresso em docs/PLAN.md, atualizado ao fim de cada fase.
- Commits e PRs: NUNCA incluir "Co-Authored-By: Claude", "Generated with Claude Code" ou qualquer atribuição ao Claude/IA. O autor é somente o usuário.

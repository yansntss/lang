# Tradutor por atalho

App desktop Tauri v2 (Rust + React/TS).
Escopo atual: **Windows** (decisão do usuário). O código procura continuar compilável em
macOS/Linux, mas não há testes nem suporte nesses sistemas (ex.: a captura de seleção e o
keychain só funcionam no Windows).
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
  - testes do front: npm test; build: npm run build
- `cargo`/`rustc` ficam em C:\Users\yansa\.cargo\bin e NÃO estão no PATH do powershell.exe chamado
  pelo WSL. Prefixar: `$env:PATH += ";C:\Users\yansa\.cargo\bin"`.
- git também deve rodar pelo Windows (`powershell.exe -c "cd C:\www\pessoal\lang; git ..."`): no WSL
  ele falha com "dubious ownership" e com `chmod` em /mnt/c.
- `npm run tauri dev` fica rodando e recompila sozinho a cada alteração em src-tauri. Antes de subir
  outra instância, verifique se o usuário já tem uma (Vite na porta 1420, `lang-app.exe`); não
  encerre a sessão dele sem avisar.
- Testes que tocam o SO real (clipboard, keychain) são `#[ignore]`: `cargo test -- --ignored`.

## Convenções
- Rust: sem unwrap()/expect() fora de testes; erros tipados (thiserror).
- Commits pequenos, um por funcionalidade (conventional commits).
- Plano e progresso em docs/PLAN.md, atualizado ao fim de cada fase.
- Commits e PRs: NUNCA incluir "Co-Authored-By: Claude", "Generated with Claude Code" ou qualquer atribuição ao Claude/IA. O autor é somente o usuário.

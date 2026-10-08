# Tradutor por atalho

App de bandeja para Windows (Tauri v2 + React/TypeScript) que traduz rápido entre inglês e
português do Brasil, feito para quem está aprendendo inglês.

Pressione **`Ctrl+Alt+T`** em qualquer app: abre uma caixa flutuante perto do cursor. Se havia
texto selecionado, ele já vem preenchido e é traduzido na hora. Inglês vira PT-BR; qualquer outro
idioma vira EN-US. **Esc** fecha; **Enter** copia a tradução e fecha.

## Status

Fases 0 a 2 prontas (bandeja, atalho, popup, DeepL, captura da seleção). Próximas: botão
"Explicar" com Claude, histórico, configurações e distribuição. O plano e o estado de cada fase
estão em [`docs/PLAN.md`](docs/PLAN.md).

## Requisitos

- Windows 10/11 com WebView2
- Node.js e Rust (toolchain MSVC)
- Uma chave da API do DeepL (o plano gratuito serve; chaves gratuitas terminam em `:fx`)

## Desenvolvimento

No PowerShell do Windows (não no WSL):

```powershell
cd C:\www\pessoal\lang
npm install
npm run tauri dev
```

Na primeira execução a janela **Configurações** abre sozinha para você colar a chave do DeepL.

```powershell
npm test                                   # testes do front
npm run build                              # tsc + vite
cd src-tauri
cargo test                                 # testes de Rust
cargo test -- --ignored                    # testes que usam o clipboard e o keychain reais
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

## Segurança e privacidade

- As chaves de API ficam **só no backend Rust**, no Gerenciador de Credenciais do Windows. O front
  não consegue lê-las, e elas nunca vão para o log.
- O texto que você traduz é enviado ao DeepL. Isso inclui a seleção capturada pelo atalho.
- Para capturar a seleção, o app simula `Ctrl+C` no app em foco e devolve o clipboard ao que era
  antes. Esse texto pode aparecer, por um instante, no histórico do clipboard do Windows (`Win+V`).
- A captura **nunca** é feita em terminais, clientes SSH, consoles remotos/VM nem em editores com
  terminal embutido (VS Code, IDEs JetBrains), onde `Ctrl+C` interromperia um processo. Nesses
  apps o popup abre vazio.
- Apps rodando como administrador não permitem a captura (limitação do Windows).

## Distribuição

Ainda não há instalador. Quando houver, ele **não será assinado**, e o Windows SmartScreen vai
avisar sobre um "editor desconhecido".

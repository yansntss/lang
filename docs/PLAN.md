# Plano: Tradutor por atalho (Tauri v2)

Atualizado ao fim de cada fase. Marcar `[x]` ao concluir.

## Estado atual (2026-10-08) — retomar daqui
- Fases 0, 1 e 2 concluídas, revisadas, testadas manualmente pelo usuário e commitadas em `main`.
- Fase 3 (Explicar com Claude) **implementada, revisada e testada com mocks**; falta só a
  verificação manual com uma chave real da Anthropic (o usuário tem apenas DeepL Free). Sem
  chave, o botão fica desabilitado, com dica.
- **Próximo: Fase 4 — Histórico e aprendizado.** Planejar antes (`/ecc:plan fase 4`).
- Decisões que o usuário ainda não tomou (detalhes nas notas da Fase 2):
  1. Manter VS Code/Cursor/IDEs JetBrains bloqueados na captura de seleção, ou liberar.
  2. Traduzir a seleção capturada automaticamente (como hoje) ou só após Enter.
- Testes: 150 de Rust (+2 `#[ignore]` que usam o clipboard e o keychain reais:
  `cargo test -- --ignored`) e 40 do front. `clippy -D warnings` e `fmt --check` limpos.
- Armadilhas de ambiente (também em `CLAUDE.md`): `cargo` fora do PATH do `powershell.exe`,
  git só funciona pelo Windows, e `tauri dev` recompila sozinho a cada alteração.

## Decisões de escopo
- Plataforma alvo: **Windows**. Código mantido compilável em macOS/Linux, mas sem testes, sem CI e sem suporte oficial a eles.
- Chaves: apenas **DeepL Free** (`:fx`, endpoint `api-free.deepl.com`). Chave da Anthropic é opcional: sem ela, "Explicar" fica desabilitado.
- Distribuição: **sem assinatura de código** (aviso do SmartScreen documentado no README). Alvo NSIS/MSI.
- Histórico: **ligado por padrão**, com opção de desligar.

## Decisões técnicas
| Tema | Decisão |
|---|---|
| Detecção de idioma | Local com `whatlang`; `detected_source_language` do DeepL como fallback |
| Histórico | SQLite (`rusqlite` bundled), acessado só no Rust |
| Configurações | `tauri-plugin-store` (JSON). Segredos só no keyring |
| Captura de seleção | Salvar clipboard → simular Ctrl+C (`enigo`) → ler (`arboard`) → restaurar |
| Streaming "Explicar" | `tauri::ipc::Channel` + SSE do Messages API |
| HTTP | `reqwest` (`rustls-tls`) atrás de traits (`Translator`, `Explainer`, `SecretStore`) |
| Atalho padrão | `Ctrl+Alt+T`, configurável |

## Estrutura

```
src-tauri/
  capabilities/  popup.json  settings.json
  permissions/
  src/
    main.rs lib.rs error.rs state.rs config.rs secrets.rs
    commands/  translate explain history settings secrets window
    services/  deepl claude lang_detect capture history_db
    platform/  tray shortcut popup_window autostart
  tests/
src/
  windows/ popup/ settings/ history/
  lib/ tauri.ts types.ts
  hooks/ components/ test/
docs/PLAN.md
```

## Comandos Tauri
| Comando | Fase |
|---|---|
| `translate(text)` | 1 |
| `copy_to_clipboard(text)` | 1 |
| `hide_popup()` | 1 |
| `set_secret(kind, value)` / `has_secret(kind)` / `delete_secret(kind)` (sem `get_secret`) | 1 |
| `explain(text, translation, on_event: Channel)` | 3 |
| `list_history` / `delete_history` / `clear_history` / `toggle_favorite` / `export_history` | 4 |
| `get_settings` / `update_settings` / `set_shortcut` | 5 |

Eventos Rust → front: `popup://prefill`, `popup://reset`.

## Plugins e crates
- Plugins: `global-shortcut`, `single-instance`, `clipboard-manager`, `store`, `autostart`, `updater`, `process`, `log`; tray via feature `tray-icon`. Remover `opener`.
- Rust: `reqwest`, `tokio`, `thiserror`, `keyring` v3 (`windows-native`), `whatlang`, `enigo`, `arboard`, `rusqlite`, `futures-util`, `tracing`. Dev: `wiremock`, `tempfile`.
- Front: `vitest`, `@testing-library/react`, `@testing-library/user-event`, `jsdom`.

## Fases
Cada fase termina com: `cargo test`, `clippy -D warnings`, `fmt --check`, testes do front, este arquivo atualizado e commits conventional.

### Fase 0 — Fundação
- [x] Renomear branch para `main`, commit inicial do scaffold
- [x] Remover `opener` e o demo do template; CSP restritivo
- [x] `AppError` (thiserror) e módulo `secrets` com trait + `InMemoryStore`
- [x] vitest configurado; entradas Vite multi-página (`popup.html`, `settings.html`)

Notas: `cargo`/`rustc` ficam em `C:\Users\yansa\.cargo\bin`, fora do PATH do `powershell.exe`
chamado pelo WSL. Prefixar com `$env:PATH += ";C:\Users\yansa\.cargo\bin"`.

### Fase 1 — MVP (bandeja + atalho + janela + DeepL)
- [x] Decisão de idioma + `DeepLClient` com testes `wiremock` (403, 456, 429, 5xx, timeout)
- [x] `KeyringStore`, comandos de segredo, janela `settings` mínima (chave DeepL)
- [x] Janela `popup` pré-carregada e oculta
- [x] Tray (Abrir, Configurações, Sair)
- [x] Atalho global (`Ctrl+Alt+T`) + posicionamento perto do cursor (lógica pura testada)
- [x] Front: tradução, Esc fecha, Enter copia, oculta ao perder foco
- [x] `single-instance`
- [x] Permissões por comando e por janela (`AppManifest` + `capabilities/popup.json`, `settings.json`)

Verificação manual (não automatizável; `npm run tauri dev`). Confirmada pelo usuário em
2026-10-07 ("fase 1 eu testei e está ok"), em relato geral, sem itemizar:
- [x] Tradução real EN→PT e PT→EN com a chave DeepL Free
- [x] Atalho abre o popup perto do cursor, inclusive com 2 monitores e DPI diferente
- [x] Popup recebe foco ao abrir (o Windows pode recusar `set_focus`) e não fecha sozinho logo após abrir
- [x] Esc fecha, Enter copia e fecha, clicar fora oculta, Alt+F4 no popup só oculta
- [x] Segunda execução do app não abre outra instância
- [x] "Configurações" pela bandeja (criar janela dentro do handler do tray pode travar no Windows)
- [x] Atalho repetido com o popup aberto não apaga o texto digitado
- [x] A chave aparece em `cmdkey /list` e não aparece em `%LOCALAPPDATA%\com.yansa.lang-app\logs`

Notas e desvios do plano original:
- Smoke test feito: o app sobe, cria a bandeja e abre sozinho a janela "Configurações" quando
  não há chave. O fluxo atalho → popup → tradução real **não** foi exercitado.
- `whatlang` sem restrição erra inglês comum (detectou "I would like to know…" como afrikaans).
  O detector usa uma allowlist (EN, PT, ES, FR, DE, IT) e só confia em `is_reliable()`. Textos
  curtos viram `Unsure` e usam o `detected_source_language` do DeepL (até 2 chamadas).
- `keyring` fixado na 3.6.3 (a 4.x tem API nova). `reqwest 0.13` usa `rustls` por padrão e
  `json` é feature própria. Plugins Tauri fixados em `2` (a linha `3.x` ainda é alpha).
- Testes do popup usam timers reais: `user-event` trava com fake timers do vitest.
- Mensagens de erro do backend são genéricas; o detalhe técnico fica só no `Debug` (log).

Revisões da Fase 1 (rust-reviewer, security-reviewer, react-reviewer): sem itens críticos ou
altos; chave não chega ao JS, logs ou erros; capabilities mínimas por janela. Corrigido:
popup preso sem foco após `BLUR_GRACE`, fallback de monitor, atalho repetido apagando o texto,
redirects/HTTPS no cliente DeepL, 401 → chave inválida, validação de caracteres na chave,
`Debug` do `InMemoryStore`, CSP (`base-uri`, `form-action`, `object-src`, `frame-ancestors`),
`.env` no `.gitignore`, foco ao reexibir, debounce com `trim`, Enter repetido, Esc em IME.
`npm audit`: 0 vulnerabilidades (`cargo audit` não está instalado).

Pendências conhecidas (fora da Fase 1):
- `keyring` só tem a feature `windows-native`; em macOS/Linux cairia num store em memória.
  Aceito porque o escopo é Windows. Revisar se o escopo mudar.
- Falha de inicialização em release é silenciosa (`windows_subsystem` + `eprintln!`) e
  `panic = "abort"` não descarrega logs. Tratar na Fase 6 (diálogo nativo ou log em arquivo).
- Rodar `cargo audit` no Windows na Fase 6.

### Fase 2 — Captura de texto selecionado
- [x] Portas (`ClipboardPort`, `InputPort`, `Sleeper`) e `capture_selection` testada com fakes
- [x] Aguardar teclas do atalho soltas (modificadores e `T`); polling com timeout; capturar
  antes de mostrar o popup
- [x] Nunca simular Ctrl+C em terminais, SSH, IDEs com terminal e consoles remotos/VM
  (nome do executável + classe da janela; app desconhecido também é bloqueado)
- [x] Clipboard devolvido byte a byte (todos os formatos em memória), sem sobrescrever conteúdo
  que outro app escreveu durante a captura
- [x] Adaptadores Windows (`SendInput`, clipboard Win32, app em foco) em `platform/windows_capture.rs`
- [x] `popup://reset` com `{ prefill }` e tradução automática pelo caminho da digitação
- [x] Trava anti-captura-sobreposta com relógio de segurança de 5 s

Verificação manual (não automatizável; `npm run tauri dev`). Confirmada pelo usuário em
2026-10-07 ("tudo ok nos teste manual"), em relato geral, sem itemizar:
- [x] Selecionar texto no Bloco de Notas, Chrome, Edge e Word: o popup abre com o texto e traduz
- [x] **Terminal:** com um comando rodando no Windows Terminal, o atalho NÃO interrompe o comando
- [x] Clipboard igual ao de antes (texto, imagem e conteúdo copiado de navegador/Word com formatação)
- [x] App como administrador: abre vazio, sem erro
- [x] Segurar o atalho por mais de 1 s: abre vazio, sem tecla presa nem "t" digitado no app
- [x] Apertar o atalho várias vezes seguidas não gera cópias sobrepostas
- [x] O popup mantém o foco depois de uma captura (o `set_focus` roda ~0,5 s após o atalho)
- [x] O log não contém trechos dos textos capturados

Notas e decisões da Fase 2:
- Em vez de `arboard` + `enigo`, usa o crate `windows` 0.62 (o mesmo que o Tauri já puxa):
  `SendInput` com `VK_C` fixo, e snapshot do clipboard por formato. Restaurar só texto/imagem
  perderia a formatação (HTML/RTF) de quem copiou do navegador ou do Word.
- Sem seleção, o popup só abre depois de `COPY_TIMEOUT` (500 ms): é o custo de não haver como
  saber, sem UI Automation, se algo está selecionado.
- Limitação de privacidade: o texto selecionado passa pelo clipboard e, portanto, pode ir para o
  histórico (Win+V) e para a nuvem do Windows. A cópia é feita pelo app de origem e não dá para
  evitar; só a restauração é excluída do histórico. Conteúdo que o app de origem marcou como
  sensível (gerenciadores de senha) nunca é lido. A saída definitiva seria UI Automation
  (`TextPattern`), que não usa o clipboard.
- Limitações: apps rodando como administrador ignoram o `SendInput` (UIPI); apps que demoram mais
  de 500 ms para copiar não são capturados.
- A lista de bloqueio não é exaustiva. Bloqueia por padrão VS Code, Cursor e IDEs JetBrains/Visual
  Studio, porque o terminal embutido recebe Ctrl+C como interrupção e é indistinguível do editor.
  Decisão pendente do usuário: aceitar isso ou permitir esses editores (Fase 5: lista editável).
- Decisão pendente do usuário: traduzir a seleção capturada automaticamente (hoje, como no
  `CLAUDE.md`) ou só após Enter. A revisão de segurança apontou que isso envia o texto ao DeepL
  sem confirmação. Candidato a opção da Fase 5.
- A tecla principal do atalho (`T`) está fixa em `WindowsInput`; ao tornar o atalho configurável
  (Fase 5) ela deve vir da configuração.

Revisões da Fase 2 (rust-reviewer e security-reviewer): sem itens críticos; `unsafe` correto
(sem vazamento, sem liberação dupla, handles sempre fechados). Corrigido: corrida entre snapshot
e cópia (contador lido antes do snapshot e conferido antes de restaurar), espera do contador
assentar, restauração com ~500 ms de tentativas, app em foco rechecado imediatamente antes do
Ctrl+C, lista de bloqueio ampliada com classe da janela, tecla `T` do atalho, teto de memória
por formato (64 MiB) e total (128 MiB), conteúdo sensível, relógio de segurança de 5 s.

### Fase 3 — Explicar com Claude (opcional em runtime)
- [x] `ClaudeClient` + testes SSE com `wiremock`
- [x] Prompt seguro (sem tools, `max_tokens` limitado, texto como dado)
- [x] `explain` com streaming; botão desabilitado sem chave
- [x] `ExplainPanel` renderizando texto puro
- [x] Cancelamento (`cancel_explain`, ao trocar o texto, ao ocultar o popup e ao reexibi-lo)

Verificação manual (pendente: exige chave da Anthropic; `npm run tauri dev`):
- [ ] Cadastrar a chave nas Configurações, traduzir um texto e clicar em "Explicar": o texto
  aparece aos poucos, em português, e o botão volta a habilitar no fim
- [ ] Sem chave: o botão aparece desabilitado, com a dica
- [ ] Esc ou clicar fora durante a explicação interrompe a geração (conferir o consumo no console
  da Anthropic)
- [ ] Teste do `ClaudeClient` contra a API real (não escrito: sem chave para validar)

Notas e decisões da Fase 3:
- Modelo fixo `claude-haiku-5-5` (constante `MODEL`); configurável na Fase 5. `max_tokens` = 800.
  Se a resposta bater no teto, o texto termina com "…". Sem `temperature`, por segurança com
  modelos novos.
- O botão só aparece depois de uma tradução que corresponde ao texto do campo, e o texto só vai
  à Anthropic no clique (nunca automaticamente).
- Streaming: `reqwest` `Response::chunk()` + parser SSE próprio (`services/sse.rs`, testado com
  eventos cortados no meio e UTF-8 partido) e `tauri::ipc::Channel`. Não precisou de `futures-util`.
- Prompt: `system` fixo mandando tratar o conteúdo de `<source_text>`/`<translation>` como dado;
  `& < >` escapados para o texto não fechar a tag; sem `tools`.
- Cancelamento: `ActiveExplain` guarda uma explicação por vez; iniciar outra, `cancel_explain`,
  `hide_popup` ou um canal fechado abortam a tarefa. No front, `useExplain` descarta eventos de
  execuções antigas.
- Mensagens de erro `Network` e `Upstream` ficaram genéricas (antes citavam "tradução").
- A capability do popup ganhou `allow-explain`, `allow-cancel-explain` e `allow-has-secret`
  (só devolve booleano).

Revisões da Fase 3 (rust-reviewer e security-reviewer): sem itens críticos ou altos. Corrigido:
geração continuava depois de ocultar o popup (agora `hide_popup` cancela), canal fechado não
parava a geração, resposta cortada por `max_tokens` parecia completa, `ClaudeClient::build`
(override de URL) agora é privado ao módulo. Aceito/adiado (baixo risco): estouro do buffer SSE
mostra "erro (200)"; evento final sem linha em branco no EOF é descartado; `find_blank_line`
reescaneia o buffer (limitado a 1 MiB); sem intervalo mínimo entre explicações; entrada
residual de `ActiveExplain` se a tarefa termina antes do `register` (inofensiva); a chave é uma
`String` comum, sem `zeroize`.

### Fase 4 — Histórico e aprendizado
- [ ] Schema + migrations (`user_version`), testes com `tempfile`
- [ ] Gravação automática (ligada por padrão), dedup, opção de desligar
- [ ] Janela de histórico: busca, favoritos, apagar
- [ ] Revisão de favoritos, exportar CSV

### Fase 5 — Configurações
- [ ] `Settings` persistido (atalho, idiomas, autostart, histórico, modelo, tema)
- [ ] `set_shortcut` com rollback em conflito
- [ ] Gerenciamento de chaves ("configurada ✓", trocar, remover, testar)
- [ ] `autostart`

### Fase 6 — Distribuição (Windows)
- [ ] Capabilities mínimas por janela + `/ecc:security-review`
- [ ] Ícones finais; bundle NSIS/MSI
- [ ] CI Windows (test, clippy, fmt, build)
- [ ] `tauri-plugin-updater` com chave própria + GitHub Releases
- [ ] README com instalação e aviso do SmartScreen

## Riscos
| Risco | Mitigação |
|---|---|
| Captura via Ctrl+C falha ou corrompe clipboard | Polling com timeout, restore garantido, testes com fake |
| Popup sem foco / posição errada em multi-monitor e DPI misto | Conversão física/lógica, testes manuais |
| Vazamento de chave em logs/erros | `AppError` sem segredos, sem `get_secret`, `tracing` sem headers |
| Cota DeepL Free (500k caracteres/mês) | Limite de entrada e mensagem clara no erro 456 |
| SmartScreen em app não assinado | Documentar no README |
| Prompt injection no "Explicar" | Sem tools, saída só texto |

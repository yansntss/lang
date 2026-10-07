# Plano: Tradutor por atalho (Tauri v2)

Atualizado ao fim de cada fase. Marcar `[x]` ao concluir.

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
- [ ] Renomear branch para `main`, commit inicial do scaffold
- [ ] Remover `opener` e o demo do template; CSP restritivo
- [ ] `AppError` (thiserror) e módulo `secrets` com trait + `InMemoryStore`
- [ ] vitest configurado; entradas Vite multi-página

### Fase 1 — MVP (bandeja + atalho + janela + DeepL)
- [ ] Decisão de idioma + `DeepLClient` com testes `wiremock` (403, 456, timeout)
- [ ] `KeyringStore`, comandos de segredo, janela `settings` mínima (chave DeepL)
- [ ] Janela `popup` pré-carregada e oculta
- [ ] Tray (Abrir, Configurações, Sair)
- [ ] Atalho global + posicionamento perto do cursor (multi-monitor/DPI)
- [ ] Front: tradução, Esc fecha, Enter copia, oculta ao perder foco
- [ ] `single-instance`

### Fase 2 — Captura de texto selecionado
- [ ] `ClipboardPort` + sequência salvar/copiar/ler/restaurar testada com fake
- [ ] Aguardar modificadores soltos; polling com timeout; capturar antes de mostrar o popup
- [ ] `popup://prefill` + tradução automática

### Fase 3 — Explicar com Claude (opcional em runtime)
- [ ] `ClaudeClient` + testes SSE com `wiremock`
- [ ] Prompt seguro (sem tools, `max_tokens` limitado, texto como dado)
- [ ] `explain` com streaming; botão desabilitado sem chave
- [ ] `ExplainPanel` renderizando texto puro

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

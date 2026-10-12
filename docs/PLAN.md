# Plano: Tradutor por atalho (Tauri v2)

Atualizado ao fim de cada fase. Marcar `[x]` ao concluir.

## Estado atual (2026-10-10) — retomar daqui
- Fase 3b (captura de região + OCR + legenda ao vivo) **implementada, revisada e testada**, em
  três commits; falta a verificação manual (checklist na seção da Fase 3b).
- Fases 0, 1 e 2 concluídas, revisadas, testadas manualmente pelo usuário e commitadas em `main`.
- Fase 3 (Explicar com Claude) **implementada, revisada e testada com mocks**; falta só a
  verificação manual com uma chave real da Anthropic (o usuário tem apenas DeepL Free). Sem
  chave, o botão fica desabilitado, com dica.
- Fase 4 (Histórico e aprendizado) **implementada, revisada e testada**; falta só a
  verificação manual (checklist na seção da Fase 4).
- Fase 5 (Configurações) **implementada, revisada e testada**; falta só a verificação manual
  (checklist na seção da Fase 5).
- Fase 6a (distribuição: instalador NSIS, ícone, README, CI, auditoria, falha de inicialização)
  **feita e testada localmente**; falta só instalar e conferir (checklist na seção da Fase 6).
- Fase 6b (atualizador e release) **implementada e testada localmente**; falta o teste ponta a
  ponta no GitHub (publicar `v0.1.0`, instalar, publicar `v0.1.1` e atualizar pelo app; ver o
  checklist da Fase 6b).
- As duas decisões da Fase 2 viraram opções nas Configurações (editores na captura: desligada;
  traduzir a seleção sozinha: ligada). Continua em aberto:
  - Histórico ligado por padrão grava texto selecionado em claro desde o primeiro Enter:
    manter assim, ou mostrar um aviso na primeira gravação / começar desligado.
- Testes: 377 de Rust (+5 `#[ignore]` que usam o clipboard, o keychain e o OCR reais:
  `cargo test -- --ignored`; só o OCR: `cargo test --lib services::ocr::windows -- --ignored`)
  e 213 do front. `clippy -D warnings` e `fmt --check` limpos.
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
| `set_shortcut(slot, accelerator)` (slot `translate` ou `ocr`) | 3b |
| `get_region_snapshot` / `confirm_region(selection)` / `cancel_region` (só a janela `region`) | 3b |
| `start_caption` / `stop_caption` (só o popup) | 3b |

Eventos Rust → front: `popup://prefill`, `popup://reset`, `region://show`, `popup://caption`,
`popup://caption-stopped`.

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
- ~~Falha de inicialização em release é silenciosa e `panic = "abort"` não descarrega logs.~~
  Resolvido na Fase 6a (`fatal.rs`).
- ~~Rodar `cargo audit` no Windows na Fase 6.~~ Feito na Fase 6a.

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
- [x] Schema + migrations (`user_version`), testes com `tempfile`
- [x] Gravação (ligada por padrão), dedup, opção de desligar
- [x] Janela de histórico: busca, favoritos, apagar
- [x] Revisão de favoritos, exportar CSV

Verificação manual (pendente; `npm run tauri dev`):
- [ ] Traduzir um texto e apertar Enter: ele aparece em "Histórico" (bandeja → Histórico);
  digitar sem confirmar não grava
- [ ] Favoritar, buscar (inclusive sem acento: "ola" acha "Olá"), copiar, apagar um item
- [ ] Aba "Revisão": mostrar tradução, Próximo, voltar ao cartão menos revisado
- [ ] Desligar "Gravar histórico": Enter deixa de gravar; os itens antigos continuam
- [ ] "Exportar CSV": o arquivo aparece em Downloads, abre no Excel com acentos corretos, e uma
  segunda exportação no mesmo segundo não sobrescreve a primeira
- [ ] "Limpar tudo" pede confirmação e apaga também os favoritos
- [ ] Com o app em uso, a janela de histórico aberta não atualiza sozinha (ver pendências)

Notas e decisões da Fase 4:
- Banco `history.db` (SQLite via `rusqlite` bundled) na pasta de dados do app, acessado só pelo
  Rust; comandos `async` com `spawn_blocking`. Se o banco não abrir, o app sobe e traduz
  normalmente, e os comandos de histórico devolvem "Não foi possível acessar o histórico".
- Grava só quando o usuário **confirma** (Enter ou Explicar), uma vez por tradução, e não a cada
  rascunho que o popup traduz enquanto se digita. Repetir o mesmo texto e idioma de destino
  atualiza a tradução, a data e o contador (`UNIQUE(source_text, target_lang)`).
- Teto de 5000 itens **não favoritos** (os mais antigos saem); favoritos nunca saem sozinhos.
- Datas em milissegundos unix; o CSV usa ISO 8601 UTC. Sem `chrono`.
- A chave "Gravar histórico" fica na tabela `preferences` do próprio banco. A Fase 5 pode
  migrá-la para o `tauri-plugin-store`.
- Busca sem diferenciar maiúsculas nem acentos do português (função `fold` registrada no SQLite);
  limitada a 200 caracteres. Páginas de 50 itens por `OFFSET`.
- "Limpar tudo" usa `secure_delete` + `VACUUM`: o texto não sobra no arquivo (há teste lendo os
  bytes do `history.db`).
- CSV: UTF-8 com BOM, CRLF, todos os campos entre aspas, células começando com `= + - @` ou
  tab/CR/LF levam um apóstrofo (proteção contra fórmula; textos legítimos como "-5 degrees"
  saem com o apóstrofo visível). Gravado em `Downloads` com `create_new` (nunca sobrescreve;
  sufixo `-2`, `-3`… se o nome existir). Sem plugin de diálogo.
- A revisão oferece o favorito há mais tempo sem revisão (os nunca revisados primeiro).

Revisões da Fase 4 (rust-reviewer, security-reviewer, react-reviewer): sem itens críticos.
Corrigido: `secure_delete`/`VACUUM`, exportação que sobrescrevia, gravação e poda numa transação,
busca sem acento, `busy_timeout`, limite da busca, validação do idioma de origem, `\n` na proteção
de fórmula, log de falhas dos comandos, corrida de `loadMore`/`clear`/`toggle` no front, lista
velha após erro de recarga, erros presos, aviso de sucesso antes da resposta, cartão de revisão
(duplo clique, tentar de novo, recarga após limpar), rótulos de acessibilidade, estado "gravação
desconhecida", gravação duplicada (Explicar + Enter) e o foco que se perdia após Explicar (agora
o Enter continua copiando).
Pendências conhecidas (baixo risco, adiadas):
- Privacidade (decisão 3 acima): sem aviso na primeira gravação; banco em texto claro (sem
  SQLCipher).
- Sem teto de bytes nem de favoritos: o pior caso teórico passa de centenas de MB, e `export`
  carrega tudo em memória.
- Banco corrompido ou de versão futura deixa o histórico indisponível sem mensagem específica
  nem recuperação (renomear para `.bak`).
- A janela de histórico não recarrega sozinha quando o popup grava um item (reabra ou busque).
- Tablist sem navegação por setas (usa botões com `aria-pressed`); foco não é movido ao revelar
  a tradução ou apagar um item; `OFFSET` pode repetir/pular um item se algo for gravado entre
  cliques em "Carregar mais"; `delete` de id inexistente é silencioso, `toggle`/`mark_reviewed`
  dão erro; a busca de um item só acha o que o `fold` cobre (letras latinas comuns).

### Fase 5 — Configurações
- [x] `Settings` persistido (atalho, idiomas, autostart, histórico, modelo, tema)
- [x] `set_shortcut` com rollback em conflito
- [x] Gerenciamento de chaves ("configurada ✓", trocar, remover, testar)
- [x] `autostart`
- [x] Opções que fecham as decisões da Fase 2 (editores na captura; traduzir a seleção só no Enter)

Verificação manual (pendente; `npm run tauri dev`):
- [ ] Configurações → Atalho global → "Alterar atalho", apertar `Ctrl+Alt+K`: o novo atalho abre
  o tradutor, o antigo deixa de abrir, e ao reiniciar o app o novo continua valendo
- [ ] Escolher um atalho que outro app já usa (ex.: um da própria Windows): o erro aparece e o
  atalho antigo continua funcionando
- [ ] `Ctrl+C`, `Alt+F4` ou só uma letra são recusados
- [ ] Com o atalho novo, selecionar texto em outro app e abrir o tradutor: a seleção é capturada
  (a espera da tecla solta usa a tecla nova, sem "digitar" a letra no app de origem)
- [ ] "Testar chave" do DeepL mostra o uso do mês; sem rede ou com chave errada mostra o erro
- [ ] "Testar chave" da Anthropic (precisa de chave real; só testado com mocks)
- [ ] "Iniciar com o Windows": ligar, reiniciar a sessão e ver o app na bandeja; desligar
- [ ] Tema Claro/Escuro/Sistema muda popup, histórico e configurações (ao receber foco)
- [ ] Inglês EN-GB e português PT-PT: a tradução usa a variante escolhida e o histórico a grava
- [ ] Modelo Sonnet: o "Explicar" usa o modelo escolhido (precisa de chave real)
- [ ] "Traduzir a seleção assim que abre" desligado: o texto capturado aparece no campo, nada é
  enviado ao DeepL, "Enter traduz" e o segundo Enter copia
- [ ] "Capturar em editores" ligado: a captura funciona no VS Code/Cursor (e continua bloqueada
  em terminais); desligado, volta a ser bloqueada

Notas e decisões da Fase 5:
- **Persistência sem `tauri-plugin-store`** (desvio da tabela de decisões): um `settings.json` na
  pasta de dados do app, lido e gravado só pelo Rust (`serde`). Escrita atômica (arquivo
  temporário com `sync_all` + renomeação); a memória só muda depois de gravar. Arquivo ausente
  usa os padrões; ilegível, com valor fora da lista ou não lido é guardado como `settings.json.bak`
  e os padrões entram no lugar. Campos novos/ausentes recebem o padrão.
- `history_enabled` continua na tabela `preferences` do SQLite; o autostart não é salvo: é lido
  da chave `Run` do Windows (`tauri-plugin-autostart`, chamado só pelo Rust).
- Atalho: texto validado no Rust (`platform/accelerator.rs`): modificadores Ctrl/Alt/Shift/Win e
  tecla A–Z, 0–9 ou F1–F12. Para letras e números exige dois entre Ctrl, Alt e Shift (ou Win);
  teclas de função aceitam um modificador, menos `Alt+F4`. Isso barra `Ctrl+C`/`Ctrl+V`/`Alt+A`,
  que engoliriam essas teclas em todos os apps e quebrariam a captura. A troca é
  `unregister(antigo)` → `register(novo)` e, se falhar, o antigo volta; o novo só é salvo depois
  de registrado, e se o salvamento falhar o registro volta ao antigo.
- A tecla principal do atalho (antes fixa em `VK_T`) e a liberação de editores ficam em
  `platform/capture_config.rs` (atômicos lidos pela thread da captura).
- Idiomas: `TargetLang` ganhou `PT-PT` e `EN-GB`; `translate_with` recebe as variantes. O
  histórico aceita as quatro. A detecção da origem não mudou.
- Modelo do Explicar: lista fechada (`claude-haiku-5-5`, `claude-sonnet-5-5`); o ID vem de um
  enum, nunca do front, e entra no `Prompt`. `update_settings` usa um tipo de entrada sem
  valores padrão e sem campos extras (payload incompleto é erro, não zera preferências).
- "Testar chave" sem gastar cota: DeepL `GET /v2/usage` (mostra "uso neste mês"), Anthropic
  `GET /v1/models`. Mesmos cuidados dos outros clientes (HTTPS, sem redirect, cabeçalho sensível).
- Editores (VS Code, Cursor, JetBrains, Zed, VSCodium…) foram separados dos terminais na lista de
  bloqueio; liberar editores nunca libera terminais, SSH, consoles remotos nem app desconhecido.
- A janela de configurações aplica as alterações uma de cada vez (ref síncrono) e junta as
  respostas como trechos, para uma seção não desfazer a outra. Tema: `data-theme` em `base.css`;
  cada janela o lê ao abrir e ao receber foco.
- Capabilities: `settings` recebe os comandos novos; `popup` e `history` recebem só
  `allow-get-settings` (tema; o popup lê `autoTranslate` do evento de reset).

Revisões da Fase 5 (rust-reviewer, security-reviewer, react-reviewer): sem itens críticos. O que
bloqueava e foi corrigido: atalhos que roubam teclas comuns, alterações seguidas que se
atropelavam, gravação do atalho que prendia o teclado, duplo envio e resposta tardia na chave,
tema com respostas fora de ordem, seleção em branco retida, `update_settings` com padrões,
gravação sem `sync_all`, arquivo ilegível sem backup, atalho salvo inválido que não era corrigido,
falhas de gravação sem log, mais editores na lista.
Pendências conhecidas (baixo risco, adiadas):
- Um valor inválido em `settings.json` (ex.: versão futura) descarta o arquivo inteiro, inclusive o
  atalho, em vez de aplicar campo a campo; o `.bak` preserva o original.
- `apply_shortcut` não é serializado (hoje o comando é síncrono e só a janela de configurações o
  chama); se o rollback também falhar, a dica da bandeja não avisa.
- Flash do tema forçado ao abrir uma janela (o tema só é aplicado após o primeiro `get_settings`);
  o popup/histórico abertos e sem foco só trocam de tema quando recebem foco.
- Gravação do atalho continua só por teclado físico (sem campo de texto alternativo); em teclado
  ABNT2 o texto mostrado usa a letra da tecla física, e `Ctrl+Alt` equivale ao AltGr.
- Foco dos selects não é restaurado após salvar; `Preferences` sem teste de payload antigo do
  front além do tipo de entrada; teste de `capture_config` depende de estado global.

### Fase 6 — Distribuição (Windows)
Dividida em **6a** (sem depender do GitHub; feita) e **6b** (atualizador e release; pendente).

6a:
- [x] Capabilities mínimas por janela + revisão de segurança final
- [x] Ícones finais; bundle NSIS (por usuário, sem administrador)
- [x] CI Windows (test, clippy, fmt, build) + auditoria
- [x] README com instalação e aviso do SmartScreen
- [x] Falha de inicialização visível e pânico em log
- [x] Versão única nos três arquivos (`npm run check:version`)

6b (implementada; falta o teste ponta a ponta):
- [x] Chave de assinatura própria (fora do repositório; só a pública está no `tauri.conf.json`)
- [x] `tauri-plugin-updater` chamado só pelo Rust; comandos `check_update` e `install_update`
- [x] Seção "Sobre e atualizações" nas Configurações (manual, sem verificação automática)
- [x] Workflow `release.yml` (tag `v*`), `version:bump`, `build:local` e Dependabot
- [ ] Teste ponta a ponta: release `v0.1.0` → instalar → release `v0.1.1` → atualizar pelo app

Verificação ponta a ponta (pendente; cria releases públicos, então só com a ordem do autor):
- [ ] Ambiente `release` do GitHub com os dois segredos e a regra de tag `v*` (feito pelo autor)
- [ ] `git tag v0.1.0` + `git push origin v0.1.0`: o workflow roda os testes, assina e publica o
  release com o `.exe`, o `.sig` e o `latest.json`
- [ ] Conferir no release: o `latest.json` aponta para o `.exe` e a `signature` não está vazia
- [ ] Instalar o `v0.1.0` baixado do release (SmartScreen: Mais informações → Executar assim mesmo)
- [ ] `npm run version:bump -- 0.1.1`, commit, push, tag `v0.1.1` e `git push origin v0.1.1`
- [ ] No app instalado (0.1.0): Configurações → Sobre e atualizações → Verificar → Instalar e
  reiniciar: o app fecha, o instalador mostra o progresso e o app reabre na 0.1.1 (confirmar que
  histórico, chaves e configurações continuam)
- [ ] Verificar de novo na 0.1.1: "Você já está na versão mais recente"
- [ ] Se a atualização for recusada por causa do `requireSignedVersion`, ver o log
  (`%LOCALAPPDATA%\com.yansa.lang-app\logs`); a opção pode ser desligada em `tauri.conf.json`

Verificação manual (pendente; o instalador está em
`src-tauri/target/release/bundle/nsis/Tradutor_0.1.0_x64-setup.exe`, 3,42 MiB):
- [ ] Fechar o `npm run tauri dev` (o `single-instance` encaminharia o segundo app ao primeiro)
- [ ] Executar o instalador: o SmartScreen avisa "editor desconhecido"; **Mais informações →
  Executar assim mesmo**; a instalação não pede administrador e fala português
- [ ] Atalho do menu Iniciar "Tradutor" com o ícone novo; ícone da bandeja legível
- [ ] Primeira execução abre as Configurações (sem chave do DeepL); salvar a chave e testar
- [ ] `Ctrl+Alt+T` abre o popup e traduz; o histórico e as configurações continuam os mesmos de
  antes (a pasta de dados é a mesma: o `identifier` não mudou)
- [ ] "Iniciar com o Windows" liga e o app sobe com a sessão
- [ ] Desinstalar pelo Windows: o app some; as chaves continuam no Gerenciador de Credenciais
  (documentado no README) e a opção de apagar os dados funciona
- [ ] Forçar uma falha de inicialização (ex.: tornar a pasta de dados ilegível) mostra a caixa de
  erro e grava `%LOCALAPPDATA%\com.yansa.lang-app\startup-error.log`

Notas e decisões da Fase 6a:
- **Instalador só NSIS, por usuário** (`installMode: currentUser`, idioma `PortugueseBR`): sem
  WiX/MSI (que exige administrador e baixa o WiX). O nome visível agora é **Tradutor**; o
  `identifier` `com.yansa.lang-app` não mudou, porque a pasta de dados, o histórico e as chaves do
  Gerenciador de Credenciais dependem dele (há um teste em `fatal.rs` que o confere).
- **Ícone** gerado por `scripts/make-icon.ps1` (dois balões, "EN" e "PT") a partir de
  `src-tauri/icon-source.png`; os tamanhos saem de `npm run tauri icon src-tauri/icon-source.png`
  (as pastas `android/` e `ios/` que o comando cria foram removidas). É um ícone simples, não
  arte final: para trocar, rode o mesmo comando com outro PNG de 1024 px.
- **Falha de inicialização:** `fatal.rs` mostra uma caixa de mensagem (`MessageBoxW`) e grava o
  detalhe em `startup-error.log` (a caixa só cita o arquivo). O `panic hook` registra só o local
  do pânico e descarrega o log antes do `abort`; a mensagem do pânico é omitida de propósito,
  porque pode conter o texto que o usuário estava traduzindo.
- **Auditoria:** `cargo audit` (instalado nesta fase) não achou vulnerabilidades; os 2 avisos
  (`proc-macro-error` sem manutenção e `glib` com solidez) são dependências só de Linux (GTK) que
  não entram no grafo do Windows (`cargo tree --target x86_64-pc-windows-msvc -i …` não imprime
  nada). `npm audit`: 0 vulnerabilidades.
- **CI** (`.github/workflows/ci.yml`): job Windows (checagem de versão, `tsc`, testes, build do
  front, `fmt`, `clippy -D warnings`, `cargo test`) e job de auditoria. Token só de leitura, sem
  segredos, ações **fixadas por commit** (o comentário traz a versão) e `cargo-audit@0.22.2`.
  Ainda não foi executado no GitHub: os mesmos comandos passaram localmente.
- O binário de release carrega a configuração inteira embutida, inclusive o `devCsp` e o
  `devUrl` (`localhost:1420`); eles só valem em `tauri dev`. Não há fixtures de teste nem chaves
  de exemplo no binário.
- Licença MIT adotada com o repositório público (arquivo `LICENSE`); o README principal ficou em
  português, com `README.en.md` em inglês e a rotina de release em `docs/RELEASING.md`.

Revisão de segurança final (security-reviewer): sem itens críticos ou altos; o histórico git não
tem segredos. Corrigido: `.gitignore` (certificados, bancos, `.claude/settings.json`), ações do CI
fixadas. Pendências (baixo risco ou só se o repositório virar público):
- **Publicação do repositório (decidido em 2026-10-08):** o hostname da máquina foi removido do
  histórico (autor do primeiro commit e uma nota deste arquivo) com `git filter-branch`. Os 45
  commits mantêm o mesmo conteúdo, mas têm **novos SHAs** e o histórico precisa de `force push`
  (com `--force-with-lease`) antes de o repositório virar público. Por decisão do autor,
  continuam públicos: o e-mail `yansantos.dev@gmail.com` nos commits, `CLAUDE.md` e
  `.claude/rules/**` (citam caminhos locais e o usuário do Windows) e o identificador
  `com.yansa.lang-app`. Sem arquivo de licença.
- `startup-error.log` cresce sem limite e é aberto sem checar link simbólico (risco baixo: pasta
  do próprio usuário); `style-src 'unsafe-inline'` continua no CSP de release.

Notas da 6b (atualizador e release):
- O repositório passou a ser público (decisão do autor), então o `latest.json` do GitHub Releases
  é acessível sem autenticação. O endpoint é fixo no `tauri.conf.json`:
  `https://github.com/yansntss/lang/releases/latest/download/latest.json`.
- **Só o Rust fala com o plugin.** Nenhuma janela tem permissão `updater:*`; só a janela de
  Configurações recebe `check_update` e `install_update` (há testes que travam isso). O front não
  escolhe endpoint, URL nem arquivo: instala exatamente a versão achada pela última verificação.
- **Estado:** uma operação por vez (verificar ou instalar); uma verificação que falha apaga a
  anterior; instalar sem verificar é erro. "Ainda não há release" (404) vira "já está na versão
  mais recente". Erros chegam ao front como frases genéricas; o detalhe vai ao log.
- **Segurança da atualização:** o plugin confere a assinatura minisign do instalador com a chave
  pública. `requireSignedVersion` está ligado: o `latest.json` não é assinado, então ele exige que
  a versão anunciada seja a gravada **dentro da assinatura** (impede empurrar um instalador antigo e
  legítimo com um número de versão inflado). Foi **verificado na prática**: o CLI 2.12.1 grava
  `version:0.1.0` no comentário confiável da `.sig` (testado com uma chave descartável, já
  apagada). `allow_downgrades` e todas as opções `dangerous_*` ficam desligadas (teste lê o
  `tauri.conf.json` real com o parser do plugin).
- **Notas da versão** vêm de fora: texto puro, sem caracteres de controle nem de direção de
  texto, cortadas em 4000 caracteres.
- **Release (`release.yml`)** publica **direto, sem rascunho** (decisão do autor). Mitigações:
  roda antes os mesmos testes e a auditoria do CI (`workflow_call`); confere que a tag é `v` +
  versão do app; recusa uma tag cujo commit não está na `main`; tag com sufixo (`-rc.1`) vira
  pré-release, que o atualizador não oferece; o job que tem a chave não restaura cache de npm
  nem de Cargo; confere que o `Cargo.lock` está em dia (`cargo metadata --locked`); ações fixadas
  por commit; `permissions: {}` no topo e `contents: write` só no job de release; segredos só no
  ambiente `release`, restrito a tags `v*`.
- **Se a chave se perder ou vazar:** procedimento no README (seção "Publicar uma versão").
- **Instalador local:** `npm run build:local` (sem a chave) gera o instalador sem os arquivos de
  atualização; `npm run tauri build` completo exige as variáveis `TAURI_SIGNING_PRIVATE_KEY*`.

Revisão de segurança da 6b (security-reviewer): sem itens críticos ou altos. Corrigido: corrida
entre verificar e instalar (uma trava só), caracteres enganosos nas notas, cache no job de release,
tag fora da `main`, pré-release, documentação de perda/vazamento da chave, Dependabot. Pendências
(decisão do autor ou baixo risco):
- **Publicação direta, sem aprovação:** quem tiver escrita no repositório e empurrar uma tag `v*`
  publica uma versão assinada. Recomendado no GitHub: 2FA com chave física, ruleset de tags `v*` e
  proteção da `main`, e "Required reviewers" no ambiente `release` (instruções no README). O
  `releaseDraft: true` seria o controle mais barato se o autor mudar de ideia.
- A chave privada ainda fica no ambiente do passo de build (que executa `build.rs`, macros e
  scripts de dependências). O ideal seria compilar sem a chave e assinar num passo final
  (`tauri signer sign` + `latest.json` manual); fica como melhoria.
- Toolchain do Rust e `windows-latest` não são fixos (reprodutibilidade).

## Fase 3b — Captura de região + OCR (texto de jogos)

Status: implementada em três commits (`feat: select a screen region with an overlay`,
`feat: read the selected region with the Windows OCR`, `feat: live caption mode for a screen
region`). Falta a verificação manual.

Fluxo: atalho de OCR (padrão `Ctrl+Alt+Y`, configurável; não pode ser igual ao do tradutor) →
foto do monitor sob o cursor → janela `region` (opaca, sem borda, sempre no topo, do tamanho do
monitor) mostra a foto escurecida → o usuário arrasta um retângulo (Esc ou perder o foco
cancela) → recorte em pixels físicos → OCR numa thread própria → popup fixado ao lado da região
com o texto (traduzido sozinho conforme a preferência "Traduzir a seleção assim que…").

- [x] Atalho por slot (`ShortcutSlot`), com `apply_shortcut` genérico e rollback. Quem já usava
  `Ctrl+Alt+Y` no tradutor: o OCR cai para o de fábrica ou para `Ctrl+Alt+Shift+Y`
  (`without_collision`). O atalho apertado é comparado com o que está **registrado** em memória,
  não com o arquivo. A dica da bandeja lista os atalhos que o sistema recusou.
- [x] Captura com `xcap` 0.9 (GDI no Windows; mesmo `windows` 0.62 do app). Só para Windows e
  macOS: no Linux o xcap puxa PipeWire/XCB, e a captura devolve "não suportado".
- [x] A foto fica só em memória (`RegionState`), chega à janela de seleção como bytes crus
  (`ipc::Response`, cabeçalho de 8 bytes + RGBA) e é desenhada num canvas: a CSP não mudou. É
  descartada ao confirmar, cancelar ou falhar, e o canvas é limpo ao fechar.
- [x] Geometria pura e testada (`region_geometry.rs`): seleção lógica → física com a escala do
  monitor, arrasto invertido, recorte nas bordas, monitores com origem negativa, tamanho mínimo
  (`RegionEmpty`); popup ao lado da região sem cobri-la (abaixo, acima, à direita, à esquerda).
- [x] Captura toda preta (jogo em tela cheia exclusiva) vira `BlankCapture`, com a orientação de
  usar janela sem bordas.
- [x] OCR: trait `OcrEngine`, `OcrService` (upscale 2× quando o menor lado < 300 px, limpeza do
  texto), Windows = `Windows.Media.Ocr` (WinRT MTA na thread do OCR, BGRA via `DataWriter`,
  `RecognizeAsync().join()`). Prefere en-US; sem ele, os idiomas do perfil
  (`TryCreateFromUserProfileLanguages`). macOS (Vision) e Linux (Tesseract) ficam para depois,
  atrás de `cfg` (`UnsupportedOcr`).
- [x] O popup abre já na confirmação (o app ainda tem o primeiro plano, então o Windows dá o
  foco), e o texto chega quando o OCR termina. Só a leitura mais recente aparece (`Generation`).
- [x] Histórico com origem (`source` = `text` | `ocr`, migração 2, selo "OCR" na lista, coluna
  `origem` no CSV). Conta como OCR só se o texto confirmado for o lido da tela, sem edição.
- [x] Legenda ao vivo (desligada por padrão; intervalo de 1 a 5 s nas Configurações, o backend
  limita a 0,5–10 s): relê a mesma região, `CaptionGate` (impressão digital normalizada, 2
  leituras iguais seguidas, não repete a última) e só então traduz (ou espera o Enter, se
  "Traduzir a seleção assim que…" estiver desligada). Para no botão, em qualquer reset ou
  ocultação do popup, ao ler outra região, ao desligar a opção e após 3 falhas seguidas
  (captura preta conta como falha). Começar e parar passam pelo mesmo lock da geração: o fim de
  uma sessão antiga nunca encerra a nova. Durante a legenda o popup fica fora das capturas
  (`set_content_protected`, que no Windows é `WDA_EXCLUDEFROMCAPTURE`) e volta ao normal ao
  parar. As falas só entram no histórico se o usuário confirmar (Enter/Explicar).
- Melhoria futura: a legenda fotografa o monitor inteiro a cada leitura e recorta; capturar só a
  região (se o xcap expuser isso de forma confiável) reduziria o custo em 4K.

Decisões e achados:
- Neste PC o Windows só tinha o OCR de **pt-BR** (não o en-US, apesar do sistema em inglês); o
  de pt-BR leu corretamente o texto em inglês do teste real. Para a melhor precisão em inglês:
  PowerShell como administrador → `Add-WindowsCapability -Online -Name
  "Language.OCR~~~en-US~0.0.1.0"`.
- Janela de seleção opaca com a foto congelada, em vez de transparente: mesma aparência, sem os
  problemas de transparência do WebView2, e o texto do jogo para de se mexer enquanto se seleciona.
- Hífen no fim da linha: mantido e colado à próxima palavra (`well-known`); jogos quase nunca
  hifenizam palavras.

Verificação manual (pendente; `npm run tauri dev`):
- [ ] `Ctrl+Alt+Y` escurece o monitor sob o cursor; arrastar e soltar abre o popup fixado ao lado
  da região com o texto lido e traduzido; Esc e clicar em outro app cancelam.
- [ ] Monitor secundário e escala diferente de 100% (125%, 150%): o recorte pega exatamente o que
  foi selecionado.
- [ ] Jogo em janela sem bordas funciona; em tela cheia exclusiva aparece o aviso da captura preta.
- [ ] Trocar o atalho de OCR nas Configurações; tentar usar o do tradutor mostra o erro.
- [ ] Legenda ao vivo: ligar nas Configurações, ler uma legenda, "Legenda ao vivo", falas novas
  traduzidas uma vez cada; "Parar legenda" e Esc encerram; o popup não aparece em prints durante
  a legenda e volta a aparecer depois.
- [ ] Histórico mostra o selo "OCR" e o CSV traz a coluna `origem`.

## Popup redimensionável
- O popup agora é `resizable` (mínimo 320×140; sem maximizar). O tamanho escolhido pelo usuário
  fica em `settings.json` (`popupSize`, pixels lógicos) e é reaplicado a cada exibição, limitado a
  90% da área útil do monitor. A gravação acontece ao ocultar, não a cada pixel arrastado.
- Crescimento automático: o front mede o conteúdo (`useFitPopupHeight`) e chama
  `fit_popup_height`; o Rust usa `max(altura escolhida, altura do conteúdo)` até o limite e puxa a
  janela de volta para dentro do monitor. Só a altura cresce; a largura é do usuário.
- O campo de texto também cresce com o que é digitado (`field-sizing: content`, de 3 a 8
  linhas; depois rola). O limite é só visual, sem teto de caracteres. O hook observa o campo
  além do resultado, então a janela acompanha.
- Um `Resized` que o app acabou de pedir não conta como escolha do usuário (`popup_size.rs`,
  `is_echo`), nem os que ocorrem nos 300 ms após a exibição.
- A borda nativa não serviu: numa janela sem decoração o Tauri (`tauri-runtime-wry`,
  `undecorated_resizing`) usa uma faixa invisível da largura de `SM_CXFRAME` (cerca de 4 px), e
  o usuário não conseguiu redimensionar. O popup tem agora alças próprias (`ResizeGrips`, 6 px
  nas bordas e 10 px nos cantos, dentro do padding) que chamam `start_popup_resizing`, e o
  sistema conduz o arrasto (`Window::start_resize_dragging`). `ResizeEdge` é uma lista fechada.
- Mover: duplo clique segurado (`event.detail === 2`) fora do campo, dos botões e dos textos
  selecionáveis chama `start_popup_dragging` (`useWindowDrag`).
- Fixar: botão de alfinete no canto do campo (`PinButton`). Fixado, o blur não oculta, o
  atalho com o popup visível só foca e preenche (sem reposicionar), e o Enter copia sem fechar.
  Ocultar (Esc) solta. O estado vive no Rust (`PopupState.pinned`) e volta ao front pelo
  `popup://reset`. Não persiste entre execuções.
- O arrasto nativo (mover ou redimensionar) tira o foco do conteúdo web, e o popup se ocultava
  como num clique fora (só funcionava fixado). `PopupState.interacting` ignora esse blur até o
  botão do mouse ser solto (`GetAsyncKeyState`, no mínimo 250 ms, teto de 60 s) e depois devolve
  o foco ao popup.
- Verificação manual pendente: alças, arrasto e fixar no Windows real, e monitores com DPI
  diferente.

## Identidade visual (Lang)

Status: concluída.

- Nome "Lang": `productName`, título do popup, dica da bandeja, pacote npm `lang`, crate `lang`
  (exe `lang.exe`, lib `lang_lib`). O instalador passa a se chamar `Lang_<versão>_x64-setup.exe`.
- O `identifier` continua `com.yansa.lang-app`: ele nomeia a pasta de dados (configurações e
  histórico) e o serviço do keychain. Trocar perderia as chaves e o histórico.
- Fontes em `assets/icon/`: `lang-icon.svg` (colorido) e `lang-icon-mono.svg` (`currentColor`).
  `src-tauri/icons/` é gerado com `npm run tauri icon assets/icon/lang-icon.svg` (sem as pastas
  `android/` e `ios/`).
- Bandeja: Windows/Linux usam o ícone do app. No macOS, `icons/tray/tray-template@2x.png` (preto,
  44 px; há também o de 22 px) com `icon_as_template(true)`. Gerado do SVG mono com `currentColor`
  trocado por preto e `tauri icon --png 22,44`. Sem teste no macOS.
- Favicon SVG nas três páginas; no popup, o ícone de 14 px fica ao lado do alfinete
  (`.popup__corner`), com `pointer-events: none` para o clique cair no campo.

## Riscos
| Risco | Mitigação |
|---|---|
| Captura via Ctrl+C falha ou corrompe clipboard | Polling com timeout, restore garantido, testes com fake |
| Popup sem foco / posição errada em multi-monitor e DPI misto | Conversão física/lógica, testes manuais |
| Vazamento de chave em logs/erros | `AppError` sem segredos, sem `get_secret`, `tracing` sem headers |
| Cota DeepL Free (500k caracteres/mês) | Limite de entrada e mensagem clara no erro 456 |
| SmartScreen em app não assinado | Documentar no README |
| Prompt injection no "Explicar" | Sem tools, saída só texto |
| Jogo em tela cheia exclusiva: captura preta e a janela de seleção não aparece por cima | Detectar a captura preta (`BlankCapture`) e orientar o modo janela sem bordas |
| Anti-cheat (EAC, BattlEye, Vanguard) estranhar o app | Só captura GDI da tela e uma janela própria: sem injeção, sem hook, sem ler memória do jogo. Ainda assim, risco documentado |
| Pacote de OCR de inglês ausente | Cai para os idiomas do perfil; sem nenhum, `OcrLanguageMissing` com o caminho nas Configurações do Windows |
| Captura de tela é permissão sensível | Só por atalho do usuário, foto só em memória e só para a janela `region`, nada em disco ou log; o texto lido nunca vai ao log |
| Legenda gastando a cota do DeepL | Desligada por padrão, `CaptionGate` (hash + estabilidade), intervalo mínimo de 0,5 s, para sozinha após falhas |
| Wayland (Linux) e permissão de Gravação de Tela (macOS) | Fora do escopo: captura e OCR atrás de `cfg`, com erro claro; Linux exige portal do xdg-desktop, macOS exige pedir a permissão |

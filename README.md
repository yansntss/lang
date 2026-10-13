<p align="center"><img src="assets/icon/lang-icon.svg" alt="Lang" width="96" height="96"></p>

# Lang — tradutor por atalho

[English](README.en.md) · **Português**

App de bandeja para **Windows 10/11** (Tauri v2 + React/TypeScript) que traduz rápido entre inglês
e português, feito para quem está aprendendo inglês e quer traduzir sem sair do que está fazendo.
A interface e esta documentação estão em português. O repositório se chama `lang`.

<p align="center">
  <a href="https://github.com/yansntss/lang/releases/latest"><img src="https://img.shields.io/github/v/release/yansntss/lang?label=Baixar%20para%20Windows&logo=windows&style=for-the-badge" alt="Baixar para Windows"></a>
</p>

<p align="center"><img src="docs/demo.gif" alt="Demonstração: um texto selecionado é traduzido com Ctrl+Alt+T, e a legenda de um jogo é lida da tela com Ctrl+Alt+Y" width="480"></p>

Aperte **`Ctrl+Alt+T`** em qualquer app: abre uma caixa flutuante perto do cursor. Se havia texto
selecionado, ele já vem preenchido e é traduzido na hora. Inglês vira português; qualquer outro
idioma vira inglês. **Esc** fecha; **Enter** copia a tradução e fecha.

Texto que não dá para selecionar (legenda de jogo, imagem, vídeo)? Aperte **`Ctrl+Alt+Y`**, a tela
escurece, e você arrasta um retângulo sobre o texto: o app lê o trecho com o OCR do próprio
Windows e traduz.

## O que faz

- **Tradução** pelo DeepL, com detecção automática do idioma de origem.
- **Captura da seleção:** o texto selecionado em outro app já aparece no campo.
- **Ler da tela (OCR):** arraste sobre qualquer trecho da tela para traduzir o que estiver escrito
  nele. Usa o OCR nativo do Windows, offline e sem chave.
- **Legenda ao vivo** (opcional): depois de ler um trecho, o app relê a mesma área a cada intervalo
  e traduz cada fala nova, bom para legendas de jogos e vídeos.
- **Explicar** (opcional): o Claude explica gramática, vocabulário e dá exemplos de uso, em
  português, em tempo real.
- **Histórico:** guarda o que você confirma (Enter ou Explicar), com busca, favoritos, revisão dos
  favoritos em cartões e exportação para CSV.
- **Configurações:** os dois atalhos, chaves de API, tema, variantes (inglês americano/britânico,
  português do Brasil/de Portugal), modelo do Explicar, legenda ao vivo (e o
  intervalo) e início com o Windows.

## Instalação

1. Baixe o instalador `Lang_<versão>_x64-setup.exe` na
   [página da versão mais recente](https://github.com/yansntss/lang/releases/latest).
2. Execute-o. A instalação é **por usuário**: não pede administrador.
3. O Windows SmartScreen vai avisar sobre um **"editor desconhecido"**, porque o instalador não é
   assinado (assinatura de código é paga). Clique em **Mais informações → Executar assim mesmo**.
4. Na primeira execução a janela **Configurações** abre sozinha, pedindo a chave do DeepL. Siga o
   [passo a passo abaixo](#primeiro-passo-a-chave-do-deepl): sem ela o app não traduz.

Requisitos: Windows 10 ou 11 com WebView2 (já vem no Windows 11 e na maioria dos Windows 10).

## Primeiro passo: a chave do DeepL

Quem traduz é o **DeepL**, um serviço de tradução on-line. Para o Lang poder usá-lo, você precisa
de uma **chave de API**: um código que funciona como a senha do seu acesso ao DeepL. O plano
gratuito permite traduzir **500 mil caracteres por mês**, o que dá para muito uso no dia a dia.
Não precisa saber programar; é só criar a conta e copiar o código.

**1. Crie a conta gratuita no DeepL**

1. Abra [deepl.com/pro-api](https://www.deepl.com/pro-api#api-pricing) e escolha o plano
   **DeepL API Free** (gratuito).
2. Faça o cadastro com seu e-mail. O DeepL pode pedir um cartão de crédito só para confirmar sua
   identidade; o plano Free não cobra nada.
3. Se você já usa o tradutor do site do DeepL com uma conta, saia dela antes: a conta da API
   precisa ser uma conta nova.

**2. Copie a sua chave**

1. Já logado, abra [deepl.com/your-account/keys](https://www.deepl.com/your-account/keys)
   (página de chaves da sua conta).
2. Copie a chave inteira. Ela é parecida com `1a2b3c4d-....:fx`: chaves do plano gratuito
   **terminam em `:fx`**.
3. Trate a chave como uma senha: não publique nem envie para ninguém.

**3. Cole a chave no Lang**

1. A janela **Configurações** abre sozinha na primeira vez que o Lang roda. Se ela não estiver
   aberta, clique com o botão direito no ícone do Lang na bandeja (perto do relógio; se não
   aparecer, clique na setinha `^`) e escolha **Configurações**.
2. Na seção **DeepL**, cole a chave no campo **Chave da API do DeepL** e clique em
   **Salvar chave**.
3. O texto acima do campo muda para **Chave configurada ✓**, e aparece o botão **Testar chave**.
   Clique nele: deve aparecer **Chave válida ✓**, junto com quanto você já usou no mês.

Pronto! Selecione um texto em inglês em qualquer app e aperte **`Ctrl+Alt+T`**.

**Se algo der errado**

| Mensagem | O que fazer |
|---|---|
| *A chave de API do DeepL é inválida ou foi recusada.* | Copie a chave de novo, inteira, sem espaços, e salve outra vez. |
| *Chave de API do DeepL não configurada.* | Faltou salvar a chave: refaça o passo 3. |
| *A cota mensal do DeepL foi esgotada.* | Você usou os 500 mil caracteres do mês. Espere o período renovar (a data aparece na sua conta do DeepL). |

**Opcional: botão Explicar.** Para o Claude explicar a gramática e o vocabulário de uma tradução,
crie uma chave em [console.anthropic.com](https://console.anthropic.com/settings/keys) e cole na
seção **Anthropic** das Configurações, do mesmo jeito. Esse serviço é pago por uso. Sem a chave,
só o botão Explicar fica desligado; o resto funciona normalmente.

**Atualizações:** o app nunca procura versões novas sozinho. Em **Configurações → Sobre e
atualizações**, clique em **Verificar atualizações**; se houver uma versão nova, **Instalar e
reiniciar** baixa o instalador do GitHub Releases, confere a assinatura e instala por cima.

## Como usar

| Ação | Como |
|---|---|
| Abrir o tradutor | `Ctrl+Alt+T` (configurável), ou clique no ícone da bandeja |
| Traduzir um trecho de outro app | Selecione o texto e aperte o atalho |
| Traduzir texto da tela (jogo, imagem, vídeo) | `Ctrl+Alt+Y` (configurável) e arraste um retângulo sobre o texto. **Esc** cancela |
| Acompanhar uma legenda | Depois de ler um trecho, clique em **Legenda ao vivo** (ative antes em **Configurações**). **Parar legenda** encerra |
| Copiar a tradução | **Enter** (também grava no histórico) |
| Fechar | **Esc**, ou clique fora |
| Redimensionar | Arraste as bordas ou os cantos da caixa (o cursor muda ao passar por eles). O tamanho é lembrado, e a caixa cresce sozinha para caber a tradução e a explicação |
| Mover | Clique duas vezes numa área vazia da caixa e segure no segundo clique, depois arraste |
| Deixar na tela | Botão de alfinete no canto do campo: fixado, o popup não some ao clicar em outro app e o **Enter** copia sem fechar. **Esc** fecha e solta |
| Entender a tradução | Botão **Explicar** (precisa da chave da Anthropic) |
| Histórico, configurações, sair | Menu do ícone da bandeja |

![Caixa de tradução perto do cursor](docs/popup.png)

## Privacidade e segurança

- As chaves de API ficam **só no Rust**, no Gerenciador de Credenciais do Windows. A interface não
  consegue lê-las e elas nunca vão para o log.
- O texto que você traduz é enviado ao **DeepL**. O texto do **Explicar** é enviado à **Anthropic**
  **somente quando você clica** no botão. Há uma opção para a seleção capturada só ser traduzida
  depois do Enter.
- O **histórico** fica só neste computador, em `%APPDATA%\com.yansa.lang-app\history.db`, **em texto
  simples**, e pode incluir textos selecionados em outros apps. Dá para desligar a gravação,
  apagar itens ou tudo, e exportar na própria janela do histórico.
- Para capturar a seleção, o app simula `Ctrl+C` no app em foco e devolve o clipboard ao que era
  antes. Esse texto pode aparecer, por um instante, no histórico do clipboard do Windows (`Win+V`).
- A captura **nunca** é feita em terminais, clientes SSH, consoles remotos ou máquinas virtuais,
  onde `Ctrl+C` interromperia um processo. Em editores com terminal embutido (VS Code, Cursor,
  JetBrains) ela fica desligada por padrão e pode ser liberada nas Configurações. Nesses apps o
  popup abre vazio.
- Apps rodando como administrador não permitem a captura (limitação do Windows).
- A leitura da tela (OCR) acontece **no próprio computador**: só o texto reconhecido vai ao DeepL,
  nunca a imagem. Na legenda ao vivo, cada fala nova gasta cota do DeepL.

## Problemas comuns

- **O atalho não faz nada:** outro app pode estar usando a combinação. Troque em
  **Configurações → Atalho global**. O ícone da bandeja avisa quando o atalho não pôde ser
  registrado.
- **O popup abre vazio com texto selecionado:** o app em foco pode ser um terminal ou editor
  bloqueado, ou estar rodando como administrador. Digite ou cole o texto.
- **O OCR diz que não há idioma de reconhecimento:** adicione o **Inglês (Estados Unidos)** em
  **Configurações do Windows → Hora e idioma → Idioma e região**.
- **O app não abre:** uma caixa de mensagem informa o arquivo com o motivo
  (`%LOCALAPPDATA%\com.yansa.lang-app\startup-error.log`). Os logs normais ficam em
  `%LOCALAPPDATA%\com.yansa.lang-app\logs`.

## Desinstalar

Use **Configurações do Windows → Aplicativos**. O desinstalador oferece apagar os dados do app.
As **chaves de API não são removidas**: apague-as antes, em **Configurações → Remover chave**, ou
depois, no **Gerenciador de Credenciais do Windows** (credenciais genéricas que contêm
`com.yansa.lang-app`). O histórico e as configurações ficam em `%APPDATA%\com.yansa.lang-app`.

## Desenvolvimento

O projeto roda no Windows. Requisitos: Node.js 22+ e Rust (toolchain MSVC).

```powershell
git clone https://github.com/yansntss/lang.git
cd lang
npm install
npm run tauri dev
```

```powershell
npm test                                   # testes do front
npm run build                              # tsc + vite
npm run check:version                      # versão igual em package.json, Cargo.toml e tauri.conf.json
cd src-tauri
cargo test                                 # testes de Rust
cargo test -- --ignored                    # testes que usam o clipboard e o keychain reais
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo audit                                # vulnerabilidades conhecidas nas dependências
```

Gerar o instalador (NSIS, em `src-tauri/target/release/bundle/nsis/`) para testar na sua máquina.
Este comando não precisa da chave de assinatura do atualizador, porque não gera os arquivos de
atualização:

```powershell
npm run build:local
```

Para publicar uma versão (chave de assinatura, GitHub Actions, proteções do repositório), veja
[`docs/RELEASING.md`](docs/RELEASING.md). O plano, as decisões e o estado de cada fase estão em
[`docs/PLAN.md`](docs/PLAN.md).

## Problemas e sugestões

Encontrou um bug ou tem uma ideia? Abra uma [issue](https://github.com/yansntss/lang/issues).
Por enquanto o projeto não aceita pull requests.

## Licença

[MIT](LICENSE).

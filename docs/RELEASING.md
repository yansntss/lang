# Publicar uma versão (mantenedor)

O instalador oficial e os arquivos de atualização são gerados pelo GitHub Actions, que assina
com a chave do atualizador e publica o release. Os apps já instalados verificam e instalam a
nova versão em **Configurações → Sobre e atualizações**.

## Uma vez só: chave de assinatura

A chave fica **fora** do repositório e nunca é commitada:

```powershell
npm run tauri signer generate -- -w $env:USERPROFILE\.tauri\lang-updater.key
```

1. Escolha uma senha forte quando o comando pedir.
2. A chave **pública** (arquivo `lang-updater.key.pub`) vai em `src-tauri/tauri.conf.json`
   (`plugins.updater.pubkey`).
3. No GitHub, em **Settings → Environments → New environment**, crie o ambiente `release`, limite
   os *Deployment branches and tags* à regra de tag `v*` e cadastre nele os segredos
   `TAURI_SIGNING_PRIVATE_KEY` (o conteúdo do arquivo `lang-updater.key`) e
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (a senha).
4. Guarde um **backup** da chave privada e da senha num gerenciador de senhas. Se elas se
   perderem, os apps já instalados não aceitam mais atualização e precisam ser reinstalados à mão.

## Proteções recomendadas no GitHub

O release é publicado direto, sem rascunho: quem consegue empurrar uma tag `v*` publica uma
versão assinada.

- Ative a verificação em duas etapas com chave física ou passkey na sua conta.
- Em **Settings → Rules → Rulesets**, crie uma regra para as tags `v*` que restrinja criar,
  alterar e apagar, e proteja a branch `main`.
- No ambiente `release`, ative **Required reviewers** (você mesmo) se quiser um clique de
  aprovação antes de cada publicação.
- O workflow já recusa uma tag cujo commit não esteja na `main`.

## Se a chave privada se perder ou vazar

- **Perdida:** nenhum app instalado aceita atualizações de outra chave. Gere uma chave nova,
  troque a `pubkey` e os segredos, publique uma versão nova e peça para reinstalar à mão.
- **Vazou:** gere uma chave nova imediatamente e troque os segredos. Para os apps instalados
  adotarem a chave nova, publique **uma versão de transição assinada com a chave antiga**, já com
  a `pubkey` nova no `tauri.conf.json`. Revogue tokens do GitHub e apague releases suspeitos. Se
  você já não controla mais a chave antiga, a recuperação é reinstalar à mão.

## A cada versão

```powershell
npm run version:bump -- 0.1.1     # troca a versão nos 4 arquivos
git commit -am "chore: release 0.1.1"
git push
git tag v0.1.1
git push origin v0.1.1             # dispara o workflow Release
```

O workflow `Release` roda os testes e a auditoria, confere se a tag é igual à versão do app, gera
o instalador assinado e publica o release com o `.exe`, o `.sig` e o `latest.json`. Se algum teste
falhar, nada é publicado.

## Ícone

Para trocar o ícone, gere um PNG quadrado de 1024 px e rode
`npm run tauri icon seu-arquivo.png` (`scripts/make-icon.ps1` desenha o ícone atual).

// Troca a versão do app nos quatro lugares onde ela mora, de uma vez:
//   package.json, src-tauri/Cargo.toml, src-tauri/Cargo.lock e src-tauri/tauri.conf.json.
// Uso: npm run version:bump -- 0.1.1
// Depois: commitar e criar a tag `v0.1.1` (o workflow de release confere se tag e versão batem).
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = (path) => fileURLToPath(new URL(`../${path}`, import.meta.url));

const SEMVER = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;
const CARGO_PACKAGE = "lang";

export function isValidVersion(version) {
  return typeof version === "string" && SEMVER.test(version);
}

/** Troca a primeira `"version": "…"` de um JSON (a do nível de cima), sem reformatar o resto. */
export function replaceJsonVersion(text, version) {
  const pattern = /("version"\s*:\s*")[^"]*(")/;
  if (!pattern.test(text)) {
    throw new Error('Campo "version" não encontrado.');
  }
  return text.replace(pattern, `$1${version}$2`);
}

/** Troca a versão da seção `[package]` de um `Cargo.toml`. */
export function replaceCargoVersion(toml, version) {
  const section = /^\[package\]([\s\S]*?)(?=^\[|$(?![\s\S]))/m.exec(toml);
  if (!section) {
    throw new Error("Seção [package] não encontrada.");
  }
  const field = /^(version\s*=\s*")[^"]*(")/m;
  if (!field.test(section[1])) {
    throw new Error("Campo version não encontrado em [package].");
  }
  const updated = section[1].replace(field, `$1${version}$2`);
  return toml.replace(section[0], `[package]${updated}`);
}

/** Troca a versão do próprio app no `Cargo.lock` (as das dependências ficam como estão). */
export function replaceLockVersion(lock, name, version) {
  const entry = new RegExp(`(name = "${name}"\\r?\\nversion = ")[^"]*(")`);
  if (!entry.test(lock)) {
    throw new Error(`Pacote ${name} não encontrado no Cargo.lock.`);
  }
  return lock.replace(entry, `$1${version}$2`);
}

const FILES = [
  ["package.json", (text, version) => replaceJsonVersion(text, version)],
  ["src-tauri/tauri.conf.json", (text, version) => replaceJsonVersion(text, version)],
  ["src-tauri/Cargo.toml", (text, version) => replaceCargoVersion(text, version)],
  ["src-tauri/Cargo.lock", (text, version) => replaceLockVersion(text, CARGO_PACKAGE, version)],
];

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const version = process.argv[2];
  if (!isValidVersion(version)) {
    console.error("Informe uma versão no formato 1.2.3 (ex.: npm run version:bump -- 0.1.1).");
    process.exit(1);
  }
  // Calcula tudo antes de gravar: se um arquivo estiver fora do padrão, nenhum é alterado.
  const updated = FILES.map(([file, replace]) => [
    file,
    replace(readFileSync(root(file), "utf8"), version),
  ]);
  for (const [file, content] of updated) {
    writeFileSync(root(file), content);
  }
  console.log(`Versão ${version} gravada em ${updated.length} arquivos.`);
  console.log(`Próximo passo: commit, e depois  git tag v${version}  e  git push origin v${version}`);
}

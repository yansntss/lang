// Confere se a versão do app é a mesma nos três lugares onde ela mora. Roda no CI e antes de
// gerar o instalador: o atualizador compara a versão do `tauri.conf.json`, e uma divergência
// publicaria um instalador com número errado.
import { readFileSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = (path) => fileURLToPath(new URL(`../${path}`, import.meta.url));

/** Versão do pacote `[package]` de um `Cargo.toml`, ou `null` se não achar. */
export function cargoVersion(toml) {
  const section = /^\[package\]([\s\S]*?)(?=^\[|$(?![\s\S]))/m.exec(toml);
  const match = section && /^version\s*=\s*"([^"]+)"/m.exec(section[1]);
  return match ? match[1] : null;
}

/**
 * Compara as versões e devolve uma mensagem de erro, ou `null` se estão todas iguais.
 * `versions` é `{ "package.json": "0.1.0", ... }`; um valor ausente conta como divergência.
 */
export function findVersionMismatch(versions) {
  const entries = Object.entries(versions);
  const missing = entries.filter(([, version]) => !version).map(([file]) => file);
  if (missing.length > 0) {
    return `Versão não encontrada em: ${missing.join(", ")}.`;
  }
  const distinct = new Set(entries.map(([, version]) => version));
  if (distinct.size === 1) {
    return null;
  }
  const lines = entries.map(([file, version]) => `  ${file}: ${version}`);
  return `As versões divergem:\n${lines.join("\n")}`;
}

function readVersions() {
  return {
    "package.json": JSON.parse(readFileSync(root("package.json"), "utf8")).version,
    "src-tauri/Cargo.toml": cargoVersion(readFileSync(root("src-tauri/Cargo.toml"), "utf8")),
    "src-tauri/tauri.conf.json": JSON.parse(
      readFileSync(root("src-tauri/tauri.conf.json"), "utf8"),
    ).version,
  };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const versions = readVersions();
  const problem = findVersionMismatch(versions);
  if (problem) {
    console.error(problem);
    process.exit(1);
  }
  console.log(`Versão ${Object.values(versions)[0]} em sincronia nos três arquivos.`);
}

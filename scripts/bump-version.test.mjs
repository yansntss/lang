import { describe, expect, it } from "vitest";
import {
  isValidVersion,
  replaceCargoVersion,
  replaceJsonVersion,
  replaceLockVersion,
} from "./bump-version.mjs";

describe("isValidVersion", () => {
  it("accepts plain and pre-release versions", () => {
    for (const version of ["0.1.0", "1.20.300", "1.0.0-rc.1", "2.0.0-beta"]) {
      expect(isValidVersion(version), version).toBe(true);
    }
  });

  it("rejects anything else, including a leading v and shell-looking text", () => {
    for (const version of [
      "",
      "1",
      "1.2",
      "v1.2.3",
      "1.2.3.4",
      "1.2.x",
      "1.2.3; rm -rf /",
      undefined,
    ]) {
      expect(isValidVersion(version), String(version)).toBe(false);
    }
  });
});

describe("replaceJsonVersion", () => {
  it("changes only the top-level version and keeps the formatting", () => {
    const text = `{\n  "name": "x",\n  "version": "0.1.0",\n  "deps": { "version": "9" }\n}\n`;

    const result = replaceJsonVersion(text, "0.2.0");

    expect(result).toBe(
      `{\n  "name": "x",\n  "version": "0.2.0",\n  "deps": { "version": "9" }\n}\n`,
    );
  });

  it("fails when there is no version field", () => {
    expect(() => replaceJsonVersion(`{"name":"x"}`, "1.0.0")).toThrow(/version/);
  });
});

describe("replaceCargoVersion", () => {
  const toml = `[package]\nname = "x"\nversion = "0.1.0"\n\n[dependencies]\nserde = { version = "1" }\nother = "2"\n`;

  it("changes the package version and not the dependencies", () => {
    const result = replaceCargoVersion(toml, "0.3.0");

    expect(result).toContain(`version = "0.3.0"`);
    expect(result).toContain(`serde = { version = "1" }`);
    expect(result).not.toContain(`version = "0.1.0"`);
  });

  it("fails without a [package] section", () => {
    expect(() => replaceCargoVersion(`[workspace]\n`, "1.0.0")).toThrow(/package/);
  });
});

describe("replaceLockVersion", () => {
  const lock = `[[package]]\nname = "serde"\nversion = "1.0.0"\n\n[[package]]\nname = "lang-app"\nversion = "0.1.0"\ndependencies = [\n "serde",\n]\n`;

  it("changes only the app entry", () => {
    const result = replaceLockVersion(lock, "lang-app", "0.2.0");

    expect(result).toContain(`name = "lang-app"\nversion = "0.2.0"`);
    expect(result).toContain(`name = "serde"\nversion = "1.0.0"`);
  });

  it("works with Windows line endings", () => {
    const crlf = lock.replace(/\n/g, "\r\n");

    expect(replaceLockVersion(crlf, "lang-app", "0.2.0")).toContain(`version = "0.2.0"`);
  });

  it("fails when the package is missing", () => {
    expect(() => replaceLockVersion(lock, "outro", "1.0.0")).toThrow(/outro/);
  });
});

import { describe, expect, it } from "vitest";
import { decodeSnapshot } from "./tauri";

function encoded(width: number, height: number, pixels: number[]): ArrayBuffer {
  const buffer = new ArrayBuffer(8 + pixels.length);
  const view = new DataView(buffer);
  view.setUint32(0, width, true);
  view.setUint32(4, height, true);
  new Uint8Array(buffer, 8).set(pixels);
  return buffer;
}

describe("decodeSnapshot", () => {
  it("reads the width, the height and the RGBA pixels after them", () => {
    const snapshot = decodeSnapshot(encoded(2, 1, [1, 2, 3, 255, 4, 5, 6, 255]));

    expect(snapshot.width).toBe(2);
    expect(snapshot.height).toBe(1);
    expect(Array.from(snapshot.pixels)).toEqual([1, 2, 3, 255, 4, 5, 6, 255]);
  });

  it("reads dimensions larger than a byte", () => {
    const snapshot = decodeSnapshot(encoded(300, 2, new Array(300 * 2 * 4).fill(0)));

    expect(snapshot.width).toBe(300);
    expect(snapshot.height).toBe(2);
  });

  it("refuses a buffer shorter than the header", () => {
    expect(() => decodeSnapshot(new ArrayBuffer(4))).toThrow("Captura da tela inválida.");
  });

  it("refuses pixels that do not match the dimensions", () => {
    expect(() => decodeSnapshot(encoded(2, 2, [0, 0, 0, 255]))).toThrow(
      "Captura da tela inválida.",
    );
  });
});

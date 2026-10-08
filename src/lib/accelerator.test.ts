import { describe, expect, it } from "vitest";
import { acceleratorFromEvent, isModifierKey, type KeyEventLike } from "./accelerator";

function press(code: string, modifiers: Partial<KeyEventLike> = {}): KeyEventLike {
  return {
    code,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    ...modifiers,
  };
}

describe("acceleratorFromEvent", () => {
  it("builds the text with the modifiers in a fixed order", () => {
    const text = acceleratorFromEvent(
      press("KeyT", { altKey: true, ctrlKey: true, shiftKey: true, metaKey: true }),
    );

    expect(text).toBe("Ctrl+Alt+Shift+Win+T");
  });

  it("accepts letters, digits and function keys", () => {
    expect(acceleratorFromEvent(press("KeyJ", { ctrlKey: true }))).toBe("Ctrl+J");
    expect(acceleratorFromEvent(press("Digit5", { altKey: true }))).toBe("Alt+5");
    expect(acceleratorFromEvent(press("F1", { ctrlKey: true }))).toBe("Ctrl+F1");
    expect(acceleratorFromEvent(press("F12", { ctrlKey: true }))).toBe("Ctrl+F12");
  });

  it("returns the key alone when no modifier is held (the backend rejects it)", () => {
    expect(acceleratorFromEvent(press("KeyT"))).toBe("T");
  });

  it("rejects keys the shortcut cannot use", () => {
    for (const code of ["Enter", "Space", "Escape", "ArrowUp", "F13", "F0", "Numpad1", "Minus"]) {
      expect(acceleratorFromEvent(press(code, { ctrlKey: true })), code).toBeNull();
    }
  });
});

describe("isModifierKey", () => {
  it("recognizes modifier keys, which only change the held state", () => {
    for (const code of [
      "ControlLeft",
      "ControlRight",
      "AltLeft",
      "ShiftRight",
      "MetaLeft",
      "OSLeft",
    ]) {
      expect(isModifierKey(code), code).toBe(true);
    }
  });

  it("does not take a main key for a modifier", () => {
    for (const code of ["KeyT", "Digit1", "F5", "Escape"]) {
      expect(isModifierKey(code), code).toBe(false);
    }
  });
});

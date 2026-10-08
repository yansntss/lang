/** O que `acceleratorFromEvent` usa de um `KeyboardEvent`. */
export interface KeyEventLike {
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

const LETTER = /^Key([A-Z])$/;
const DIGIT = /^Digit([0-9])$/;
const FUNCTION_KEY = /^F([1-9]|1[0-2])$/;
const MODIFIER_KEY = /^(Control|Alt|Shift|Meta|OS)(Left|Right)$/;

/** Teclas que só mudam o estado dos modificadores: aguardar a tecla principal. */
export function isModifierKey(code: string): boolean {
  return MODIFIER_KEY.test(code);
}

function mainKey(code: string): string | null {
  const match = LETTER.exec(code) ?? DIGIT.exec(code);
  if (match) {
    return match[1];
  }
  return FUNCTION_KEY.test(code) ? code : null;
}

/**
 * Monta o texto do atalho (`"Ctrl+Alt+T"`) a partir de uma tecla apertada. Devolve `null` se a
 * tecla principal não é aceita (só letras, números e F1–F12 valem). O backend valida de novo.
 */
export function acceleratorFromEvent(event: KeyEventLike): string | null {
  const key = mainKey(event.code);
  if (key === null) {
    return null;
  }
  const modifiers = [
    event.ctrlKey && "Ctrl",
    event.altKey && "Alt",
    event.shiftKey && "Shift",
    event.metaKey && "Win",
  ].filter((name): name is string => name !== false);
  return [...modifiers, key].join("+");
}

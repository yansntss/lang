import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { SecretKind, Translation } from "./types";

// Único ponto do front que fala com o backend. Nenhum comando devolve chaves de API.

export const POPUP_RESET_EVENT = "popup://reset";

const FALLBACK_ERROR_MESSAGE = "Algo deu errado. Tente novamente.";

export function translate(text: string): Promise<Translation> {
  return invoke<Translation>("translate", { text });
}

export function copyToClipboard(text: string): Promise<void> {
  return invoke<void>("copy_to_clipboard", { text });
}

export function hidePopup(): Promise<void> {
  return invoke<void>("hide_popup");
}

export function setSecret(kind: SecretKind, value: string): Promise<void> {
  return invoke<void>("set_secret", { kind, value });
}

export function hasSecret(kind: SecretKind): Promise<boolean> {
  return invoke<boolean>("has_secret", { kind });
}

export function deleteSecret(kind: SecretKind): Promise<void> {
  return invoke<void>("delete_secret", { kind });
}

/** Devolve a função que cancela a escuta. */
export function onPopupReset(handler: () => void): Promise<() => void> {
  return listen(POPUP_RESET_EVENT, () => handler());
}

/** Extrai uma mensagem exibível de um erro do backend ou de qualquer valor lançado. */
export function errorMessage(error: unknown): string {
  if (typeof error === "object" && error !== null && "message" in error) {
    const { message } = error as { message: unknown };
    if (typeof message === "string" && message.length > 0) {
      return message;
    }
  }
  return FALLBACK_ERROR_MESSAGE;
}

import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ExplainEvent, PopupResetPayload, SecretKind, Translation } from "./types";

// Único ponto do front que fala com o backend. Nenhum comando devolve chaves de API.

export const POPUP_RESET_EVENT = "popup://reset";

const FALLBACK_ERROR_MESSAGE = "Algo deu errado. Tente novamente.";

export function translate(text: string): Promise<Translation> {
  return invoke<Translation>("translate", { text });
}

export function copyToClipboard(text: string): Promise<void> {
  return invoke<void>("copy_to_clipboard", { text });
}

/**
 * Pede a explicação de `text` e `translation`. O texto chega em `onEvent` (deltas, depois
 * `done` ou `error`). Devolve o id da explicação, usado para cancelá-la.
 */
export function explain(
  text: string,
  translation: string,
  onEvent: (event: ExplainEvent) => void,
): Promise<number> {
  const channel = new Channel<ExplainEvent>();
  channel.onmessage = onEvent;
  return invoke<number>("explain", { text, translation, onEvent: channel });
}

export function cancelExplain(id: number): Promise<void> {
  return invoke<void>("cancel_explain", { id });
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

/**
 * O popup foi exibido de novo. `prefill` é o texto que o usuário tinha selecionado em outro
 * app, ou `null` quando não houve captura. Devolve a função que cancela a escuta.
 */
export function onPopupReset(handler: (prefill: string | null) => void): Promise<() => void> {
  return listen<PopupResetPayload>(POPUP_RESET_EVENT, (event) => handler(event.payload.prefill));
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

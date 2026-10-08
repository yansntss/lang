import { useEffect, useRef } from "react";
import { errorMessage, hidePopup, onPopupReset } from "../lib/tauri";

interface PopupLifecycleHandlers {
  /** O popup foi exibido de novo: focar o campo e preenchê-lo com o texto capturado, se houver. */
  onReset: (prefill: string | null) => void;
  onError: (message: string) => void;
}

/** Escuta o evento de reset do backend e fecha o popup com Esc. */
export function usePopupLifecycle({ onReset, onError }: PopupLifecycleHandlers): void {
  const handlers = useRef({ onReset, onError });
  useEffect(() => {
    handlers.current = { onReset, onError };
  });

  useEffect(() => {
    let disposed = false;
    let stopListening: (() => void) | undefined;

    onPopupReset((prefill) => handlers.current.onReset(prefill)).then(
      (stop) => {
        if (disposed) {
          stop();
        } else {
          stopListening = stop;
        }
      },
      (error: unknown) => handlers.current.onError(errorMessage(error)),
    );

    const onKeyDown = (event: KeyboardEvent) => {
      // Esc durante uma composição de IME cancela a composição, não o popup.
      if (event.key === "Escape" && !event.isComposing) {
        hidePopup().catch((error: unknown) => handlers.current.onError(errorMessage(error)));
      }
    };
    document.addEventListener("keydown", onKeyDown);

    return () => {
      disposed = true;
      stopListening?.();
      document.removeEventListener("keydown", onKeyDown);
    };
  }, []);
}

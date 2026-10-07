import { useEffect, useState } from "react";
import { errorMessage, translate } from "../lib/tauri";
import type { Translation } from "../lib/types";

export const TRANSLATE_DEBOUNCE_MS = 400;

export type TranslateState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "success"; sourceText: string; translation: Translation }
  | { status: "error"; message: string };

/**
 * Traduz `text` após uma pausa na digitação. Respostas de pedidos antigos são descartadas,
 * então o resultado exibido sempre corresponde ao último texto enviado.
 */
export function useTranslate(
  text: string,
  delayMs: number = TRANSLATE_DEBOUNCE_MS,
): TranslateState {
  const [state, setState] = useState<TranslateState>({ status: "idle" });

  useEffect(() => {
    const sourceText = text.trim();
    if (sourceText === "") {
      setState({ status: "idle" });
      return;
    }

    let cancelled = false;
    const timer = setTimeout(() => {
      setState({ status: "loading" });
      translate(sourceText).then(
        (translation) => {
          if (!cancelled) {
            setState({ status: "success", sourceText, translation });
          }
        },
        (error: unknown) => {
          if (!cancelled) {
            setState({ status: "error", message: errorMessage(error) });
          }
        },
      );
    }, delayMs);

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [text, delayMs]);

  return state;
}

import { useEffect, useState } from "react";
import { errorMessage, translate } from "../lib/tauri";
import type { Translation } from "../lib/types";

export const TRANSLATE_DEBOUNCE_MS = 400;

const IDLE: TranslateState = { status: "idle" };

export type TranslateState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "success"; sourceText: string; translation: Translation }
  | { status: "error"; message: string };

/**
 * Traduz `text` após uma pausa na digitação. Respostas de pedidos antigos são descartadas,
 * então o resultado exibido sempre corresponde ao último texto enviado. Com `enabled` falso
 * nada é enviado (o texto ainda não foi confirmado pelo usuário).
 */
export function useTranslate(
  text: string,
  delayMs: number = TRANSLATE_DEBOUNCE_MS,
  enabled: boolean = true,
): TranslateState {
  const [state, setState] = useState<TranslateState>(IDLE);
  // O efeito depende do texto aparado: espaços nas pontas não reiniciam o debounce nem
  // geram um novo pedido idêntico (a cota do DeepL Free é por caractere).
  const sourceText = text.trim();
  const idle = sourceText === "" || !enabled;

  useEffect(() => {
    if (idle) {
      setState(IDLE);
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
  }, [sourceText, delayMs, idle]);

  // Sem texto (ou retido) é sempre "idle", sem esperar o efeito: evita piscar o resultado antigo
  // após um reset.
  return idle ? IDLE : state;
}

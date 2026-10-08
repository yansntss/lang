import { useCallback, useEffect, useRef, useState } from "react";
import { cancelExplain, errorMessage, explain } from "../lib/tauri";
import type { ExplainEvent } from "../lib/types";

export type ExplainState =
  | { status: "idle" }
  | { status: "streaming"; text: string }
  | { status: "done"; text: string }
  | { status: "error"; text: string; message: string };

const IDLE: ExplainState = { status: "idle" };

export interface UseExplain {
  state: ExplainState;
  start: (text: string, translation: string) => void;
  /** Cancela a explicação em andamento (se houver) e volta ao estado inicial. */
  cancel: () => void;
}

function applyEvent(state: ExplainState, event: ExplainEvent): ExplainState {
  const text = state.status === "idle" ? "" : state.text;
  switch (event.kind) {
    case "delta":
      return { status: "streaming", text: text + event.text };
    case "done":
      return { status: "done", text };
    case "error":
      return { status: "error", text, message: event.message };
  }
}

/**
 * Gerencia uma explicação por vez. Eventos de uma explicação já cancelada ou substituída são
 * descartados, e o backend é avisado para parar de gerar.
 */
export function useExplain(): UseExplain {
  const [state, setState] = useState<ExplainState>(IDLE);
  // Cada início ou cancelamento muda `run`: eventos de um `run` antigo são ignorados.
  const run = useRef(0);
  const activeId = useRef<number | null>(null);

  const stopBackend = useCallback(() => {
    run.current += 1;
    const id = activeId.current;
    activeId.current = null;
    if (id !== null) {
      cancelExplain(id).catch(() => {});
    }
  }, []);

  const cancel = useCallback(() => {
    stopBackend();
    setState(IDLE);
  }, [stopBackend]);

  const start = useCallback(
    (text: string, translation: string) => {
      stopBackend();
      const thisRun = run.current;
      setState({ status: "streaming", text: "" });

      explain(text, translation, (event) => {
        if (thisRun === run.current) {
          setState((current) => applyEvent(current, event));
        }
      }).then(
        (id) => {
          if (thisRun === run.current) {
            activeId.current = id;
          } else {
            cancelExplain(id).catch(() => {});
          }
        },
        (error: unknown) => {
          if (thisRun === run.current) {
            setState({ status: "error", text: "", message: errorMessage(error) });
          }
        },
      );
    },
    [stopBackend],
  );

  useEffect(() => stopBackend, [stopBackend]);

  return { state, start, cancel };
}

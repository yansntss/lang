import { useCallback, useEffect, useRef, useState } from "react";
import { errorMessage, markReviewed, nextReview } from "../../lib/tauri";
import type { HistoryEntry } from "../../lib/types";

type ReviewState =
  | { status: "loading" }
  | { status: "empty" }
  | { status: "card"; entry: HistoryEntry; revealed: boolean; advancing: boolean }
  | { status: "error"; message: string };

/** Revisão dos favoritos: mostra o original, revela a tradução e passa para o próximo. */
export default function ReviewCard() {
  const [state, setState] = useState<ReviewState>({ status: "loading" });
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const load = useCallback(() => {
    setState({ status: "loading" });
    nextReview().then(
      (entry) => {
        if (!mounted.current) return;
        setState(
          entry
            ? { status: "card", entry, revealed: false, advancing: false }
            : { status: "empty" },
        );
      },
      (failure: unknown) => {
        if (mounted.current) setState({ status: "error", message: errorMessage(failure) });
      },
    );
  }, []);

  useEffect(load, [load]);

  if (state.status === "loading") {
    return <p className="history__hint">Carregando…</p>;
  }
  if (state.status === "empty") {
    return (
      <p className="history__hint">Favorite traduções na aba Histórico para revisá-las aqui.</p>
    );
  }
  if (state.status === "error") {
    return (
      <>
        <p role="alert" className="history__error">
          {state.message}
        </p>
        <button type="button" onClick={load}>
          Tentar de novo
        </button>
      </>
    );
  }

  const { entry, revealed, advancing } = state;
  // O botão fica desabilitado enquanto avança: um duplo clique não pode pular um cartão.
  const next = () => {
    setState({ ...state, advancing: true });
    markReviewed(entry.id).then(
      () => {
        if (mounted.current) load();
      },
      (failure: unknown) => {
        if (mounted.current) setState({ status: "error", message: errorMessage(failure) });
      },
    );
  };

  return (
    <section className="history__card" aria-label="Cartão de revisão">
      <p className="history__source">{entry.sourceText}</p>
      {revealed ? (
        <>
          <p className="history__translation" aria-live="polite">
            {entry.translatedText}
          </p>
          <button type="button" disabled={advancing} onClick={next}>
            Próximo
          </button>
        </>
      ) : (
        <button type="button" onClick={() => setState({ ...state, revealed: true })}>
          Mostrar tradução
        </button>
      )}
    </section>
  );
}

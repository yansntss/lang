import type { ExplainState } from "../../hooks/useExplain";

interface ExplainPanelProps {
  state: ExplainState;
}

/** A resposta da IA é exibida como texto puro: nunca como HTML. */
export default function ExplainPanel({ state }: ExplainPanelProps) {
  if (state.status === "idle") {
    return null;
  }

  return (
    <section className="popup__explanation" aria-label="Explicação" aria-live="polite">
      {state.text !== "" && <p className="popup__explanation-text">{state.text}</p>}
      {state.status === "streaming" && <p className="popup__hint">Explicando…</p>}
      {state.status === "error" && (
        <p role="alert" className="popup__error">
          {state.message}
        </p>
      )}
    </section>
  );
}

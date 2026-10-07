import type { TranslateState } from "../../hooks/useTranslate";

interface TranslationResultProps {
  state: TranslateState;
}

export default function TranslationResult({ state }: TranslationResultProps) {
  switch (state.status) {
    case "idle":
      return <p className="popup__hint">Enter copia · Esc fecha</p>;
    case "loading":
      return <p className="popup__hint">Traduzindo…</p>;
    case "error":
      return (
        <p role="alert" className="popup__error">
          {state.message}
        </p>
      );
    case "success":
      return (
        <>
          <p className="popup__translation">{state.translation.text}</p>
          <p className="popup__meta">
            {state.translation.sourceLang} → {state.translation.targetLang}
          </p>
        </>
      );
  }
}

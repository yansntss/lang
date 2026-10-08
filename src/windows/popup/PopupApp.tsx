import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useExplain } from "../../hooks/useExplain";
import { useHasSecret } from "../../hooks/useHasSecret";
import { usePopupLifecycle } from "../../hooks/usePopupLifecycle";
import { useTranslate } from "../../hooks/useTranslate";
import { copyToClipboard, errorMessage, hidePopup } from "../../lib/tauri";
import "../../styles/base.css";
import ExplainPanel from "./ExplainPanel";
import TranslationResult from "./TranslationResult";
import "./popup.css";

export default function PopupApp() {
  const [text, setText] = useState("");
  const [actionError, setActionError] = useState<string | null>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const result = useTranslate(text);
  const explanation = useExplain();
  const cancelExplanation = explanation.cancel;
  const canExplain = useHasSecret("anthropic");

  // Texto capturado da seleção do usuário (se houver) substitui o que estava no campo, e a
  // tradução começa sozinha pelo mesmo caminho da digitação.
  const reset = useCallback(
    (prefill: string | null) => {
      cancelExplanation();
      setText(prefill ?? "");
      setActionError(null);
      inputRef.current?.focus();
    },
    [cancelExplanation],
  );
  usePopupLifecycle({ onReset: reset, onError: setActionError });

  // A tradução que está na tela, só se ainda corresponde ao texto do campo.
  const currentTranslation =
    result.status === "success" && result.sourceText === text.trim() ? result.translation : null;

  // A explicação pertence a uma tradução: trocou o texto, ou o popup perdeu o foco (e vai ser
  // ocultado), então ela é cancelada para não gastar cota em segundo plano.
  useEffect(() => {
    cancelExplanation();
  }, [currentTranslation, cancelExplanation]);
  useEffect(() => {
    window.addEventListener("blur", cancelExplanation);
    return () => window.removeEventListener("blur", cancelExplanation);
  }, [cancelExplanation]);

  // Só o clique do usuário envia o texto ao provedor de IA.
  const explainTranslation = () => {
    if (currentTranslation) {
      explanation.start(text.trim(), currentTranslation.text);
    }
  };

  // O popup é ocultado e reexibido, não remontado: `autoFocus` só vale na primeira vez.
  // Sempre que a janela recebe o foco do sistema, devolve o foco ao campo.
  useEffect(() => {
    const focusField = () => inputRef.current?.focus();
    window.addEventListener("focus", focusField);
    return () => window.removeEventListener("focus", focusField);
  }, []);

  // Só copia se a tradução exibida corresponde ao texto atual do campo.
  const copyAndClose = async () => {
    if (result.status !== "success" || result.sourceText !== text.trim()) {
      return;
    }
    try {
      await copyToClipboard(result.translation.text);
      await hidePopup();
    } catch (error: unknown) {
      setActionError(errorMessage(error));
    }
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key !== "Enter" || event.shiftKey || event.nativeEvent.isComposing) {
      return;
    }
    // Enter nunca insere quebra de linha, mas segurar a tecla não deve copiar várias vezes.
    event.preventDefault();
    if (event.repeat) {
      return;
    }
    void copyAndClose();
  };

  return (
    <main className="popup">
      <textarea
        ref={inputRef}
        className="popup__input"
        aria-label="Texto para traduzir"
        placeholder="Digite ou cole um texto…"
        rows={3}
        spellCheck={false}
        autoFocus
        value={text}
        onChange={(event) => {
          setText(event.target.value);
          setActionError(null);
        }}
        onKeyDown={handleKeyDown}
      />
      <section className="popup__result" aria-live="polite">
        <TranslationResult state={result} />
        {currentTranslation && (
          <div className="popup__explain">
            <button
              type="button"
              className="popup__explain-button"
              disabled={!canExplain || explanation.state.status === "streaming"}
              onClick={explainTranslation}
            >
              Explicar
            </button>
            {!canExplain && (
              <p className="popup__hint">
                Configure a chave da Anthropic em Configurações para usar o Explicar.
              </p>
            )}
          </div>
        )}
        <ExplainPanel state={explanation.state} />
        {actionError && (
          <p role="alert" className="popup__error">
            {actionError}
          </p>
        )}
      </section>
    </main>
  );
}

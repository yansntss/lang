import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from "react";
import { usePopupLifecycle } from "../../hooks/usePopupLifecycle";
import { useTranslate } from "../../hooks/useTranslate";
import { copyToClipboard, errorMessage, hidePopup } from "../../lib/tauri";
import "../../styles/base.css";
import TranslationResult from "./TranslationResult";
import "./popup.css";

export default function PopupApp() {
  const [text, setText] = useState("");
  const [actionError, setActionError] = useState<string | null>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const result = useTranslate(text);

  const reset = useCallback(() => {
    setText("");
    setActionError(null);
    inputRef.current?.focus();
  }, []);
  usePopupLifecycle({ onReset: reset, onError: setActionError });

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
        {actionError && (
          <p role="alert" className="popup__error">
            {actionError}
          </p>
        )}
      </section>
    </main>
  );
}

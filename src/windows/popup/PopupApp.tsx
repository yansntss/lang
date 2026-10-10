import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useExplain } from "../../hooks/useExplain";
import { useFitPopupHeight } from "../../hooks/useFitPopupHeight";
import { useHasSecret } from "../../hooks/useHasSecret";
import { usePopupLifecycle } from "../../hooks/usePopupLifecycle";
import { useTheme } from "../../hooks/useTheme";
import { useTranslate } from "../../hooks/useTranslate";
import { useWindowDrag } from "../../hooks/useWindowDrag";
import {
  copyToClipboard,
  errorMessage,
  hidePopup,
  recordHistory,
  setPopupPinned,
} from "../../lib/tauri";
import type { Translation } from "../../lib/types";
import "../../styles/base.css";
import ExplainPanel from "./ExplainPanel";
import langIcon from "../../../assets/icon/lang-icon.svg";
import PinButton from "./PinButton";
import ResizeGrips from "./ResizeGrips";
import TranslationResult from "./TranslationResult";
import "./popup.css";

export default function PopupApp() {
  const [text, setText] = useState("");
  const [actionError, setActionError] = useState<string | null>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const popupRef = useRef<HTMLElement>(null);
  const resultRef = useRef<HTMLElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const lastRecorded = useRef<string | null>(null);
  // Texto capturado que o usuário pediu para só traduzir depois do Enter (configurações).
  const [held, setHeld] = useState(false);
  // Fixado, o popup não some ao perder o foco, e o Enter copia sem fechar.
  const [pinned, setPinned] = useState(false);
  const [copied, setCopied] = useState(false);
  const result = useTranslate(text, undefined, !held);
  const explanation = useExplain();
  const cancelExplanation = explanation.cancel;
  const canExplain = useHasSecret("anthropic");
  useTheme();
  useFitPopupHeight(popupRef, resultRef, contentRef);
  useWindowDrag(popupRef);

  // Texto capturado da seleção do usuário (se houver) substitui o que estava no campo, e a
  // tradução começa sozinha pelo mesmo caminho da digitação, a menos que o usuário tenha
  // pedido para traduzir só depois do Enter. `stillPinned` vem do backend: o popup só continua
  // fixado se não foi ocultado entre uma exibição e outra.
  const reset = useCallback(
    (prefill: string | null, autoTranslate: boolean = true, stillPinned: boolean = false) => {
      cancelExplanation();
      lastRecorded.current = null;
      // Seleção em branco não tem o que traduzir: reter só a prenderia em "Enter traduz".
      setHeld(prefill !== null && prefill.trim() !== "" && !autoTranslate);
      setText(prefill ?? "");
      setActionError(null);
      setCopied(false);
      setPinned(stillPinned);
      inputRef.current?.focus();
    },
    [cancelExplanation],
  );

  const togglePin = () => {
    const next = !pinned;
    setPinned(next);
    setPopupPinned(next).catch((error: unknown) => {
      setPinned(!next);
      setActionError(errorMessage(error));
    });
  };
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

  // A tradução só entra no histórico quando o usuário a confirma (Enter ou Explicar): a do
  // popup sai sozinha enquanto se digita, e gravá-las todas encheria o histórico de rascunhos.
  // O backend registra a falha em log, e ela não deve atrapalhar a cópia nem a explicação.
  const remember = (sourceText: string, translation: Translation) => {
    // Explicar e depois Enter (ou cliques repetidos) são a mesma tradução: conta uma vez só.
    const key = `${sourceText}\u0000${translation.text}`;
    if (lastRecorded.current === key) {
      return;
    }
    lastRecorded.current = key;
    recordHistory({
      sourceText,
      translatedText: translation.text,
      sourceLang: translation.sourceLang,
      targetLang: translation.targetLang,
    }).catch(() => {});
  };

  // Só o clique do usuário envia o texto ao provedor de IA.
  const explainTranslation = () => {
    if (currentTranslation) {
      remember(text.trim(), currentTranslation);
      explanation.start(text.trim(), currentTranslation.text);
      // O botão fica desabilitado durante a resposta e o foco se perderia: devolvê-lo ao campo
      // mantém o Enter copiando a tradução.
      inputRef.current?.focus();
    }
  };

  // O popup é ocultado e reexibido, não remontado: `autoFocus` só vale na primeira vez.
  // Sempre que a janela recebe o foco do sistema, devolve o foco ao campo.
  useEffect(() => {
    const focusField = () => inputRef.current?.focus();
    window.addEventListener("focus", focusField);
    return () => window.removeEventListener("focus", focusField);
  }, []);

  // Só copia se a tradução exibida corresponde ao texto atual do campo. Fixado, o popup fica
  // aberto e só avisa que copiou.
  const copyTranslation = async () => {
    if (result.status !== "success" || result.sourceText !== text.trim()) {
      return;
    }
    remember(result.sourceText, result.translation);
    try {
      await copyToClipboard(result.translation.text);
      if (pinned) {
        setCopied(true);
      } else {
        await hidePopup();
      }
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
    // Com a tradução retida, o primeiro Enter só a libera; o seguinte copia.
    if (held) {
      setHeld(false);
      return;
    }
    void copyTranslation();
  };

  return (
    <main ref={popupRef} className="popup">
      <ResizeGrips />
      <div className="popup__corner">
        <img className="popup__logo" src={langIcon} alt="Lang" width={14} height={14} />
        <PinButton pinned={pinned} onToggle={togglePin} />
      </div>
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
          setHeld(false);
          setCopied(false);
        }}
        onKeyDown={handleKeyDown}
      />
      <section ref={resultRef} className="popup__result" aria-live="polite">
        <div ref={contentRef} className="popup__content">
          {held ? (
            <p className="popup__hint">Enter traduz · Esc fecha</p>
          ) : (
            <TranslationResult state={result} />
          )}
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
          {copied && <p className="popup__hint">Copiado</p>}
          <ExplainPanel state={explanation.state} />
          {actionError && (
            <p role="alert" className="popup__error">
              {actionError}
            </p>
          )}
        </div>
      </section>
    </main>
  );
}

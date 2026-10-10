import { useCallback, useEffect, useState } from "react";
import { acceleratorFromEvent, isModifierKey } from "../../lib/accelerator";
import { errorMessage, setShortcut } from "../../lib/tauri";
import type { SettingsView, ShortcutSlot } from "../../lib/types";

interface Feedback {
  kind: "success" | "error";
  text: string;
}

interface ShortcutSectionProps {
  /** Qual atalho esta seção troca. */
  slot: ShortcutSlot;
  title: string;
  /** Para que serve o atalho, mostrado acima do atual. */
  description?: string;
  shortcut: string;
  onChanged: (view: SettingsView) => void;
}

const UNSUPPORTED_KEY = "Use uma letra, um número ou F1 a F12 como tecla do atalho.";
const NEEDS_MODIFIER = "Use Ctrl, Alt ou Win junto da tecla do atalho.";
const NAVIGATION_KEYS = ["Tab", "Enter", "NumpadEnter", "Space"];

/** O texto salvo para `slot` no que o backend devolveu. */
function shortcutOf(view: SettingsView, slot: ShortcutSlot): string {
  return slot === "ocr" ? view.ocrShortcut : view.shortcut;
}

/**
 * Mostra um atalho global e o troca por gravação: o usuário aperta a nova combinação. O backend
 * valida de novo e, se o sistema recusar o atalho, o atual continua valendo.
 */
export default function ShortcutSection({
  slot,
  title,
  description,
  shortcut,
  onChanged,
}: ShortcutSectionProps) {
  const [recording, setRecording] = useState(false);
  const [busy, setBusy] = useState(false);
  const [feedback, setFeedback] = useState<Feedback | null>(null);

  const apply = useCallback(
    async (accelerator: string) => {
      setBusy(true);
      setFeedback(null);
      try {
        const view = await setShortcut(accelerator, slot);
        onChanged(view);
        setFeedback({ kind: "success", text: `Atalho alterado para ${shortcutOf(view, slot)}.` });
      } catch (error: unknown) {
        setFeedback({ kind: "error", text: errorMessage(error) });
      } finally {
        setBusy(false);
      }
    },
    [onChanged, slot],
  );

  useEffect(() => {
    if (!recording) {
      return;
    }
    // Captura antes de qualquer outro tratador: a combinação não deve disparar nada na janela.
    const onKeyDown = (event: KeyboardEvent) => {
      const bare = !event.ctrlKey && !event.altKey && !event.metaKey;
      // Tab, Enter e Espaço sozinhos seguem navegando e acionando botões: sem isso, quem usa
      // só o teclado ficaria preso na gravação, sem alcançar o botão "Cancelar".
      if (bare && NAVIGATION_KEYS.includes(event.code)) {
        return;
      }
      event.preventDefault();
      // Imediata: a outra seção de atalho também pode estar gravando, e a combinação vale
      // só para uma delas.
      event.stopImmediatePropagation();
      // Segurar a tecla repete o evento; só a primeira conta.
      if (event.repeat) {
        return;
      }
      if (event.code === "Escape") {
        setRecording(false);
        return;
      }
      if (isModifierKey(event.code)) {
        return;
      }
      setRecording(false);
      const accelerator = acceleratorFromEvent(event);
      if (accelerator === null) {
        setFeedback({ kind: "error", text: UNSUPPORTED_KEY });
        return;
      }
      if (bare) {
        setFeedback({ kind: "error", text: NEEDS_MODIFIER });
        return;
      }
      void apply(accelerator);
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [recording, apply]);

  return (
    <section aria-labelledby={`shortcut-${slot}-title`} className="settings__section">
      <h2 id={`shortcut-${slot}-title`}>{title}</h2>
      {description && <p className="settings__hint">{description}</p>}
      <p className="settings__status">
        Atalho atual: <kbd>{shortcut}</kbd>
      </p>
      <div className="settings__row">
        <button
          type="button"
          className="settings__button"
          disabled={busy}
          onClick={() => {
            setFeedback(null);
            setRecording((current) => !current);
          }}
        >
          {recording ? "Cancelar" : "Alterar atalho"}
        </button>
      </div>
      {recording && (
        <p role="status" className="settings__hint">
          Aperte a nova combinação (por exemplo, Ctrl+Alt+T). Esc cancela.
        </p>
      )}
      {feedback && (
        <p
          role={feedback.kind === "error" ? "alert" : "status"}
          className={`settings__feedback settings__feedback--${feedback.kind}`}
        >
          {feedback.text}
        </p>
      )}
      <p className="settings__hint">
        Com letra ou número, combine dois entre Ctrl, Alt e Shift (ou use Win); com F1 a F12, basta
        Ctrl ou Alt (menos Alt+F4). Atalhos comuns como Ctrl+C são recusados. No teclado ABNT2,
        Ctrl+Alt equivale ao AltGr e pode atrapalhar a digitação de alguns símbolos.
      </p>
    </section>
  );
}

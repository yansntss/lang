import { useEffect, useRef, useState, type FormEvent } from "react";
import { deleteSecret, errorMessage, hasSecret, setSecret, testSecret } from "../../lib/tauri";
import type { KeyCheck, SecretKind } from "../../lib/types";

type KeyStatus = "checking" | "configured" | "missing";

interface Feedback {
  kind: "success" | "error";
  text: string;
}

const STATUS_LABELS: Record<KeyStatus, string> = {
  checking: "Verificando…",
  configured: "Chave configurada ✓",
  missing: "Chave não configurada",
};

interface SecretSectionProps {
  kind: SecretKind;
  /** Nome do serviço, também o título da seção. */
  title: string;
  /** "do" ou "da", para o rótulo do campo: "Chave da API do DeepL". */
  article: "do" | "da";
  hint: string;
}

const formatCount = (value: number) => value.toLocaleString("pt-BR");

/** Texto do resultado de "Testar chave". O DeepL informa também o uso do mês. */
function describeCheck(check: KeyCheck): string {
  if (check.usage === null) {
    return "Chave válida ✓";
  }
  const { used, limit } = check.usage;
  const total = limit > 0 ? ` de ${formatCount(limit)}` : "";
  return `Chave válida ✓ Uso neste mês: ${formatCount(used)}${total} caracteres.`;
}

/**
 * Uma chave de API. O campo é limpo antes mesmo da chamada: o front nunca retém a chave, e o
 * backend nunca a devolve.
 */
export default function SecretSection({ kind, title, article, hint }: SecretSectionProps) {
  const [status, setStatus] = useState<KeyStatus>("checking");
  const [keyInput, setKeyInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  // Síncronos: o estado só muda no próximo render, e dois eventos podem chegar antes dele.
  const busyRef = useRef(false);
  const changedRef = useRef(false);
  const titleId = `${kind}-title`;
  const inputId = `${kind}-key`;

  useEffect(() => {
    let cancelled = false;
    hasSecret(kind).then(
      (configured) => {
        // Se a chave já foi salva ou removida aqui, esta resposta (a leitura inicial) é velha.
        if (!cancelled && !changedRef.current) {
          setStatus(configured ? "configured" : "missing");
        }
      },
      (error: unknown) => {
        if (!cancelled) {
          setStatus("missing");
          setFeedback({ kind: "error", text: errorMessage(error) });
        }
      },
    );
    return () => {
      cancelled = true;
    };
  }, [kind]);

  const save = async (event: FormEvent) => {
    event.preventDefault();
    const value = keyInput;
    if (busyRef.current || value.trim() === "") {
      return;
    }

    busyRef.current = true;
    changedRef.current = true;
    setKeyInput("");
    setBusy(true);
    setFeedback(null);
    try {
      await setSecret(kind, value);
      setStatus("configured");
      setFeedback({ kind: "success", text: "Chave salva." });
    } catch (error: unknown) {
      // O campo já foi limpo (a chave não fica retida), então avisa que é preciso digitar de novo.
      setFeedback({ kind: "error", text: `${errorMessage(error)} Digite a chave novamente.` });
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  };

  const remove = async () => {
    if (busyRef.current) {
      return;
    }
    busyRef.current = true;
    changedRef.current = true;
    setBusy(true);
    setFeedback(null);
    try {
      await deleteSecret(kind);
      setStatus("missing");
      setFeedback({ kind: "success", text: "Chave removida." });
    } catch (error: unknown) {
      setFeedback({ kind: "error", text: errorMessage(error) });
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  };

  const test = async () => {
    if (busyRef.current) {
      return;
    }
    busyRef.current = true;
    setBusy(true);
    setFeedback(null);
    try {
      setFeedback({ kind: "success", text: describeCheck(await testSecret(kind)) });
    } catch (error: unknown) {
      setFeedback({ kind: "error", text: errorMessage(error) });
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  };

  return (
    <section aria-labelledby={titleId} className="settings__section">
      <h2 id={titleId}>{title}</h2>
      <p className="settings__status">{STATUS_LABELS[status]}</p>

      <form className="settings__form" onSubmit={save}>
        <label htmlFor={inputId}>
          Chave da API {article} {title}
        </label>
        <div className="settings__row">
          <input
            id={inputId}
            className="settings__input"
            type="password"
            autoComplete="off"
            spellCheck={false}
            value={keyInput}
            onChange={(event) => setKeyInput(event.target.value)}
          />
          <button
            type="submit"
            className="settings__button settings__button--primary"
            disabled={busy || keyInput.trim() === ""}
          >
            Salvar chave
          </button>
        </div>
      </form>

      {status === "configured" && (
        <div className="settings__row">
          <button type="button" className="settings__button" onClick={test} disabled={busy}>
            Testar chave
          </button>
          <button type="button" className="settings__button" onClick={remove} disabled={busy}>
            Remover chave
          </button>
        </div>
      )}

      {feedback && (
        <p
          role={feedback.kind === "error" ? "alert" : "status"}
          className={`settings__feedback settings__feedback--${feedback.kind}`}
        >
          {feedback.text}
        </p>
      )}

      <p className="settings__hint">{hint}</p>
    </section>
  );
}

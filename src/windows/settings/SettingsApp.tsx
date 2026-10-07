import { useEffect, useState, type FormEvent } from "react";
import { deleteSecret, errorMessage, hasSecret, setSecret } from "../../lib/tauri";
import "../../styles/base.css";
import "./settings.css";

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

export default function SettingsApp() {
  const [status, setStatus] = useState<KeyStatus>("checking");
  const [keyInput, setKeyInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [feedback, setFeedback] = useState<Feedback | null>(null);

  useEffect(() => {
    let cancelled = false;
    hasSecret("deepl").then(
      (configured) => {
        if (!cancelled) {
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
  }, []);

  const save = async (event: FormEvent) => {
    event.preventDefault();
    const value = keyInput;
    if (busy || value.trim() === "") {
      return;
    }

    // A chave sai do estado do componente antes mesmo da chamada: o front nunca a retém.
    setKeyInput("");
    setBusy(true);
    setFeedback(null);
    try {
      await setSecret("deepl", value);
      setStatus("configured");
      setFeedback({ kind: "success", text: "Chave salva." });
    } catch (error: unknown) {
      setFeedback({ kind: "error", text: errorMessage(error) });
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    setBusy(true);
    setFeedback(null);
    try {
      await deleteSecret("deepl");
      setStatus("missing");
      setFeedback({ kind: "success", text: "Chave removida." });
    } catch (error: unknown) {
      setFeedback({ kind: "error", text: errorMessage(error) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <main className="settings">
      <h1>Configurações</h1>
      <section aria-labelledby="deepl-title">
        <h2 id="deepl-title">DeepL</h2>
        <p className="settings__status">{STATUS_LABELS[status]}</p>

        <form className="settings__form" onSubmit={save}>
          <label htmlFor="deepl-key">Chave da API do DeepL</label>
          <div className="settings__row">
            <input
              id="deepl-key"
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
          <button type="button" className="settings__button" onClick={remove} disabled={busy}>
            Remover chave
          </button>
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
          A chave fica guardada no Gerenciador de Credenciais do Windows e nunca é exibida de
          volta. Chaves do plano gratuito terminam em :fx.
        </p>
      </section>
    </main>
  );
}

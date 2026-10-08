import { useEffect, useState } from "react";
import { useHistory } from "../../hooks/useHistory";
import { useTheme } from "../../hooks/useTheme";
import {
  copyToClipboard,
  errorMessage,
  exportHistory,
  getHistoryEnabled,
  setHistoryEnabled,
} from "../../lib/tauri";
import "../../styles/base.css";
import HistoryList from "./HistoryList";
import ReviewCard from "./ReviewCard";
import "./history.css";

type Tab = "history" | "review";

export default function HistoryApp() {
  const [tab, setTab] = useState<Tab>("history");
  const [query, setQuery] = useState("");
  const [favoritesOnly, setFavoritesOnly] = useState(false);
  const [recording, setRecording] = useState<boolean | null>(null);
  const [recordingUnknown, setRecordingUnknown] = useState(false);
  const [confirmingClear, setConfirmingClear] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  // Muda quando o histórico é apagado: o cartão de revisão é remontado e busca de novo.
  const [reviewVersion, setReviewVersion] = useState(0);
  const history = useHistory(query, favoritesOnly);
  useTheme();

  useEffect(() => {
    let cancelled = false;
    getHistoryEnabled().then(
      (enabled) => {
        if (!cancelled) setRecording(enabled);
      },
      (failure: unknown) => {
        if (!cancelled) {
          setRecordingUnknown(true);
          setActionError(errorMessage(failure));
        }
      },
    );
    return () => {
      cancelled = true;
    };
  }, []);

  const succeed = (message: string | null) => {
    setNotice(message);
    setActionError(null);
  };
  const fail = (failure: unknown) => {
    setNotice(null);
    setActionError(errorMessage(failure));
  };

  const changeRecording = (enabled: boolean) => {
    setHistoryEnabled(enabled).then(() => {
      setRecording(enabled);
      succeed(null);
    }, fail);
  };

  const copy = (text: string) => {
    copyToClipboard(text).then(() => succeed("Copiado."), fail);
  };

  const exportCsv = () => {
    exportHistory().then((path) => succeed(`Exportado para ${path}`), fail);
  };

  const clearAll = () => {
    setConfirmingClear(false);
    void history.clearAll().then((cleared) => {
      if (cleared) {
        setReviewVersion((version) => version + 1);
        succeed("Histórico apagado.");
      } else {
        setNotice(null);
      }
    });
  };

  const showTab = (next: Tab) => {
    setTab(next);
    succeed(null);
  };

  const filtering = query.trim() !== "" || favoritesOnly;
  const error = tab === "history" ? (actionError ?? history.error) : actionError;

  return (
    <main className="history">
      <header className="history__toolbar">
        {recordingUnknown ? (
          <span className="history__hint">Não foi possível ler se a gravação está ligada.</span>
        ) : (
          <label className="history__switch">
            <input
              type="checkbox"
              checked={recording === true}
              disabled={recording === null}
              onChange={(event) => changeRecording(event.target.checked)}
            />
            Gravar histórico
          </label>
        )}
        <div className="history__toolbar-actions">
          <button type="button" onClick={exportCsv}>
            Exportar CSV
          </button>
          {confirmingClear ? (
            <>
              <button type="button" onClick={clearAll}>
                Sim, apagar tudo
              </button>
              <button type="button" onClick={() => setConfirmingClear(false)}>
                Cancelar
              </button>
            </>
          ) : (
            <button type="button" onClick={() => setConfirmingClear(true)}>
              Limpar tudo
            </button>
          )}
        </div>
      </header>
      {confirmingClear && (
        <p className="history__hint">Isso apaga todo o histórico, inclusive os favoritos.</p>
      )}
      <p className="history__hint">
        O histórico fica só neste computador, em texto simples, e pode incluir textos
        selecionados em outros apps.
      </p>

      <nav className="history__tabs" aria-label="Seções">
        <button type="button" aria-pressed={tab === "history"} onClick={() => showTab("history")}>
          Histórico
        </button>
        <button type="button" aria-pressed={tab === "review"} onClick={() => showTab("review")}>
          Revisão
        </button>
      </nav>

      {tab === "history" ? (
        <section className="history__body">
          <div className="history__filters">
            <input
              type="search"
              aria-label="Buscar no histórico"
              placeholder="Buscar…"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
            />
            <label>
              <input
                type="checkbox"
                checked={favoritesOnly}
                onChange={(event) => setFavoritesOnly(event.target.checked)}
              />
              Só favoritos
            </label>
          </div>
          {!history.loading && history.items.length === 0 && !error && (
            <p className="history__hint">
              {filtering ? "Nenhum resultado." : "Nada por aqui ainda."}
            </p>
          )}
          <HistoryList
            items={history.items}
            onToggleFavorite={history.toggle}
            onDelete={history.remove}
            onCopy={copy}
          />
          {history.hasMore && !history.loading && (
            <button type="button" onClick={history.loadMore}>
              Carregar mais
            </button>
          )}
        </section>
      ) : (
        <ReviewCard key={reviewVersion} />
      )}

      <div aria-live="polite">
        {notice && <p className="history__hint">{notice}</p>}
        {error && (
          <p role="alert" className="history__error">
            {error}
          </p>
        )}
      </div>
    </main>
  );
}

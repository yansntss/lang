import { useRef, useState } from "react";
import { checkUpdate, errorMessage, installUpdate } from "../../lib/tauri";
import type { AvailableUpdate, UpdateEvent } from "../../lib/types";

type UpdateState =
  | { status: "idle" }
  | { status: "checking" }
  | { status: "current" }
  | { status: "available"; update: AvailableUpdate }
  | { status: "downloading"; update: AvailableUpdate; downloaded: number; total: number | null }
  | { status: "installing"; update: AvailableUpdate }
  | { status: "error"; message: string; update: AvailableUpdate | null };

interface UpdateSectionProps {
  /** Versão do app em execução. */
  version: string;
}

const BYTES_PER_MEGABYTE = 1024 * 1024;

const megabytes = (bytes: number) => (bytes / BYTES_PER_MEGABYTE).toFixed(1);

function percent(downloaded: number, total: number | null): number | null {
  if (total === null || total <= 0) {
    return null;
  }
  return Math.min(100, Math.round((downloaded / total) * 100));
}

function formatDate(millis: number): string {
  return new Date(millis).toLocaleDateString("pt-BR", { dateStyle: "long" });
}

/**
 * Verificação e instalação de atualizações, só quando o usuário pede. O app nunca busca
 * atualização sozinho. As notas vêm de fora, então são exibidas como texto puro.
 */
export default function UpdateSection({ version }: UpdateSectionProps) {
  const [state, setState] = useState<UpdateState>({ status: "idle" });
  // Síncrono: dois cliques podem chegar antes do próximo render.
  const busyRef = useRef(false);

  const check = async () => {
    if (busyRef.current) {
      return;
    }
    busyRef.current = true;
    setState({ status: "checking" });
    try {
      const update = await checkUpdate();
      setState(update ? { status: "available", update } : { status: "current" });
    } catch (error: unknown) {
      setState({ status: "error", message: errorMessage(error), update: null });
    } finally {
      busyRef.current = false;
    }
  };

  const install = async (update: AvailableUpdate) => {
    if (busyRef.current) {
      return;
    }
    busyRef.current = true;
    setState({ status: "downloading", update, downloaded: 0, total: null });
    try {
      await installUpdate((event: UpdateEvent) => {
        setState(
          event.kind === "progress"
            ? { status: "downloading", update, downloaded: event.downloaded, total: event.total }
            : { status: "installing", update },
        );
      });
      // No Windows o app encerra antes de chegar aqui; em outro sistema é preciso reabrir.
      setState({ status: "installing", update });
    } catch (error: unknown) {
      setState({ status: "error", message: errorMessage(error), update });
    } finally {
      busyRef.current = false;
    }
  };

  const working =
    state.status === "checking" || state.status === "downloading" || state.status === "installing";
  const pending = state.status === "available" || state.status === "error" ? state.update : null;

  return (
    <section aria-labelledby="update-title" aria-busy={working} className="settings__section">
      <h2 id="update-title">Sobre e atualizações</h2>
      <p className="settings__status">Versão atual: {version}</p>

      <div className="settings__row">
        <button type="button" className="settings__button" disabled={working} onClick={check}>
          Verificar atualizações
        </button>
        {pending && (
          <button
            type="button"
            className="settings__button settings__button--primary"
            disabled={working}
            onClick={() => void install(pending)}
          >
            Instalar e reiniciar
          </button>
        )}
      </div>

      <div aria-live="polite">
        {state.status === "checking" && <p className="settings__hint">Verificando…</p>}
        {state.status === "current" && (
          <p className="settings__feedback settings__feedback--success">
            Você já está na versão mais recente.
          </p>
        )}
        {(state.status === "available" ||
          state.status === "downloading" ||
          state.status === "installing" ||
          (state.status === "error" && state.update)) && <UpdateDetails state={state} />}
      </div>

      {state.status === "error" && (
        <p role="alert" className="settings__feedback settings__feedback--error">
          {state.message}
        </p>
      )}
    </section>
  );
}

function UpdateDetails({ state }: { state: Extract<UpdateState, { update: AvailableUpdate | null }> }) {
  const { update } = state;
  if (!update) {
    return null;
  }
  return (
    <div className="settings__update">
      <p className="settings__status">
        Versão {update.version} disponível
        {update.publishedAt !== null && ` (publicada em ${formatDate(update.publishedAt)})`}.
      </p>
      {update.notes && <p className="settings__notes">{update.notes}</p>}
      {state.status === "downloading" && <DownloadProgress state={state} />}
      {state.status === "installing" && (
        <p className="settings__hint">
          Instalando… o aplicativo vai fechar e abrir de novo sozinho.
        </p>
      )}
    </div>
  );
}

function DownloadProgress({ state }: { state: Extract<UpdateState, { status: "downloading" }> }) {
  const value = percent(state.downloaded, state.total);
  return (
    <>
      <progress
        aria-label="Progresso do download"
        max={100}
        value={value ?? undefined}
        className="settings__progress"
      />
      <p className="settings__hint">
        Baixando… {megabytes(state.downloaded)}
        {state.total !== null && ` de ${megabytes(state.total)}`} MB
        {value !== null && ` (${value}%)`}
      </p>
    </>
  );
}

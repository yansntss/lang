import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AvailableUpdate,
  ExplainEvent,
  HistoryEntry,
  HistoryRecord,
  KeyCheck,
  PopupResetPayload,
  Preferences,
  RegionSelection,
  RegionSnapshot,
  ResizeEdge,
  SecretKind,
  SettingsView,
  ShortcutSlot,
  Translation,
  UpdateEvent,
} from "./types";

// Único ponto do front que fala com o backend. Nenhum comando devolve chaves de API.

export const POPUP_RESET_EVENT = "popup://reset";
export const REGION_SHOW_EVENT = "region://show";

const FALLBACK_ERROR_MESSAGE = "Algo deu errado. Tente novamente.";

export function translate(text: string): Promise<Translation> {
  return invoke<Translation>("translate", { text });
}

export function copyToClipboard(text: string): Promise<void> {
  return invoke<void>("copy_to_clipboard", { text });
}

/**
 * Pede a explicação de `text` e `translation`. O texto chega em `onEvent` (deltas, depois
 * `done` ou `error`). Devolve o id da explicação, usado para cancelá-la.
 */
export function explain(
  text: string,
  translation: string,
  onEvent: (event: ExplainEvent) => void,
): Promise<number> {
  const channel = new Channel<ExplainEvent>();
  channel.onmessage = onEvent;
  return invoke<number>("explain", { text, translation, onEvent: channel });
}

export function cancelExplain(id: number): Promise<void> {
  return invoke<void>("cancel_explain", { id });
}

/** Itens por página de `listHistory` (o backend usa o mesmo valor). */
export const HISTORY_PAGE_SIZE = 50;

/** Guarda uma tradução confirmada. Não grava nada se o histórico estiver desligado. */
export function recordHistory(record: HistoryRecord): Promise<void> {
  return invoke<void>("record_history", { ...record });
}

export function listHistory(
  query: string | null,
  favoritesOnly: boolean,
  offset: number,
): Promise<HistoryEntry[]> {
  return invoke<HistoryEntry[]>("list_history", { query, favoritesOnly, offset });
}

export function deleteHistory(id: number): Promise<void> {
  return invoke<void>("delete_history", { id });
}

export function clearHistory(): Promise<void> {
  return invoke<void>("clear_history");
}

/** Devolve o novo estado do favorito. */
export function toggleFavorite(id: number): Promise<boolean> {
  return invoke<boolean>("toggle_favorite", { id });
}

/** Exporta o histórico em CSV e devolve o caminho do arquivo gravado. */
export function exportHistory(): Promise<string> {
  return invoke<string>("export_history");
}

export function getHistoryEnabled(): Promise<boolean> {
  return invoke<boolean>("get_history_enabled");
}

export function setHistoryEnabled(enabled: boolean): Promise<void> {
  return invoke<void>("set_history_enabled", { enabled });
}

/** O próximo favorito a revisar, ou `null` se não houver favoritos. */
export function nextReview(): Promise<HistoryEntry | null> {
  return invoke<HistoryEntry | null>("next_review");
}

export function markReviewed(id: number): Promise<void> {
  return invoke<void>("mark_reviewed", { id });
}

export function hidePopup(): Promise<void> {
  return invoke<void>("hide_popup");
}

/** Pede à janela do popup a altura (em pixels) que o conteúdo precisa; ela cresce até caber. */
export function fitPopupHeight(contentHeight: number): Promise<void> {
  return invoke<void>("fit_popup_height", { contentHeight });
}

/** Fixado, o popup não some ao perder o foco. Ocultá-lo o solta de novo. */
export function setPopupPinned(pinned: boolean): Promise<void> {
  return invoke<void>("set_popup_pinned", { pinned });
}

/** Chamar no mousedown: o sistema move a janela até o botão ser solto. */
export function startPopupDragging(): Promise<void> {
  return invoke<void>("start_popup_dragging");
}

/** Chamar no mousedown de uma alça: o sistema redimensiona até o botão ser solto. */
export function startPopupResizing(edge: ResizeEdge): Promise<void> {
  return invoke<void>("start_popup_resizing", { edge });
}

export function setSecret(kind: SecretKind, value: string): Promise<void> {
  return invoke<void>("set_secret", { kind, value });
}

export function hasSecret(kind: SecretKind): Promise<boolean> {
  return invoke<boolean>("has_secret", { kind });
}

export function deleteSecret(kind: SecretKind): Promise<void> {
  return invoke<void>("delete_secret", { kind });
}

/** Confere a chave sem gastar a cota; o DeepL informa também o uso do mês. */
export function testSecret(kind: SecretKind): Promise<KeyCheck> {
  return invoke<KeyCheck>("test_secret", { kind });
}

export function getSettings(): Promise<SettingsView> {
  return invoke<SettingsView>("get_settings");
}

export function updateSettings(preferences: Preferences): Promise<SettingsView> {
  return invoke<SettingsView>("update_settings", { preferences });
}

/** Troca um dos atalhos globais (ex.: `"Ctrl+Alt+T"`). Se falhar, o atual continua valendo. */
export function setShortcut(accelerator: string, slot: ShortcutSlot): Promise<SettingsView> {
  return invoke<SettingsView>("set_shortcut", { slot, accelerator });
}

export function setAutostart(enabled: boolean): Promise<void> {
  return invoke<void>("set_autostart", { enabled });
}

/** Procura uma versão nova. `null` significa que o app já está na mais recente. */
export function checkUpdate(): Promise<AvailableUpdate | null> {
  return invoke<AvailableUpdate | null>("check_update");
}

/**
 * Baixa e instala a versão que a última verificação encontrou. O andamento chega em `onEvent`.
 * No Windows o app encerra sozinho quando o instalador assume.
 */
export function installUpdate(onEvent: (event: UpdateEvent) => void): Promise<void> {
  const channel = new Channel<UpdateEvent>();
  channel.onmessage = onEvent;
  return invoke<void>("install_update", { onEvent: channel });
}

/**
 * O popup foi exibido de novo. `prefill` é o texto que o usuário tinha selecionado em outro
 * app, ou `null` quando não houve captura. Devolve a função que cancela a escuta.
 */
export function onPopupReset(
  handler: (
    prefill: string | null,
    autoTranslate?: boolean,
    pinned?: boolean,
    notice?: string | null,
  ) => void,
): Promise<() => void> {
  return listen<PopupResetPayload>(POPUP_RESET_EVENT, (event) =>
    handler(
      event.payload.prefill,
      event.payload.autoTranslate ?? true,
      event.payload.pinned ?? false,
      event.payload.notice ?? null,
    ),
  );
}

/** Bytes do cabeçalho da captura: largura e altura, `u32` little-endian. */
const SNAPSHOT_HEADER_BYTES = 8;

/** Lê o formato de `get_region_snapshot`: largura, altura e os pixels RGBA. */
export function decodeSnapshot(buffer: ArrayBuffer): RegionSnapshot {
  if (buffer.byteLength < SNAPSHOT_HEADER_BYTES) {
    throw new Error("Captura da tela inválida.");
  }
  const header = new DataView(buffer, 0, SNAPSHOT_HEADER_BYTES);
  const width = header.getUint32(0, true);
  const height = header.getUint32(4, true);
  const pixels = new Uint8ClampedArray(buffer, SNAPSHOT_HEADER_BYTES);
  if (pixels.length !== width * height * 4) {
    throw new Error("Captura da tela inválida.");
  }
  return { width, height, pixels };
}

/** A captura do monitor sobre o qual a janela de seleção está aberta. */
export async function getRegionSnapshot(): Promise<RegionSnapshot> {
  return decodeSnapshot(await invoke<ArrayBuffer>("get_region_snapshot"));
}

/** Confirma a área arrastada. Uma área pequena demais é recusada e a seleção continua. */
export function confirmRegion(selection: RegionSelection): Promise<void> {
  return invoke<void>("confirm_region", { selection });
}

export function cancelRegion(): Promise<void> {
  return invoke<void>("cancel_region");
}

/** A janela de seleção foi aberta de novo, com uma captura nova. */
export function onRegionShow(handler: () => void): Promise<() => void> {
  return listen<null>(REGION_SHOW_EVENT, () => handler());
}

/** Extrai uma mensagem exibível de um erro do backend ou de qualquer valor lançado. */
export function errorMessage(error: unknown): string {
  if (typeof error === "object" && error !== null && "message" in error) {
    const { message } = error as { message: unknown };
    if (typeof message === "string" && message.length > 0) {
      return message;
    }
  }
  return FALLBACK_ERROR_MESSAGE;
}

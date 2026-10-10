export type TargetLang = "PT-BR" | "PT-PT" | "EN-US" | "EN-GB";

export type Theme = "system" | "light" | "dark";
export type EnglishVariant = "EN-US" | "EN-GB";
export type PortugueseVariant = "PT-BR" | "PT-PT";
export type ExplainModel = "claude-haiku-5-5" | "claude-sonnet-5-5";

/** Preferências que a janela de configurações grava de uma vez (espelha `Preferences`). */
export interface Preferences {
  theme: Theme;
  englishVariant: EnglishVariant;
  portugueseVariant: PortugueseVariant;
  explainModel: ExplainModel;
  /** Liberar a captura da seleção em VS Code, Cursor e IDEs JetBrains. */
  captureInEditors: boolean;
  /** Traduzir a seleção capturada assim que o popup abre (senão, só após o Enter). */
  autoTranslateSelection: boolean;
}

/** Qual dos atalhos globais: o do tradutor ou o da leitura de um trecho da tela (OCR). */
export type ShortcutSlot = "translate" | "ocr";

/** O que `get_settings` devolve: preferências, atalhos e o início com o Windows. */
export interface SettingsView extends Preferences {
  shortcut: string;
  ocrShortcut: string;
  autostart: boolean;
  /** Versão do app em execução. */
  appVersion: string;
}

/** Uma versão nova encontrada (espelha `AvailableUpdate` do backend). */
export interface AvailableUpdate {
  version: string;
  /** Notas da versão, em texto puro. */
  notes: string | null;
  /** Milissegundos desde 1970 UTC. */
  publishedAt: number | null;
}

/** Andamento da instalação (espelha `UpdateEvent` do backend). */
export type UpdateEvent =
  | { kind: "progress"; downloaded: number; total: number | null }
  | { kind: "installing" };

/** Resultado de "Testar chave". `usage` vem preenchido só para o DeepL. */
export interface KeyCheck {
  usage: { used: number; limit: number } | null;
}

export interface Translation {
  text: string;
  sourceLang: string;
  targetLang: TargetLang;
}

/** Erro serializado pelo backend (`AppError`): `message` já é seguro para exibir. */
export interface AppErrorDto {
  code: string;
  message: string;
}

export type SecretKind = "deepl" | "anthropic";

/** Evento do canal de `explain` (espelha `ExplainEvent` do backend). */
export type ExplainEvent =
  | { kind: "delta"; text: string }
  | { kind: "done" }
  | { kind: "error"; code: string; message: string };

/** Item do histórico (espelha `HistoryEntry` do backend). Datas em ms desde 1970 UTC. */
export interface HistoryEntry {
  id: number;
  sourceText: string;
  translatedText: string;
  sourceLang: string;
  targetLang: TargetLang;
  favorite: boolean;
  createdAt: number;
  lastUsedAt: number;
  useCount: number;
  lastReviewedAt: number | null;
}

/** Tradução confirmada pelo usuário, para guardar no histórico. */
export interface HistoryRecord {
  sourceText: string;
  translatedText: string;
  sourceLang: string;
  targetLang: TargetLang;
}

/** Corpo do evento `popup://reset`. */
export interface PopupResetPayload {
  prefill: string | null;
  /** Ausente em versões antigas do backend: tratar como `true`. */
  autoTranslate?: boolean;
  /** O popup continua fixado na tela. Ausente: `false`. */
  pinned?: boolean;
  /** Aviso a mostrar no popup (por exemplo, a captura da tela falhou). */
  notice?: string | null;
}

/** Retângulo arrastado na janela de seleção, em pixels da janela (CSS). */
export interface RegionSelection {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** A captura da tela que a janela de seleção desenha, em pixels físicos RGBA. */
export interface RegionSnapshot {
  width: number;
  height: number;
  pixels: Uint8ClampedArray<ArrayBuffer>;
}

/** Borda ou canto do popup que o usuário arrasta para redimensionar. */
export type ResizeEdge =
  | "north"
  | "south"
  | "east"
  | "west"
  | "northEast"
  | "northWest"
  | "southEast"
  | "southWest";

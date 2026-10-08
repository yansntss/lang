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

/** O que `get_settings` devolve: preferências, atalho e o início com o Windows. */
export interface SettingsView extends Preferences {
  shortcut: string;
  autostart: boolean;
}

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
}

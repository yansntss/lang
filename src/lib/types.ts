export type TargetLang = "PT-BR" | "EN-US";

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
}

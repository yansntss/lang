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

/** Corpo do evento `popup://reset`. */
export interface PopupResetPayload {
  prefill: string | null;
}

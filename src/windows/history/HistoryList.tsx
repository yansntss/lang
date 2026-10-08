import type { HistoryEntry } from "../../lib/types";

interface HistoryListProps {
  items: HistoryEntry[];
  onToggleFavorite: (id: number) => void;
  onDelete: (id: number) => void;
  onCopy: (text: string) => void;
}

const SNIPPET_CHARS = 30;

function formatDate(millis: number): string {
  return new Date(millis).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" });
}

/** Trecho do original que identifica a linha para quem usa leitor de tela. */
function snippet(text: string): string {
  const chars = Array.from(text);
  return chars.length > SNIPPET_CHARS ? `${chars.slice(0, SNIPPET_CHARS).join("")}…` : text;
}

/** Os textos são exibidos como texto puro: vieram de qualquer app, então nunca como HTML. */
export default function HistoryList({
  items,
  onToggleFavorite,
  onDelete,
  onCopy,
}: HistoryListProps) {
  return (
    <ul className="history__list">
      {items.map((entry) => {
        const label = snippet(entry.sourceText);
        return (
          <li key={entry.id} className="history__item">
            <p className="history__source">{entry.sourceText}</p>
            <p className="history__translation">{entry.translatedText}</p>
            <p className="history__meta">
              {entry.sourceLang} → {entry.targetLang} · {formatDate(entry.lastUsedAt)}
              {entry.useCount > 1 && ` · ${entry.useCount}×`}
            </p>
            <div className="history__actions">
              <button
                type="button"
                aria-pressed={entry.favorite}
                aria-label={`Favorito: ${label}`}
                onClick={() => onToggleFavorite(entry.id)}
              >
                {entry.favorite ? "★" : "☆"}
              </button>
              <button
                type="button"
                aria-label={`Copiar tradução de ${label}`}
                onClick={() => onCopy(entry.translatedText)}
              >
                Copiar
              </button>
              <button
                type="button"
                aria-label={`Apagar ${label}`}
                onClick={() => onDelete(entry.id)}
              >
                Apagar
              </button>
            </div>
          </li>
        );
      })}
    </ul>
  );
}

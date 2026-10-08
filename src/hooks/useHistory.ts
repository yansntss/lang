import { useCallback, useEffect, useRef, useState } from "react";
import {
  HISTORY_PAGE_SIZE,
  clearHistory,
  deleteHistory,
  errorMessage,
  listHistory,
  toggleFavorite,
} from "../lib/tauri";
import type { HistoryEntry } from "../lib/types";

export const HISTORY_SEARCH_DEBOUNCE_MS = 300;

export interface UseHistory {
  items: HistoryEntry[];
  loading: boolean;
  /** Mensagem do último erro (listagem ou ação); some quando a próxima ação começa. */
  error: string | null;
  hasMore: boolean;
  loadMore: () => void;
  remove: (id: number) => void;
  toggle: (id: number) => void;
  /** Resolve `true` se o histórico foi apagado, `false` se falhou (o erro fica em `error`). */
  clearAll: () => Promise<boolean>;
}

/**
 * Lista paginada do histórico. Mudar a busca ou o filtro recomeça da primeira página (a busca
 * espera uma pausa na digitação) e respostas de pedidos antigos são descartadas.
 */
export function useHistory(
  query: string,
  favoritesOnly: boolean,
  delayMs: number = HISTORY_SEARCH_DEBOUNCE_MS,
): UseHistory {
  const [items, setItems] = useState<HistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [hasMore, setHasMore] = useState(false);
  // Muda a cada recomeço da lista (ou limpeza): o que vier de uma época anterior é ignorado.
  const epoch = useRef(0);
  const loadingMore = useRef(false);
  // As respostas das ações chegam depois; elas precisam do filtro de agora, não o do clique.
  const currentFavoritesOnly = useRef(favoritesOnly);
  useEffect(() => {
    currentFavoritesOnly.current = favoritesOnly;
  }, [favoritesOnly]);

  const search = query.trim();
  const searchArgument = search === "" ? null : search;

  useEffect(() => {
    epoch.current += 1;
    const thisEpoch = epoch.current;
    loadingMore.current = false;
    setLoading(true);

    // Só a digitação precisa esperar; a carga inicial e o filtro de favoritos respondem já.
    const wait = searchArgument === null ? 0 : delayMs;
    const timer = setTimeout(() => {
      listHistory(searchArgument, favoritesOnly, 0).then(
        (page) => {
          if (thisEpoch !== epoch.current) return;
          setItems(page);
          setHasMore(page.length >= HISTORY_PAGE_SIZE);
          setError(null);
          setLoading(false);
        },
        (failure: unknown) => {
          if (thisEpoch !== epoch.current) return;
          // Não deixa a lista da consulta anterior na tela: o próximo "carregar mais" a
          // paginaria com o offset errado.
          setItems([]);
          setHasMore(false);
          setError(errorMessage(failure));
          setLoading(false);
        },
      );
    }, wait);

    return () => clearTimeout(timer);
  }, [searchArgument, favoritesOnly, delayMs]);

  const loadMore = useCallback(() => {
    if (loading || loadingMore.current) return;
    loadingMore.current = true;
    const thisEpoch = epoch.current;
    listHistory(searchArgument, favoritesOnly, items.length).then(
      (page) => {
        if (thisEpoch !== epoch.current) return;
        loadingMore.current = false;
        setItems((current) => [
          ...current,
          ...page.filter((entry) => !current.some((known) => known.id === entry.id)),
        ]);
        setHasMore(page.length >= HISTORY_PAGE_SIZE);
      },
      (failure: unknown) => {
        if (thisEpoch !== epoch.current) return;
        loadingMore.current = false;
        setError(errorMessage(failure));
      },
    );
  }, [loading, searchArgument, favoritesOnly, items.length]);

  const remove = useCallback((id: number) => {
    setError(null);
    deleteHistory(id).then(
      () => setItems((current) => current.filter((entry) => entry.id !== id)),
      (failure: unknown) => setError(errorMessage(failure)),
    );
  }, []);

  const toggle = useCallback((id: number) => {
    setError(null);
    toggleFavorite(id).then(
      (favorite) =>
        setItems((current) =>
          currentFavoritesOnly.current && !favorite
            ? current.filter((entry) => entry.id !== id)
            : current.map((entry) => (entry.id === id ? { ...entry, favorite } : entry)),
        ),
      (failure: unknown) => setError(errorMessage(failure)),
    );
  }, []);

  const clearAll = useCallback(() => {
    setError(null);
    return clearHistory().then(
      () => {
        // Pedidos em voo não podem repovoar a lista com o que acabou de ser apagado.
        epoch.current += 1;
        loadingMore.current = false;
        setItems([]);
        setHasMore(false);
        return true;
      },
      (failure: unknown) => {
        setError(errorMessage(failure));
        return false;
      },
    );
  }, []);

  return { items, loading, error, hasMore, loadMore, remove, toggle, clearAll };
}

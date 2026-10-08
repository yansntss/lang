import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as tauri from "../../lib/tauri";
import type { HistoryEntry } from "../../lib/types";
import HistoryApp from "./HistoryApp";

vi.mock("../../lib/tauri", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../lib/tauri")>()),
  listHistory: vi.fn(),
  deleteHistory: vi.fn(),
  clearHistory: vi.fn(),
  toggleFavorite: vi.fn(),
  exportHistory: vi.fn(),
  getHistoryEnabled: vi.fn(),
  setHistoryEnabled: vi.fn(),
  nextReview: vi.fn(),
  markReviewed: vi.fn(),
  copyToClipboard: vi.fn(),
}));

const listHistory = vi.mocked(tauri.listHistory);
const deleteHistory = vi.mocked(tauri.deleteHistory);
const clearHistory = vi.mocked(tauri.clearHistory);
const toggleFavorite = vi.mocked(tauri.toggleFavorite);
const exportHistory = vi.mocked(tauri.exportHistory);
const getHistoryEnabled = vi.mocked(tauri.getHistoryEnabled);
const setHistoryEnabled = vi.mocked(tauri.setHistoryEnabled);
const nextReview = vi.mocked(tauri.nextReview);
const markReviewed = vi.mocked(tauri.markReviewed);
const copyToClipboard = vi.mocked(tauri.copyToClipboard);

const BACKEND_ERROR = {
  code: "history",
  message: "Não foi possível acessar o histórico.",
};

function entry(id: number, overrides: Partial<HistoryEntry> = {}): HistoryEntry {
  return {
    id,
    sourceText: `original ${id}`,
    translatedText: `tradução ${id}`,
    sourceLang: "EN",
    targetLang: "PT-BR",
    favorite: false,
    createdAt: 1_700_000_000_000,
    lastUsedAt: 1_700_000_000_000,
    useCount: 1,
    lastReviewedAt: null,
    ...overrides,
  };
}

const PAGE = tauri.HISTORY_PAGE_SIZE;

beforeEach(() => {
  listHistory.mockReset().mockResolvedValue([]);
  deleteHistory.mockReset().mockResolvedValue(undefined);
  clearHistory.mockReset().mockResolvedValue(undefined);
  toggleFavorite.mockReset();
  exportHistory.mockReset();
  getHistoryEnabled.mockReset().mockResolvedValue(true);
  setHistoryEnabled.mockReset().mockResolvedValue(undefined);
  nextReview.mockReset().mockResolvedValue(null);
  markReviewed.mockReset().mockResolvedValue(undefined);
  copyToClipboard.mockReset().mockResolvedValue(undefined);
});

describe("HistoryApp — lista", () => {
  it("shows the saved translations as plain text", async () => {
    listHistory.mockResolvedValue([
      entry(1, { sourceText: "<b>bold</b>", translatedText: "<img src=x>", useCount: 3 }),
    ]);

    render(<HistoryApp />);

    expect(await screen.findByText("<b>bold</b>")).toBeInTheDocument();
    expect(screen.getByText("<img src=x>")).toBeInTheDocument();
    expect(screen.getByText(/EN → PT-BR/)).toHaveTextContent("3×");
    expect(document.querySelector("b, img")).toBeNull();
    expect(listHistory).toHaveBeenCalledWith(null, false, 0);
  });

  it("says so when there is nothing saved", async () => {
    render(<HistoryApp />);

    expect(await screen.findByText("Nada por aqui ainda.")).toBeInTheDocument();
  });

  it("warns that the history is stored locally in plain text", async () => {
    render(<HistoryApp />);

    expect(await screen.findByText(/só neste computador, em texto simples/)).toBeInTheDocument();
  });

  it("shows the backend error when the list cannot be loaded", async () => {
    listHistory.mockRejectedValue(BACKEND_ERROR);

    render(<HistoryApp />);

    expect(await screen.findByRole("alert")).toHaveTextContent(BACKEND_ERROR.message);
  });

  it("searches after a pause in typing", async () => {
    const user = userEvent.setup();
    render(<HistoryApp />);
    await screen.findByText("Nada por aqui ainda.");

    await user.type(screen.getByRole("searchbox", { name: "Buscar no histórico" }), "good");

    await waitFor(() => expect(listHistory).toHaveBeenLastCalledWith("good", false, 0));
    expect(listHistory).not.toHaveBeenCalledWith("g", false, 0);
  });

  it("filters favorites only", async () => {
    const user = userEvent.setup();
    render(<HistoryApp />);
    await screen.findByText("Nada por aqui ainda.");

    await user.click(screen.getByRole("checkbox", { name: "Só favoritos" }));

    await waitFor(() => expect(listHistory).toHaveBeenLastCalledWith(null, true, 0));
  });

  it("loads the next page and hides the button when the last page is short", async () => {
    const firstPage = Array.from({ length: PAGE }, (_, index) => entry(index + 1));
    listHistory.mockResolvedValueOnce(firstPage).mockResolvedValueOnce([entry(100)]);
    const user = userEvent.setup();
    render(<HistoryApp />);

    await user.click(await screen.findByRole("button", { name: "Carregar mais" }));

    expect(await screen.findByText("original 100")).toBeInTheDocument();
    expect(listHistory).toHaveBeenLastCalledWith(null, false, PAGE);
    expect(screen.queryByRole("button", { name: "Carregar mais" })).not.toBeInTheDocument();
  });
});

describe("HistoryApp — ações", () => {
  it("favorites an entry and updates the button", async () => {
    listHistory.mockResolvedValue([entry(1)]);
    toggleFavorite.mockResolvedValue(true);
    const user = userEvent.setup();
    render(<HistoryApp />);

    const star = await screen.findByRole("button", { name: /^Favorito:/ });
    expect(star).toHaveAttribute("aria-pressed", "false");
    await user.click(star);

    expect(toggleFavorite).toHaveBeenCalledExactlyOnceWith(1);
    await waitFor(() => expect(star).toHaveAttribute("aria-pressed", "true"));
  });

  it("drops an entry from the favorites-only list when it is unfavorited", async () => {
    listHistory.mockResolvedValue([entry(1, { favorite: true })]);
    toggleFavorite.mockResolvedValue(false);
    const user = userEvent.setup();
    render(<HistoryApp />);
    await user.click(screen.getByRole("checkbox", { name: "Só favoritos" }));

    await user.click(await screen.findByRole("button", { name: /^Favorito:/ }));

    await waitFor(() => expect(screen.queryByText("original 1")).not.toBeInTheDocument());
  });

  it("deletes one entry", async () => {
    listHistory.mockResolvedValue([entry(1), entry(2)]);
    const user = userEvent.setup();
    render(<HistoryApp />);
    const row = (await screen.findByText("original 1")).closest("li") as HTMLElement;

    await user.click(within(row).getByRole("button", { name: /^Apagar/ }));

    expect(deleteHistory).toHaveBeenCalledExactlyOnceWith(1);
    await waitFor(() => expect(screen.queryByText("original 1")).not.toBeInTheDocument());
    expect(screen.getByText("original 2")).toBeInTheDocument();
  });

  it("copies the translation of an entry", async () => {
    listHistory.mockResolvedValue([entry(1)]);
    const user = userEvent.setup();
    render(<HistoryApp />);

    await user.click(await screen.findByRole("button", { name: /^Copiar/ }));

    expect(copyToClipboard).toHaveBeenCalledExactlyOnceWith("tradução 1");
    expect(await screen.findByText("Copiado.")).toBeInTheDocument();
  });

  it("asks for confirmation before clearing everything", async () => {
    listHistory.mockResolvedValue([entry(1, { favorite: true })]);
    const user = userEvent.setup();
    render(<HistoryApp />);
    await screen.findByText("original 1");

    await user.click(screen.getByRole("button", { name: "Limpar tudo" }));
    expect(clearHistory).not.toHaveBeenCalled();
    expect(screen.getByText(/inclusive os favoritos/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(clearHistory).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Limpar tudo" }));
    await user.click(screen.getByRole("button", { name: "Sim, apagar tudo" }));

    expect(clearHistory).toHaveBeenCalledOnce();
    await waitFor(() => expect(screen.queryByText("original 1")).not.toBeInTheDocument());
    expect(screen.getByText("Histórico apagado.")).toBeInTheDocument();
  });

  it("shows where the CSV was saved", async () => {
    exportHistory.mockResolvedValue("C:\\Users\\yansa\\Downloads\\traducoes.csv");
    const user = userEvent.setup();
    render(<HistoryApp />);

    await user.click(screen.getByRole("button", { name: "Exportar CSV" }));

    expect(
      await screen.findByText(/Exportado para C:\\Users\\yansa\\Downloads\\traducoes\.csv/),
    ).toBeInTheDocument();
  });

  it("shows an export error", async () => {
    exportHistory.mockRejectedValue(BACKEND_ERROR);
    const user = userEvent.setup();
    render(<HistoryApp />);

    await user.click(screen.getByRole("button", { name: "Exportar CSV" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(BACKEND_ERROR.message);
  });
});

describe("HistoryApp — gravação", () => {
  it("reflects that recording is off and turns it back on", async () => {
    getHistoryEnabled.mockResolvedValue(false);
    const user = userEvent.setup();
    render(<HistoryApp />);
    const toggle = screen.getByRole("checkbox", { name: "Gravar histórico" });
    await waitFor(() => expect(toggle).toBeEnabled());
    expect(toggle).not.toBeChecked();

    await user.click(toggle);

    expect(setHistoryEnabled).toHaveBeenCalledExactlyOnceWith(true);
    await waitFor(() => expect(toggle).toBeChecked());
  });

  it("keeps the previous state when the change fails", async () => {
    setHistoryEnabled.mockRejectedValue(BACKEND_ERROR);
    const user = userEvent.setup();
    render(<HistoryApp />);
    const toggle = screen.getByRole("checkbox", { name: "Gravar histórico" });
    await waitFor(() => expect(toggle).toBeEnabled());

    await user.click(toggle);

    expect(await screen.findByRole("alert")).toBeInTheDocument();
    expect(toggle).toBeChecked();
  });
});

describe("HistoryApp — revisão", () => {
  it("explains how to get cards when there are no favorites", async () => {
    const user = userEvent.setup();
    render(<HistoryApp />);

    await user.click(screen.getByRole("button", { name: "Revisão" }));

    expect(await screen.findByText(/Favorite traduções/)).toBeInTheDocument();
  });

  it("reveals the translation, then moves on marking the card as reviewed", async () => {
    nextReview.mockResolvedValueOnce(entry(1)).mockResolvedValueOnce(entry(2));
    const user = userEvent.setup();
    render(<HistoryApp />);
    await user.click(screen.getByRole("button", { name: "Revisão" }));

    expect(await screen.findByText("original 1")).toBeInTheDocument();
    expect(screen.queryByText("tradução 1")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Mostrar tradução" }));
    expect(screen.getByText("tradução 1")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Próximo" }));

    expect(markReviewed).toHaveBeenCalledExactlyOnceWith(1);
    expect(await screen.findByText("original 2")).toBeInTheDocument();
    expect(screen.queryByText("tradução 2")).not.toBeInTheDocument();
  });

  it("shows an error when the next card cannot be loaded", async () => {
    nextReview.mockRejectedValue(BACKEND_ERROR);
    const user = userEvent.setup();
    render(<HistoryApp />);

    await user.click(screen.getByRole("button", { name: "Revisão" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(BACKEND_ERROR.message);
  });
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

describe("HistoryApp — corridas e falhas", () => {
  it("discards a slow answer for an old search", async () => {
    const initial = deferred<HistoryEntry[]>();
    listHistory.mockReturnValueOnce(initial.promise);
    listHistory.mockResolvedValueOnce([entry(2, { sourceText: "resultado novo" })]);
    const user = userEvent.setup();
    render(<HistoryApp />);

    await user.type(screen.getByRole("searchbox", { name: "Buscar no histórico" }), "a");
    expect(await screen.findByText("resultado novo")).toBeInTheDocument();
    initial.resolve([entry(1, { sourceText: "resultado velho" })]);
    await Promise.resolve();

    expect(screen.queryByText("resultado velho")).not.toBeInTheDocument();
    expect(screen.getByText("resultado novo")).toBeInTheDocument();
  });

  it("says there are no results, not that the history is empty, when filtering", async () => {
    const user = userEvent.setup();
    render(<HistoryApp />);
    await screen.findByText("Nada por aqui ainda.");

    await user.type(screen.getByRole("searchbox", { name: "Buscar no histórico" }), "zzz");

    expect(await screen.findByText("Nenhum resultado.")).toBeInTheDocument();
  });

  it("does not show a success notice nor empty the list when clearing fails", async () => {
    listHistory.mockResolvedValue([entry(1)]);
    clearHistory.mockRejectedValue(BACKEND_ERROR);
    const user = userEvent.setup();
    render(<HistoryApp />);
    await screen.findByText("original 1");

    await user.click(screen.getByRole("button", { name: "Limpar tudo" }));
    await user.click(screen.getByRole("button", { name: "Sim, apagar tudo" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(BACKEND_ERROR.message);
    expect(screen.queryByText("Histórico apagado.")).not.toBeInTheDocument();
    expect(screen.getByText("original 1")).toBeInTheDocument();
  });

  it("keeps the entry and shows an error when deleting fails", async () => {
    listHistory.mockResolvedValue([entry(1)]);
    deleteHistory.mockRejectedValue(BACKEND_ERROR);
    const user = userEvent.setup();
    render(<HistoryApp />);

    await user.click(await screen.findByRole("button", { name: /^Apagar/ }));

    expect(await screen.findByRole("alert")).toHaveTextContent(BACKEND_ERROR.message);
    expect(screen.getByText("original 1")).toBeInTheDocument();
  });

  it("keeps the star as it was when favoriting fails", async () => {
    listHistory.mockResolvedValue([entry(1)]);
    toggleFavorite.mockRejectedValue(BACKEND_ERROR);
    const user = userEvent.setup();
    render(<HistoryApp />);
    const star = await screen.findByRole("button", { name: /^Favorito:/ });

    await user.click(star);

    expect(await screen.findByRole("alert")).toHaveTextContent(BACKEND_ERROR.message);
    expect(star).toHaveAttribute("aria-pressed", "false");
  });

  it("does not claim recording is on when its state cannot be read", async () => {
    getHistoryEnabled.mockRejectedValue(BACKEND_ERROR);
    render(<HistoryApp />);

    expect(await screen.findByText(/Não foi possível ler se a gravação está ligada/)).toBeInTheDocument();
    expect(screen.queryByRole("checkbox", { name: "Gravar histórico" })).not.toBeInTheDocument();
  });

  it("shows the load-more error without losing the loaded entries", async () => {
    listHistory
      .mockResolvedValueOnce(Array.from({ length: PAGE }, (_, index) => entry(index + 1)))
      .mockRejectedValueOnce(BACKEND_ERROR);
    const user = userEvent.setup();
    render(<HistoryApp />);

    await user.click(await screen.findByRole("button", { name: "Carregar mais" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(BACKEND_ERROR.message);
    expect(screen.getByText("original 1")).toBeInTheDocument();
  });

  it("does not fetch the same page twice on a double click", async () => {
    const more = deferred<HistoryEntry[]>();
    listHistory
      .mockResolvedValueOnce(Array.from({ length: PAGE }, (_, index) => entry(index + 1)))
      .mockReturnValueOnce(more.promise);
    const user = userEvent.setup();
    render(<HistoryApp />);

    await user.dblClick(await screen.findByRole("button", { name: "Carregar mais" }));

    expect(listHistory).toHaveBeenCalledTimes(2);
    more.resolve([]);
  });
});

describe("HistoryApp — revisão (corridas e falhas)", () => {
  it("does not skip a card on a double click on Próximo", async () => {
    nextReview.mockResolvedValue(entry(1));
    const user = userEvent.setup();
    render(<HistoryApp />);
    await user.click(screen.getByRole("button", { name: "Revisão" }));
    await user.click(await screen.findByRole("button", { name: "Mostrar tradução" }));

    await user.dblClick(screen.getByRole("button", { name: "Próximo" }));

    await waitFor(() => expect(nextReview).toHaveBeenCalledTimes(2));
    expect(markReviewed).toHaveBeenCalledOnce();
  });

  it("offers to try again when marking as reviewed fails", async () => {
    nextReview.mockResolvedValue(entry(1));
    markReviewed.mockRejectedValueOnce(BACKEND_ERROR);
    const user = userEvent.setup();
    render(<HistoryApp />);
    await user.click(screen.getByRole("button", { name: "Revisão" }));
    await user.click(await screen.findByRole("button", { name: "Mostrar tradução" }));
    await user.click(screen.getByRole("button", { name: "Próximo" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(BACKEND_ERROR.message);
    await user.click(screen.getByRole("button", { name: "Tentar de novo" }));

    expect(await screen.findByText("original 1")).toBeInTheDocument();
  });

  it("reloads the review card after the history is cleared", async () => {
    nextReview.mockResolvedValueOnce(entry(1)).mockResolvedValueOnce(null);
    const user = userEvent.setup();
    render(<HistoryApp />);
    await user.click(screen.getByRole("button", { name: "Revisão" }));
    await screen.findByText("original 1");

    await user.click(screen.getByRole("button", { name: "Limpar tudo" }));
    await user.click(screen.getByRole("button", { name: "Sim, apagar tudo" }));

    expect(await screen.findByText(/Favorite traduções/)).toBeInTheDocument();
    expect(screen.queryByText("original 1")).not.toBeInTheDocument();
  });
});

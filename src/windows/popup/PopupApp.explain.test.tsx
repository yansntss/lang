import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as tauri from "../../lib/tauri";
import type { ExplainEvent, Translation } from "../../lib/types";
import PopupApp from "./PopupApp";

vi.mock("../../lib/tauri", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../lib/tauri")>()),
  translate: vi.fn(),
  hasSecret: vi.fn(),
  explain: vi.fn(),
  cancelExplain: vi.fn(),
  copyToClipboard: vi.fn(),
  hidePopup: vi.fn(),
  onPopupReset: vi.fn(),
}));

const translate = vi.mocked(tauri.translate);
const hasSecret = vi.mocked(tauri.hasSecret);
const explain = vi.mocked(tauri.explain);
const cancelExplain = vi.mocked(tauri.cancelExplain);
const onPopupReset = vi.mocked(tauri.onPopupReset);

const EXPLANATION_ID = 42;

let emit: (event: ExplainEvent) => void;
let resetHandler: ((prefill: string | null) => void) | undefined;

const translation: Translation = { text: "olá", sourceLang: "EN", targetLang: "PT-BR" };

const field = () => screen.getByRole("textbox", { name: "Texto para traduzir" });
const explainButton = () => screen.findByRole("button", { name: "Explicar" });

async function translated(user: ReturnType<typeof userEvent.setup>, text = "hello") {
  await user.type(field(), text);
  return explainButton();
}

/** Traduz, espera o botão habilitar (a consulta da chave é assíncrona) e clica nele. */
async function startExplanation(user: ReturnType<typeof userEvent.setup>) {
  const button = await translated(user);
  await waitFor(() => expect(button).toBeEnabled());
  await user.click(button);
  await waitFor(() => expect(explain).toHaveBeenCalledOnce());
  return button;
}

beforeEach(() => {
  resetHandler = undefined;
  emit = () => {
    throw new Error("explain() não foi chamado");
  };
  translate.mockReset().mockResolvedValue(translation);
  hasSecret.mockReset().mockResolvedValue(true);
  cancelExplain.mockReset().mockResolvedValue(undefined);
  vi.mocked(tauri.copyToClipboard).mockReset().mockResolvedValue(undefined);
  vi.mocked(tauri.hidePopup).mockReset().mockResolvedValue(undefined);
  explain.mockReset().mockImplementation(async (_text, _translation, onEvent) => {
    emit = onEvent;
    return EXPLANATION_ID;
  });
  onPopupReset.mockReset().mockImplementation(async (handler) => {
    resetHandler = handler;
    return () => {};
  });
});

describe("PopupApp — Explicar", () => {
  it("does not show the button before there is a translation", () => {
    render(<PopupApp />);

    expect(screen.queryByRole("button", { name: "Explicar" })).not.toBeInTheDocument();
  });

  it("disables the button and says why when the Anthropic key is missing", async () => {
    hasSecret.mockResolvedValue(false);
    const user = userEvent.setup();
    render(<PopupApp />);

    const button = await translated(user);

    expect(button).toBeDisabled();
    expect(screen.getByText(/chave da Anthropic/)).toBeInTheDocument();
    expect(hasSecret).toHaveBeenCalledWith("anthropic");
  });

  it("does not call the AI provider until the user clicks", async () => {
    const user = userEvent.setup();
    render(<PopupApp />);

    await translated(user);

    expect(explain).not.toHaveBeenCalled();
  });

  it("sends the original text and the translation, then streams the explanation", async () => {
    const user = userEvent.setup();
    render(<PopupApp />);

    const button = await startExplanation(user);

    expect(explain).toHaveBeenCalledExactlyOnceWith("hello", "olá", expect.any(Function));
    expect(button).toBeDisabled();
    act(() => emit({ kind: "delta", text: "Significa " }));
    act(() => emit({ kind: "delta", text: "cumprimento." }));
    expect(await screen.findByText("Significa cumprimento.")).toBeInTheDocument();
    expect(screen.getByText("Explicando…")).toBeInTheDocument();
    act(() => emit({ kind: "done" }));
    await waitFor(() => expect(screen.queryByText("Explicando…")).not.toBeInTheDocument());
    expect(screen.getByText("Significa cumprimento.")).toBeInTheDocument();
    expect(button).toBeEnabled();
  });

  it("renders the explanation as plain text, never as HTML", async () => {
    const user = userEvent.setup();
    render(<PopupApp />);
    await startExplanation(user);

    act(() => emit({ kind: "delta", text: "<img src=x onerror=alert(1)> **negrito**" }));

    expect(await screen.findByText("<img src=x onerror=alert(1)> **negrito**")).toBeInTheDocument();
    expect(document.querySelector("img")).toBeNull();
  });

  it("shows an error event and keeps the partial text", async () => {
    const user = userEvent.setup();
    render(<PopupApp />);
    const button = await startExplanation(user);

    act(() => emit({ kind: "delta", text: "parcial" }));
    act(() => emit({ kind: "error", code: "rate_limited", message: "Muitas requisições." }));

    expect(await screen.findByRole("alert")).toHaveTextContent("Muitas requisições.");
    expect(screen.getByText("parcial")).toBeInTheDocument();
    expect(button).toBeEnabled();
  });

  it("shows the message when the command itself is rejected", async () => {
    explain.mockRejectedValue({ code: "invalid_input", message: "Traduza antes." });
    const user = userEvent.setup();
    render(<PopupApp />);
    const button = await translated(user);
    await waitFor(() => expect(button).toBeEnabled());

    await user.click(button);

    expect(await screen.findByRole("alert")).toHaveTextContent("Traduza antes.");
  });

  it("cancels the backend and clears the panel when the text changes", async () => {
    const user = userEvent.setup();
    render(<PopupApp />);
    await startExplanation(user);
    act(() => emit({ kind: "delta", text: "explicação antiga" }));
    await screen.findByText("explicação antiga");

    await user.type(field(), "!");

    await waitFor(() => expect(cancelExplain).toHaveBeenCalledWith(EXPLANATION_ID));
    expect(screen.queryByText("explicação antiga")).not.toBeInTheDocument();
  });

  it("ignores events of a cancelled explanation", async () => {
    const user = userEvent.setup();
    render(<PopupApp />);
    await startExplanation(user);
    const staleEmit = emit;

    await user.type(field(), "!");
    act(() => staleEmit({ kind: "delta", text: "atrasado" }));

    expect(screen.queryByText("atrasado")).not.toBeInTheDocument();
  });

  it("cancels the explanation when the popup is shown again", async () => {
    const user = userEvent.setup();
    render(<PopupApp />);
    await startExplanation(user);
    act(() => emit({ kind: "delta", text: "texto" }));
    await screen.findByText("texto");

    act(() => resetHandler?.(null));

    await waitFor(() => expect(cancelExplain).toHaveBeenCalledWith(EXPLANATION_ID));
    expect(screen.queryByText("texto")).not.toBeInTheDocument();
  });
});

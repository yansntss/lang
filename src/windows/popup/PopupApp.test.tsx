import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as tauri from "../../lib/tauri";
import type { Translation } from "../../lib/types";
import PopupApp from "./PopupApp";

vi.mock("../../lib/tauri", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../lib/tauri")>()),
  translate: vi.fn(),
  copyToClipboard: vi.fn(),
  hidePopup: vi.fn(),
  onPopupReset: vi.fn(),
}));

const translate = vi.mocked(tauri.translate);
const copyToClipboard = vi.mocked(tauri.copyToClipboard);
const hidePopup = vi.mocked(tauri.hidePopup);
const onPopupReset = vi.mocked(tauri.onPopupReset);

// Timers reais: o debounce padrão é 400 ms, então esperamos um pouco mais que isso.
const AFTER_DEBOUNCE_MS = 450;

let resetHandler: ((prefill: string | null) => void) | undefined;

function translation(text: string): Translation {
  return { text, sourceLang: "EN", targetLang: "PT-BR" };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

const field = () => screen.getByRole("textbox", { name: "Texto para traduzir" });

const pause = (ms: number) => new Promise((done) => setTimeout(done, ms));

async function waitForTranslateCalls(count: number) {
  await waitFor(() => expect(translate).toHaveBeenCalledTimes(count));
}

beforeEach(() => {
  resetHandler = undefined;
  translate.mockReset();
  copyToClipboard.mockReset().mockResolvedValue(undefined);
  hidePopup.mockReset().mockResolvedValue(undefined);
  onPopupReset.mockReset().mockImplementation(async (handler) => {
    resetHandler = handler;
    return () => {};
  });
});

describe("PopupApp", () => {
  it("focuses the text field on mount", () => {
    render(<PopupApp />);

    expect(field()).toHaveFocus();
  });

  it("translates only after the debounce and shows the result with the languages", async () => {
    translate.mockResolvedValue(translation("olá"));
    const user = userEvent.setup();
    render(<PopupApp />);

    await user.type(field(), "hello");
    expect(translate).not.toHaveBeenCalled();

    expect(await screen.findByText("olá")).toBeInTheDocument();
    expect(translate).toHaveBeenCalledExactlyOnceWith("hello");
    expect(screen.getByText("EN → PT-BR")).toBeInTheDocument();
  });

  it("does not call the backend for blank text", async () => {
    const user = userEvent.setup();
    render(<PopupApp />);

    await user.type(field(), "   ");
    await pause(AFTER_DEBOUNCE_MS);

    expect(translate).not.toHaveBeenCalled();
  });

  it("shows the message of a backend error", async () => {
    translate.mockRejectedValue({
      code: "quota_exceeded",
      message: "A cota mensal do DeepL foi esgotada.",
    });
    const user = userEvent.setup();
    render(<PopupApp />);

    await user.type(field(), "hello");

    expect(await screen.findByRole("alert")).toHaveTextContent("A cota mensal do DeepL foi esgotada.");
  });

  it("discards a stale response when the text changed meanwhile", async () => {
    const first = deferred<Translation>();
    const second = deferred<Translation>();
    translate.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
    const user = userEvent.setup();
    render(<PopupApp />);

    await user.type(field(), "a");
    await waitForTranslateCalls(1);
    await user.type(field(), "b");
    await waitForTranslateCalls(2);
    await act(async () => second.resolve(translation("traduzido-B")));
    await act(async () => first.resolve(translation("traduzido-A")));

    expect(await screen.findByText("traduzido-B")).toBeInTheDocument();
    expect(screen.queryByText("traduzido-A")).not.toBeInTheDocument();
  });

  it("copies the translation and hides the popup on Enter", async () => {
    translate.mockResolvedValue(translation("olá"));
    const user = userEvent.setup();
    render(<PopupApp />);
    await user.type(field(), "hello");
    await screen.findByText("olá");

    await user.keyboard("{Enter}");

    expect(copyToClipboard).toHaveBeenCalledExactlyOnceWith("olá");
    await waitFor(() => expect(hidePopup).toHaveBeenCalledOnce());
  });

  it("does not insert a line break when Enter copies", async () => {
    translate.mockResolvedValue(translation("olá"));
    const user = userEvent.setup();
    render(<PopupApp />);
    await user.type(field(), "hello");
    await screen.findByText("olá");

    await user.keyboard("{Enter}");

    expect(field()).toHaveValue("hello");
  });

  it("keeps Shift+Enter for line breaks without copying", async () => {
    translate.mockResolvedValue(translation("olá"));
    const user = userEvent.setup();
    render(<PopupApp />);
    await user.type(field(), "hello");
    await screen.findByText("olá");

    await user.keyboard("{Shift>}{Enter}{/Shift}");

    expect(copyToClipboard).not.toHaveBeenCalled();
    expect(field()).toHaveValue("hello\n");
  });

  it("ignores Enter while the translation of the current text is not ready", async () => {
    translate.mockResolvedValue(translation("olá"));
    const user = userEvent.setup();
    render(<PopupApp />);

    await user.type(field(), "hello{Enter}");

    expect(copyToClipboard).not.toHaveBeenCalled();
    expect(hidePopup).not.toHaveBeenCalled();
  });

  it("ignores Enter when the visible translation belongs to older text", async () => {
    translate.mockResolvedValue(translation("olá"));
    const user = userEvent.setup();
    render(<PopupApp />);
    await user.type(field(), "hello");
    await screen.findByText("olá");

    await user.type(field(), "!{Enter}");

    expect(copyToClipboard).not.toHaveBeenCalled();
  });

  it("keeps the popup open and shows the error when copying fails", async () => {
    translate.mockResolvedValue(translation("olá"));
    copyToClipboard.mockRejectedValue({
      code: "clipboard",
      message: "Não foi possível copiar para a área de transferência.",
    });
    const user = userEvent.setup();
    render(<PopupApp />);
    await user.type(field(), "hello");
    await screen.findByText("olá");

    await user.keyboard("{Enter}");

    expect(await screen.findByRole("alert")).toHaveTextContent("área de transferência");
    expect(hidePopup).not.toHaveBeenCalled();
  });

  it("hides the popup on Escape", async () => {
    const user = userEvent.setup();
    render(<PopupApp />);

    await user.keyboard("{Escape}");

    expect(hidePopup).toHaveBeenCalledOnce();
  });

  it("refocuses the field when the window regains focus", async () => {
    render(<PopupApp />);
    field().blur();
    expect(field()).not.toHaveFocus();

    fireEvent.focus(window);

    expect(field()).toHaveFocus();
  });

  it("does not translate again when only surrounding whitespace changes", async () => {
    translate.mockResolvedValue(translation("olá"));
    const user = userEvent.setup();
    render(<PopupApp />);
    await user.type(field(), "hello");
    await screen.findByText("olá");

    await user.type(field(), " ");
    await pause(AFTER_DEBOUNCE_MS);

    expect(translate).toHaveBeenCalledOnce();
  });

  it("ignores the key repeat of Enter", async () => {
    translate.mockResolvedValue(translation("olá"));
    const user = userEvent.setup();
    render(<PopupApp />);
    await user.type(field(), "hello");
    await screen.findByText("olá");

    fireEvent.keyDown(field(), { key: "Enter", repeat: true });

    expect(copyToClipboard).not.toHaveBeenCalled();
  });

  it("does not hide on Escape while an IME composition is active", () => {
    render(<PopupApp />);

    fireEvent.keyDown(document, { key: "Escape", isComposing: true });

    expect(hidePopup).not.toHaveBeenCalled();
  });

  it("clears the text and refocuses the field when the popup is shown again", async () => {
    const user = userEvent.setup();
    render(<PopupApp />);
    await user.type(field(), "hello");
    await user.tab();

    await act(async () => resetHandler?.(null));

    expect(field()).toHaveValue("");
    expect(field()).toHaveFocus();
  });

  it("fills the field with the captured selection and translates it", async () => {
    translate.mockResolvedValue(translation("olá"));
    render(<PopupApp />);

    await act(async () => resetHandler?.("hello world"));

    expect(field()).toHaveValue("hello world");
    expect(field()).toHaveFocus();
    expect(await screen.findByText("olá")).toBeInTheDocument();
    expect(translate).toHaveBeenCalledExactlyOnceWith("hello world");
  });

  it("replaces previously typed text when a new selection is captured", async () => {
    translate.mockResolvedValue(translation("novo"));
    const user = userEvent.setup();
    render(<PopupApp />);
    await user.type(field(), "texto antigo");

    await act(async () => resetHandler?.("new selection"));

    expect(field()).toHaveValue("new selection");
  });

  it("lets Enter copy the translation of a captured selection", async () => {
    translate.mockResolvedValue(translation("olá"));
    const user = userEvent.setup();
    render(<PopupApp />);
    await act(async () => resetHandler?.("hello"));
    await screen.findByText("olá");

    await user.keyboard("{Enter}");

    expect(copyToClipboard).toHaveBeenCalledExactlyOnceWith("olá");
  });
});

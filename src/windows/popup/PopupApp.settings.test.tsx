import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as tauri from "../../lib/tauri";
import type { SettingsView, Translation } from "../../lib/types";
import PopupApp from "./PopupApp";

vi.mock("../../lib/tauri", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../lib/tauri")>()),
  translate: vi.fn(),
  hasSecret: vi.fn(),
  getSettings: vi.fn(),
  recordHistory: vi.fn(),
  copyToClipboard: vi.fn(),
  hidePopup: vi.fn(),
  onPopupReset: vi.fn(),
}));

const translate = vi.mocked(tauri.translate);
const getSettings = vi.mocked(tauri.getSettings);
const copyToClipboard = vi.mocked(tauri.copyToClipboard);

const translation: Translation = { text: "olá", sourceLang: "EN", targetLang: "PT-BR" };
const SETTINGS: SettingsView = {
  shortcut: "Ctrl+Alt+T",
  autostart: false,
  theme: "system",
  englishVariant: "EN-US",
  portugueseVariant: "PT-BR",
  explainModel: "claude-haiku-5-5",
  captureInEditors: false,
  autoTranslateSelection: true,
};
// Um pouco mais que o debounce de 400 ms: se a tradução fosse sair sozinha, já teria saído.
const PAST_DEBOUNCE_MS = 600;

let resetHandler: (prefill: string | null, autoTranslate?: boolean) => void;

const field = () => screen.getByRole("textbox", { name: "Texto para traduzir" });
const pause = (ms: number) => new Promise((done) => setTimeout(done, ms));

beforeEach(() => {
  delete document.documentElement.dataset.theme;
  translate.mockReset().mockResolvedValue(translation);
  getSettings.mockReset().mockResolvedValue(SETTINGS);
  copyToClipboard.mockReset().mockResolvedValue(undefined);
  vi.mocked(tauri.hasSecret).mockReset().mockResolvedValue(false);
  vi.mocked(tauri.recordHistory).mockReset().mockResolvedValue(undefined);
  vi.mocked(tauri.hidePopup).mockReset().mockResolvedValue(undefined);
  vi.mocked(tauri.onPopupReset)
    .mockReset()
    .mockImplementation(async (handler) => {
      resetHandler = handler;
      return () => {};
    });
});

describe("PopupApp — seleção capturada", () => {
  it("translates the captured selection by itself by default", async () => {
    render(<PopupApp />);
    await waitFor(() => expect(tauri.onPopupReset).toHaveBeenCalled());

    act(() => resetHandler("hello", true));

    expect(await screen.findByText("olá")).toBeInTheDocument();
    expect(translate).toHaveBeenCalledExactlyOnceWith("hello");
  });

  it("holds the captured selection until Enter when automatic translation is off", async () => {
    render(<PopupApp />);
    await waitFor(() => expect(tauri.onPopupReset).toHaveBeenCalled());

    act(() => resetHandler("hello", false));
    await pause(PAST_DEBOUNCE_MS);

    expect(field()).toHaveValue("hello");
    expect(translate).not.toHaveBeenCalled();
    expect(screen.getByText("Enter traduz · Esc fecha")).toBeInTheDocument();
  });

  it("translates on the first Enter and copies only on the second", async () => {
    const user = userEvent.setup();
    render(<PopupApp />);
    await waitFor(() => expect(tauri.onPopupReset).toHaveBeenCalled());
    act(() => resetHandler("hello", false));

    await user.click(field());
    await user.keyboard("{Enter}");

    expect(await screen.findByText("olá")).toBeInTheDocument();
    expect(translate).toHaveBeenCalledExactlyOnceWith("hello");
    expect(copyToClipboard).not.toHaveBeenCalled();
    await user.keyboard("{Enter}");
    await waitFor(() => expect(copyToClipboard).toHaveBeenCalledExactlyOnceWith("olá"));
  });

  it("releases the held text as soon as the user edits it", async () => {
    const user = userEvent.setup();
    render(<PopupApp />);
    await waitFor(() => expect(tauri.onPopupReset).toHaveBeenCalled());
    act(() => resetHandler("hello", false));

    await user.click(field());
    await user.type(field(), "!");

    expect(await screen.findByText("olá")).toBeInTheDocument();
    expect(translate).toHaveBeenCalledExactlyOnceWith("hello!");
  });

  it("does not hold a blank selection, which has nothing to translate", async () => {
    render(<PopupApp />);
    await waitFor(() => expect(tauri.onPopupReset).toHaveBeenCalled());

    act(() => resetHandler("   ", false));

    expect(screen.queryByText("Enter traduz · Esc fecha")).not.toBeInTheDocument();
  });

  it("does not hold anything when nothing was captured", async () => {
    const user = userEvent.setup();
    render(<PopupApp />);
    await waitFor(() => expect(tauri.onPopupReset).toHaveBeenCalled());
    act(() => resetHandler(null, false));

    await user.type(field(), "hello");

    expect(await screen.findByText("olá")).toBeInTheDocument();
    expect(screen.queryByText("Enter traduz · Esc fecha")).not.toBeInTheDocument();
  });

  it("a new capture with automatic translation on lifts a previous hold", async () => {
    render(<PopupApp />);
    await waitFor(() => expect(tauri.onPopupReset).toHaveBeenCalled());
    act(() => resetHandler("one", false));
    await pause(PAST_DEBOUNCE_MS);
    expect(translate).not.toHaveBeenCalled();

    act(() => resetHandler("two", true));

    expect(await screen.findByText("olá")).toBeInTheDocument();
    expect(translate).toHaveBeenCalledExactlyOnceWith("two");
  });
});

describe("PopupApp — tema", () => {
  it("applies the saved theme", async () => {
    getSettings.mockResolvedValue({ ...SETTINGS, theme: "dark" });

    render(<PopupApp />);

    await waitFor(() => expect(document.documentElement.dataset.theme).toBe("dark"));
  });

  it("keeps the system theme when the settings cannot be read", async () => {
    getSettings.mockRejectedValue({ code: "settings", message: "x" });

    render(<PopupApp />);
    await waitFor(() => expect(getSettings).toHaveBeenCalled());

    expect(document.documentElement.dataset.theme).toBeUndefined();
  });
});

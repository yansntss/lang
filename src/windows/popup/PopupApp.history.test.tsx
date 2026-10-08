import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as tauri from "../../lib/tauri";
import type { Translation } from "../../lib/types";
import PopupApp from "./PopupApp";

vi.mock("../../lib/tauri", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../lib/tauri")>()),
  translate: vi.fn(),
  hasSecret: vi.fn(),
  explain: vi.fn(),
  cancelExplain: vi.fn(),
  recordHistory: vi.fn(),
  copyToClipboard: vi.fn(),
  hidePopup: vi.fn(),
  onPopupReset: vi.fn(),
}));

const translate = vi.mocked(tauri.translate);
const hasSecret = vi.mocked(tauri.hasSecret);
const explain = vi.mocked(tauri.explain);
const recordHistory = vi.mocked(tauri.recordHistory);
const copyToClipboard = vi.mocked(tauri.copyToClipboard);
const hidePopup = vi.mocked(tauri.hidePopup);

const translation: Translation = { text: "olá", sourceLang: "EN", targetLang: "PT-BR" };
const RECORD = {
  sourceText: "hello",
  translatedText: "olá",
  sourceLang: "EN",
  targetLang: "PT-BR",
};

const field = () => screen.getByRole("textbox", { name: "Texto para traduzir" });
const pause = (ms: number) => new Promise((done) => setTimeout(done, ms));

beforeEach(() => {
  translate.mockReset().mockResolvedValue(translation);
  hasSecret.mockReset().mockResolvedValue(true);
  explain.mockReset().mockResolvedValue(1);
  vi.mocked(tauri.cancelExplain).mockReset().mockResolvedValue(undefined);
  recordHistory.mockReset().mockResolvedValue(undefined);
  copyToClipboard.mockReset().mockResolvedValue(undefined);
  hidePopup.mockReset().mockResolvedValue(undefined);
  vi.mocked(tauri.onPopupReset)
    .mockReset()
    .mockImplementation(async () => () => {});
});

describe("PopupApp — histórico", () => {
  it("does not record a translation that was only typed", async () => {
    const user = userEvent.setup();
    render(<PopupApp />);

    await user.type(field(), "hello");
    await screen.findByText("olá");
    await pause(100);

    expect(recordHistory).not.toHaveBeenCalled();
  });

  it("records the translation when Enter copies it", async () => {
    const user = userEvent.setup();
    render(<PopupApp />);
    await user.type(field(), "hello");
    await screen.findByText("olá");

    await user.keyboard("{Enter}");

    expect(recordHistory).toHaveBeenCalledExactlyOnceWith(RECORD);
    await waitFor(() => expect(copyToClipboard).toHaveBeenCalledWith("olá"));
  });

  it("records the translation when the user asks for an explanation", async () => {
    const user = userEvent.setup();
    render(<PopupApp />);
    await user.type(field(), "hello");
    const button = await screen.findByRole("button", { name: "Explicar" });
    await waitFor(() => expect(button).toBeEnabled());

    await user.click(button);

    expect(recordHistory).toHaveBeenCalledExactlyOnceWith(RECORD);
    expect(explain).toHaveBeenCalledOnce();
  });

  it("records once when the user explains and then copies the same translation", async () => {
    const user = userEvent.setup();
    render(<PopupApp />);
    await user.type(field(), "hello");
    const button = await screen.findByRole("button", { name: "Explicar" });
    await waitFor(() => expect(button).toBeEnabled());
    await user.click(button);

    await user.keyboard("{Enter}");

    await waitFor(() => expect(copyToClipboard).toHaveBeenCalledWith("olá"));
    expect(recordHistory).toHaveBeenCalledOnce();
  });

  it("still copies and closes when recording the history fails", async () => {
    recordHistory.mockRejectedValue({
      code: "history",
      message: "Não foi possível acessar o histórico.",
    });
    const user = userEvent.setup();
    render(<PopupApp />);
    await user.type(field(), "hello");
    await screen.findByText("olá");

    await user.keyboard("{Enter}");

    await waitFor(() => expect(copyToClipboard).toHaveBeenCalledWith("olá"));
    await waitFor(() => expect(hidePopup).toHaveBeenCalledOnce());
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});

import { act, render, screen, waitFor } from "@testing-library/react";
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
  recordHistory: vi.fn(),
  onPopupReset: vi.fn(),
  onCaption: vi.fn(),
  onCaptionStopped: vi.fn(),
  startCaption: vi.fn(),
  stopCaption: vi.fn(),
  setPopupPinned: vi.fn(),
}));

const translate = vi.mocked(tauri.translate);
const recordHistory = vi.mocked(tauri.recordHistory);
const startCaption = vi.mocked(tauri.startCaption);
const stopCaption = vi.mocked(tauri.stopCaption);

let resetHandler: Parameters<typeof tauri.onPopupReset>[0] | undefined;
let captionHandler: ((text: string, autoTranslate: boolean) => void) | undefined;
let stoppedHandler: ((notice: string) => void) | undefined;

function translation(text: string): Translation {
  return { text, sourceLang: "EN", targetLang: "PT-BR" };
}

const startButton = () => screen.getByRole("button", { name: "Legenda ao vivo" });
const field = () => screen.getByRole("textbox", { name: "Texto para traduzir" });

async function openFromScreen(captionAvailable: boolean) {
  render(<PopupApp />);
  await waitFor(() => expect(resetHandler).toBeDefined());
  act(() => resetHandler?.("Run!", true, true, null, "ocr", captionAvailable));
}

beforeEach(() => {
  resetHandler = undefined;
  captionHandler = undefined;
  stoppedHandler = undefined;
  translate.mockReset().mockImplementation(async (text: string) => translation(`pt:${text}`));
  recordHistory.mockReset().mockResolvedValue(undefined);
  vi.mocked(tauri.copyToClipboard).mockReset().mockResolvedValue(undefined);
  vi.mocked(tauri.hidePopup).mockReset().mockResolvedValue(undefined);
  vi.mocked(tauri.setPopupPinned).mockReset().mockResolvedValue(undefined);
  startCaption.mockReset().mockResolvedValue(undefined);
  stopCaption.mockReset().mockResolvedValue(undefined);
  vi.mocked(tauri.onPopupReset)
    .mockReset()
    .mockImplementation(async (handler) => {
      resetHandler = handler;
      return () => {};
    });
  vi.mocked(tauri.onCaption)
    .mockReset()
    .mockImplementation(async (handler) => {
      captionHandler = handler;
      return () => {};
    });
  vi.mocked(tauri.onCaptionStopped)
    .mockReset()
    .mockImplementation(async (handler) => {
      stoppedHandler = handler;
      return () => {};
    });
});

describe("PopupApp — legenda ao vivo", () => {
  it("offers no live caption when the backend says it is not available", async () => {
    await openFromScreen(false);

    expect(screen.queryByRole("button", { name: "Legenda ao vivo" })).not.toBeInTheDocument();
  });

  it("starts the live caption and offers to stop it", async () => {
    const user = userEvent.setup();
    await openFromScreen(true);

    await user.click(startButton());

    expect(startCaption).toHaveBeenCalledOnce();
    expect(await screen.findByRole("button", { name: "Parar legenda" })).toBeInTheDocument();
  });

  it("translates each new caption line", async () => {
    const user = userEvent.setup();
    await openFromScreen(true);
    await user.click(startButton());
    await waitFor(() => expect(captionHandler).toBeDefined());

    act(() => captionHandler?.("Hide!", true));

    expect(field()).toHaveValue("Hide!");
    expect(await screen.findByText("pt:Hide!")).toBeInTheDocument();
  });

  it("stops when the user asks", async () => {
    const user = userEvent.setup();
    await openFromScreen(true);
    await user.click(startButton());

    await user.click(await screen.findByRole("button", { name: "Parar legenda" }));

    expect(stopCaption).toHaveBeenCalledOnce();
    expect(startButton()).toBeInTheDocument();
  });

  it("shows why the caption stopped on its own", async () => {
    const user = userEvent.setup();
    await openFromScreen(true);
    await user.click(startButton());
    await waitFor(() => expect(stoppedHandler).toBeDefined());

    act(() => stoppedHandler?.("Não foi possível capturar a tela."));

    expect(screen.getByRole("alert")).toHaveTextContent("Não foi possível capturar a tela.");
    expect(startButton()).toBeInTheDocument();
  });

  it("shows the error when the caption cannot start", async () => {
    startCaption.mockRejectedValue({
      code: "invalid_input",
      message: "Ligue a legenda ao vivo nas Configurações.",
    });
    const user = userEvent.setup();
    await openFromScreen(true);

    await user.click(startButton());

    expect(await screen.findByRole("alert")).toHaveTextContent("Ligue a legenda ao vivo");
    expect(startButton()).toBeInTheDocument();
  });

  it("records a caption line confirmed with Enter as read from the screen", async () => {
    const user = userEvent.setup();
    await openFromScreen(true);
    await user.click(startButton());
    await waitFor(() => expect(captionHandler).toBeDefined());
    act(() => captionHandler?.("Hide!", true));
    await screen.findByText("pt:Hide!");

    await user.click(field());
    await user.keyboard("{Enter}");

    expect(recordHistory).toHaveBeenCalledExactlyOnceWith(
      expect.objectContaining({ sourceText: "Hide!", source: "ocr" }),
    );
  });

  it("waits for Enter before translating a caption line when auto-translate is off", async () => {
    const user = userEvent.setup();
    await openFromScreen(true);
    await user.click(startButton());
    await waitFor(() => expect(captionHandler).toBeDefined());
    translate.mockClear();

    act(() => captionHandler?.("Hide!", false));

    expect(field()).toHaveValue("Hide!");
    expect(screen.getByText("Enter traduz · Esc fecha")).toBeInTheDocument();
    expect(translate).not.toHaveBeenCalled();
  });

  it("does not show the caption as running when a reset arrives before it starts", async () => {
    let finishStart: () => void = () => {};
    startCaption.mockImplementation(
      () => new Promise<void>((resolve) => {
        finishStart = resolve;
      }),
    );
    const user = userEvent.setup();
    await openFromScreen(true);
    await user.click(startButton());

    act(() => resetHandler?.("Other", true, true, null, "ocr", true));
    await act(async () => finishStart());

    expect(startButton()).toBeInTheDocument();
  });

  it("a new reset ends the running state", async () => {
    const user = userEvent.setup();
    await openFromScreen(true);
    await user.click(startButton());
    await screen.findByRole("button", { name: "Parar legenda" });

    act(() => resetHandler?.("Other", true, true, null, "ocr", true));

    expect(startButton()).toBeInTheDocument();
  });
});

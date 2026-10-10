import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { shouldStartDrag } from "../../hooks/useWindowDrag";
import * as tauri from "../../lib/tauri";
import type { Translation } from "../../lib/types";
import PopupApp from "./PopupApp";

vi.mock("../../lib/tauri", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../lib/tauri")>()),
  translate: vi.fn(),
  copyToClipboard: vi.fn(),
  hidePopup: vi.fn(),
  onPopupReset: vi.fn(),
  setPopupPinned: vi.fn(),
  startPopupDragging: vi.fn(),
  startPopupResizing: vi.fn(),
}));

const translate = vi.mocked(tauri.translate);
const copyToClipboard = vi.mocked(tauri.copyToClipboard);
const hidePopup = vi.mocked(tauri.hidePopup);
const setPopupPinned = vi.mocked(tauri.setPopupPinned);
const startPopupDragging = vi.mocked(tauri.startPopupDragging);
const startPopupResizing = vi.mocked(tauri.startPopupResizing);

let resetHandler:
  | ((prefill: string | null, autoTranslate?: boolean, pinned?: boolean) => void)
  | undefined;

function translation(text: string): Translation {
  return { text, sourceLang: "EN", targetLang: "PT-BR" };
}

const pinButton = () => screen.getByRole("button", { name: "Fixar na tela" });
const field = () => screen.getByRole("textbox", { name: "Texto para traduzir" });

beforeEach(() => {
  resetHandler = undefined;
  translate.mockReset();
  copyToClipboard.mockReset().mockResolvedValue(undefined);
  hidePopup.mockReset().mockResolvedValue(undefined);
  setPopupPinned.mockReset().mockResolvedValue(undefined);
  startPopupDragging.mockReset().mockResolvedValue(undefined);
  startPopupResizing.mockReset().mockResolvedValue(undefined);
  vi.mocked(tauri.onPopupReset)
    .mockReset()
    .mockImplementation(async (handler) => {
      resetHandler = handler;
      return () => {};
    });
});

describe("PopupApp pin button", () => {
  it("shows the Lang icon next to the pin button", () => {
    render(<PopupApp />);

    const logo = screen.getByRole("img", { name: "Lang" });
    expect(logo.getAttribute("src")).toMatch(/lang-icon\.svg|^data:image\/svg\+xml/);
    expect(logo.parentElement).toContainElement(pinButton());
  });

  it("starts unpinned", () => {
    render(<PopupApp />);

    expect(pinButton()).toHaveAttribute("aria-pressed", "false");
  });

  it("pins and unpins the popup", async () => {
    const user = userEvent.setup();
    render(<PopupApp />);

    await user.click(pinButton());
    expect(setPopupPinned).toHaveBeenLastCalledWith(true);
    expect(pinButton()).toHaveAttribute("aria-pressed", "true");

    await user.click(pinButton());
    expect(setPopupPinned).toHaveBeenLastCalledWith(false);
    expect(pinButton()).toHaveAttribute("aria-pressed", "false");
  });

  it("keeps the focus in the text field when clicked", async () => {
    const user = userEvent.setup();
    render(<PopupApp />);
    field().focus();

    await user.click(pinButton());

    expect(field()).toHaveFocus();
  });

  it("goes back to unpinned and shows the error when the backend refuses", async () => {
    setPopupPinned.mockRejectedValue({ code: "window", message: "Não foi possível fixar." });
    const user = userEvent.setup();
    render(<PopupApp />);

    await user.click(pinButton());

    await waitFor(() => expect(pinButton()).toHaveAttribute("aria-pressed", "false"));
    expect(screen.getByRole("alert")).toHaveTextContent("Não foi possível fixar.");
  });

  it("follows what the backend says when the popup is shown again", async () => {
    const user = userEvent.setup();
    render(<PopupApp />);
    await user.click(pinButton());

    act(() => resetHandler?.(null, true, false));
    expect(pinButton()).toHaveAttribute("aria-pressed", "false");

    act(() => resetHandler?.("hello", true, true));
    expect(pinButton()).toHaveAttribute("aria-pressed", "true");
  });

  it("copies on Enter without closing while pinned", async () => {
    translate.mockResolvedValue(translation("olá"));
    const user = userEvent.setup();
    render(<PopupApp />);
    await user.click(pinButton());
    await user.type(field(), "hello");
    await screen.findByText("olá");

    await user.keyboard("{Enter}");

    expect(copyToClipboard).toHaveBeenCalledExactlyOnceWith("olá");
    expect(await screen.findByText("Copiado")).toBeInTheDocument();
    expect(hidePopup).not.toHaveBeenCalled();
    expect(field()).toHaveValue("hello");
  });
});

describe("moving the window", () => {
  it("starts dragging on a held double click over an empty area", () => {
    const { container } = render(<PopupApp />);

    fireEvent.mouseDown(container.querySelector(".popup")!, { button: 0, detail: 2 });

    expect(startPopupDragging).toHaveBeenCalledOnce();
  });

  it("does not drag on a single click", () => {
    const { container } = render(<PopupApp />);

    fireEvent.mouseDown(container.querySelector(".popup")!, { button: 0, detail: 1 });

    expect(startPopupDragging).not.toHaveBeenCalled();
  });

  it("leaves a double click in the text field for selecting words", () => {
    render(<PopupApp />);

    fireEvent.mouseDown(field(), { button: 0, detail: 2 });

    expect(startPopupDragging).not.toHaveBeenCalled();
  });

  it("leaves the translation text and the buttons alone", () => {
    const translationText = document.createElement("p");
    translationText.className = "popup__translation";
    const button = document.createElement("button");

    expect(shouldStartDrag({ button: 0, detail: 2, target: translationText })).toBe(false);
    expect(shouldStartDrag({ button: 0, detail: 2, target: button })).toBe(false);
    expect(shouldStartDrag({ button: 2, detail: 2, target: null })).toBe(false);
  });
});

describe("resizing the window", () => {
  it("starts resizing from the grip that was pressed", () => {
    const { container } = render(<PopupApp />);

    fireEvent.mouseDown(container.querySelector('[data-edge="southEast"]')!, { button: 0 });

    expect(startPopupResizing).toHaveBeenCalledExactlyOnceWith("southEast");
  });

  it("has a grip on every edge and corner", () => {
    const { container } = render(<PopupApp />);

    expect(container.querySelectorAll(".popup__grip")).toHaveLength(8);
  });

  it("ignores the right mouse button", () => {
    const { container } = render(<PopupApp />);

    fireEvent.mouseDown(container.querySelector('[data-edge="north"]')!, { button: 2 });

    expect(startPopupResizing).not.toHaveBeenCalled();
  });

  it("does not move the window when a grip is double clicked", () => {
    const { container } = render(<PopupApp />);

    fireEvent.mouseDown(container.querySelector('[data-edge="west"]')!, { button: 0, detail: 2 });

    expect(startPopupDragging).not.toHaveBeenCalled();
  });
});

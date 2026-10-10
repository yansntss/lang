import { fireEvent, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as tauri from "../../lib/tauri";
import PopupApp from "./PopupApp";

vi.mock("../../lib/tauri", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../lib/tauri")>()),
  translate: vi.fn(),
  hidePopup: vi.fn(),
  onPopupReset: vi.fn(),
  fitPopupHeight: vi.fn(),
}));

const fitPopupHeight = vi.mocked(tauri.fitPopupHeight);

// O jsdom não faz layout: as medidas vêm destes valores.
const POPUP_HEIGHT = 300;
const RESULT_AREA_HEIGHT = 100;
let contentHeight = 150;

let notifyContentResized: (() => void) | undefined;

class FakeResizeObserver {
  constructor(callback: () => void) {
    notifyContentResized = callback;
  }
  observe() {}
  disconnect() {}
  unobserve() {}
}

function stubLayout() {
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockImplementation(function (
    this: HTMLElement,
  ) {
    if (this.classList.contains("popup")) {
      return POPUP_HEIGHT;
    }
    return this.classList.contains("popup__content") ? contentHeight : 0;
  });
  vi.spyOn(Element.prototype, "clientHeight", "get").mockImplementation(function (
    this: Element,
  ) {
    return this.classList.contains("popup__result") ? RESULT_AREA_HEIGHT : 0;
  });
}

beforeEach(() => {
  contentHeight = 150;
  notifyContentResized = undefined;
  fitPopupHeight.mockReset().mockResolvedValue(undefined);
  vi.mocked(tauri.onPopupReset).mockReset().mockResolvedValue(() => {});
  vi.stubGlobal("ResizeObserver", FakeResizeObserver);
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("PopupApp window height", () => {
  it("asks for the height the content needs when it opens", () => {
    stubLayout();

    render(<PopupApp />);

    // 300 (janela) - 100 (área do resultado) + 150 (conteúdo)
    expect(fitPopupHeight).toHaveBeenCalledWith(350);
  });

  it("asks again when the content changes size", () => {
    stubLayout();
    render(<PopupApp />);
    fitPopupHeight.mockClear();

    contentHeight = 420;
    notifyContentResized?.();

    // 300 - 100 + 420
    expect(fitPopupHeight).toHaveBeenCalledWith(620);
  });

  it("asks again when the window regains focus, since it reopens at the chosen size", () => {
    stubLayout();
    render(<PopupApp />);
    fitPopupHeight.mockClear();

    fireEvent.focus(window);

    expect(fitPopupHeight).toHaveBeenCalledWith(350);
  });

  it("does not ask when the layout reports no height", () => {
    render(<PopupApp />);

    expect(fitPopupHeight).not.toHaveBeenCalled();
  });

  it("keeps working when the backend refuses the request", async () => {
    stubLayout();
    fitPopupHeight.mockRejectedValue(new Error("janela indisponível"));

    expect(() => render(<PopupApp />)).not.toThrow();
    await Promise.resolve();
  });
});

import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as tauri from "../../lib/tauri";
import RegionApp from "./RegionApp";

vi.mock("../../lib/tauri", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../lib/tauri")>()),
  getRegionSnapshot: vi.fn(),
  confirmRegion: vi.fn(),
  cancelRegion: vi.fn(),
  onRegionShow: vi.fn(),
}));

const getRegionSnapshot = vi.mocked(tauri.getRegionSnapshot);
const confirmRegion = vi.mocked(tauri.confirmRegion);
const cancelRegion = vi.mocked(tauri.cancelRegion);

let showHandler: (() => void) | undefined;

const area = () => screen.getByRole("application", { name: "Selecionar o texto na tela" });

function drag(from: [number, number], to: [number, number]) {
  fireEvent.pointerDown(area(), { button: 0, clientX: from[0], clientY: from[1] });
  fireEvent.pointerMove(area(), { clientX: to[0], clientY: to[1] });
  fireEvent.pointerUp(area(), { clientX: to[0], clientY: to[1] });
}

async function renderLoaded() {
  render(<RegionApp />);
  await act(async () => {});
}

beforeEach(() => {
  showHandler = undefined;
  // O jsdom não desenha em canvas: sem contexto, a captura só não aparece.
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
  getRegionSnapshot
    .mockReset()
    .mockResolvedValue({ width: 1, height: 1, pixels: new Uint8ClampedArray(4) });
  confirmRegion.mockReset().mockResolvedValue(undefined);
  cancelRegion.mockReset().mockResolvedValue(undefined);
  vi.mocked(tauri.onRegionShow)
    .mockReset()
    .mockImplementation(async (handler) => {
      showHandler = handler;
      return () => {};
    });
});

describe("RegionApp", () => {
  it("loads the capture when it opens and again every time it is shown", async () => {
    await renderLoaded();
    expect(getRegionSnapshot).toHaveBeenCalledTimes(1);

    await act(async () => showHandler?.());

    expect(getRegionSnapshot).toHaveBeenCalledTimes(2);
  });

  it("confirms the dragged area in window pixels", async () => {
    await renderLoaded();

    drag([100, 50], [400, 90]);

    expect(confirmRegion).toHaveBeenCalledExactlyOnceWith({
      x: 100,
      y: 50,
      width: 300,
      height: 40,
    });
  });

  it("sends the same area when dragging up and to the left", async () => {
    await renderLoaded();

    drag([400, 90], [100, 50]);

    expect(confirmRegion).toHaveBeenCalledExactlyOnceWith({
      x: 100,
      y: 50,
      width: 300,
      height: 40,
    });
  });

  it("shows the area being dragged", async () => {
    await renderLoaded();

    fireEvent.pointerDown(area(), { button: 0, clientX: 10, clientY: 20 });
    fireEvent.pointerMove(area(), { clientX: 110, clientY: 70 });

    const box = screen.getByTestId("region-selection");
    expect(box).toHaveStyle({ left: "10px", top: "20px", width: "100px", height: "50px" });
  });

  it("a click without dragging asks for an area instead of confirming", async () => {
    await renderLoaded();

    drag([100, 100], [102, 101]);

    expect(confirmRegion).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent("Arraste para selecionar");
  });

  it("ignores the right mouse button", async () => {
    await renderLoaded();

    fireEvent.pointerDown(area(), { button: 2, clientX: 100, clientY: 50 });
    fireEvent.pointerUp(area(), { clientX: 400, clientY: 90 });

    expect(confirmRegion).not.toHaveBeenCalled();
  });

  it("cancels with Esc", async () => {
    await renderLoaded();

    fireEvent.keyDown(document, { key: "Escape" });

    expect(cancelRegion).toHaveBeenCalledTimes(1);
  });

  it("shows why the backend refused the area and lets the user drag again", async () => {
    confirmRegion.mockRejectedValueOnce({
      code: "region_empty",
      message: "Selecione uma área maior, que contenha o texto.",
    });
    await renderLoaded();

    drag([100, 50], [400, 90]);

    expect(await screen.findByRole("alert")).toHaveTextContent("Selecione uma área maior");
    expect((document.querySelector("canvas") as HTMLCanvasElement).width).toBe(1);
    expect(screen.queryByTestId("region-selection")).not.toBeInTheDocument();

    drag([10, 10], [300, 200]);
    expect(confirmRegion).toHaveBeenCalledTimes(2);
  });

  it("a new capture clears the previous message and selection", async () => {
    confirmRegion.mockRejectedValueOnce({ code: "region_empty", message: "Área pequena." });
    await renderLoaded();
    drag([100, 50], [400, 90]);
    await screen.findByRole("alert");

    await act(async () => showHandler?.());

    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("drops the capture from the page when the selection ends", async () => {
    getRegionSnapshot.mockResolvedValue({
      width: 3,
      height: 2,
      pixels: new Uint8ClampedArray(3 * 2 * 4),
    });
    const { container } = render(<RegionApp />);
    await act(async () => {});
    const canvas = container.querySelector("canvas") as HTMLCanvasElement;
    expect(canvas.width).toBe(3);

    fireEvent.keyDown(document, { key: "Escape" });
    expect(canvas.width).toBe(0);

    await act(async () => showHandler?.());
    expect(canvas.width).toBe(3);
    drag([100, 50], [400, 90]);
    await act(async () => {});
    expect(canvas.width).toBe(0);

    await act(async () => showHandler?.());
    fireEvent.blur(window);
    expect(canvas.width).toBe(0);
  });

  it("shows an error when the capture cannot be loaded", async () => {
    getRegionSnapshot.mockRejectedValueOnce({
      code: "invalid_input",
      message: "Nenhuma captura em andamento.",
    });

    await renderLoaded();

    expect(screen.getByRole("alert")).toHaveTextContent("Nenhuma captura em andamento.");
  });
});

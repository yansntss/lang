import { useCallback, useEffect, useRef, useState, type PointerEvent } from "react";
import {
  cancelRegion,
  confirmRegion,
  errorMessage,
  getRegionSnapshot,
  onRegionShow,
} from "../../lib/tauri";
import type { RegionSelection, RegionSnapshot } from "../../lib/types";
import "./region.css";

/** Menor lado (pixels da janela) para valer como arrasto; menos que isso foi um clique. */
export const MIN_DRAG_PX = 6;

const CLICK_HINT = "Arraste para selecionar a área com o texto.";

interface Drag {
  startX: number;
  startY: number;
  x: number;
  y: number;
}

/** O retângulo do arrasto com largura e altura positivas, qualquer que seja a direção. */
function toSelection(drag: Drag): RegionSelection {
  return {
    x: Math.min(drag.startX, drag.x),
    y: Math.min(drag.startY, drag.y),
    width: Math.abs(drag.x - drag.startX),
    height: Math.abs(drag.y - drag.startY),
  };
}

/** Tira a captura da página: a janela oculta não guarda a imagem da tela. */
function clear(canvas: HTMLCanvasElement | null) {
  if (!canvas) return;
  canvas.width = 0;
  canvas.height = 0;
}

/** Desenha a captura em pixels físicos; o CSS a estica sobre a janela inteira. */
function draw(canvas: HTMLCanvasElement | null, snapshot: RegionSnapshot) {
  if (!canvas) return;
  canvas.width = snapshot.width;
  canvas.height = snapshot.height;
  const context = canvas.getContext("2d");
  context?.putImageData(new ImageData(snapshot.pixels, snapshot.width, snapshot.height), 0, 0);
}

/**
 * Janela de seleção de região: mostra a tela congelada e escurecida, e o usuário arrasta um
 * retângulo sobre o texto. Soltar o mouse confirma; Esc cancela.
 */
export default function RegionApp() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const load = useCallback(() => {
    setDrag(null);
    setMessage(null);
    // Nada da captura anterior aparece enquanto a nova chega.
    clear(canvasRef.current);
    getRegionSnapshot().then(
      (snapshot) => draw(canvasRef.current, snapshot),
      (error: unknown) => setMessage(errorMessage(error)),
    );
  }, []);

  useEffect(() => {
    let disposed = false;
    let stopListening: (() => void) | undefined;
    // Na primeira abertura a janela acabou de ser criada e pode ter perdido o evento.
    load();
    onRegionShow(load).then(
      (stop) => {
        if (disposed) stop();
        else stopListening = stop;
      },
      (error: unknown) => setMessage(errorMessage(error)),
    );
    return () => {
      disposed = true;
      stopListening?.();
    };
  }, [load]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        clear(canvasRef.current);
        cancelRegion().catch((error: unknown) => setMessage(errorMessage(error)));
      }
    };
    // Perder o foco cancela a seleção no backend.
    const onBlur = () => clear(canvasRef.current);
    document.addEventListener("keydown", onKeyDown);
    window.addEventListener("blur", onBlur);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("blur", onBlur);
    };
  }, []);

  const onPointerDown = (event: PointerEvent<HTMLElement>) => {
    if (event.button !== 0) return;
    // O jsdom (testes) não implementa a captura do ponteiro.
    event.currentTarget.setPointerCapture?.(event.pointerId);
    setMessage(null);
    setDrag({ startX: event.clientX, startY: event.clientY, x: event.clientX, y: event.clientY });
  };

  const onPointerMove = (event: PointerEvent<HTMLElement>) => {
    setDrag((current) => (current ? { ...current, x: event.clientX, y: event.clientY } : null));
  };

  const onPointerUp = (event: PointerEvent<HTMLElement>) => {
    if (!drag) return;
    const selection = toSelection({ ...drag, x: event.clientX, y: event.clientY });
    if (selection.width < MIN_DRAG_PX || selection.height < MIN_DRAG_PX) {
      setDrag(null);
      setMessage(CLICK_HINT);
      return;
    }
    setDrag(null);
    // Recusada, a seleção continua: a captura fica na tela para o usuário tentar de novo.
    confirmRegion(selection).then(
      () => clear(canvasRef.current),
      (error: unknown) => setMessage(errorMessage(error)),
    );
  };

  const selection = drag ? toSelection(drag) : null;

  return (
    <main
      className="region"
      role="application"
      aria-label="Selecionar o texto na tela"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
    >
      <canvas ref={canvasRef} className="region__image" aria-hidden="true" />
      {selection ? (
        <div
          data-testid="region-selection"
          className="region__selection"
          style={{
            left: `${selection.x}px`,
            top: `${selection.y}px`,
            width: `${selection.width}px`,
            height: `${selection.height}px`,
          }}
        />
      ) : (
        <div className="region__shade" />
      )}
      <p className="region__hint">Arraste sobre o texto · Esc cancela</p>
      {message && (
        <p role="alert" className="region__message">
          {message}
        </p>
      )}
    </main>
  );
}

import type { MouseEvent } from "react";
import { startPopupResizing } from "../../lib/tauri";
import type { ResizeEdge } from "../../lib/types";

const EDGES: readonly ResizeEdge[] = [
  "north",
  "south",
  "east",
  "west",
  "northEast",
  "northWest",
  "southEast",
  "southWest",
];

/**
 * Alças invisíveis nas bordas e nos cantos. A borda nativa de uma janela sem decoração tem só
 * alguns pixels e não aparece, então o arrasto começa aqui e o sistema o conduz.
 */
export default function ResizeGrips() {
  const startResizing = (edge: ResizeEdge) => (event: MouseEvent) => {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    // Se o sistema recusar, o tamanho só não muda: não há o que avisar ao usuário.
    startPopupResizing(edge).catch(() => {});
  };

  return (
    <>
      {EDGES.map((edge) => (
        <div
          key={edge}
          className={`popup__grip popup__grip--${edge}`}
          data-edge={edge}
          aria-hidden="true"
          onMouseDown={startResizing(edge)}
        />
      ))}
    </>
  );
}

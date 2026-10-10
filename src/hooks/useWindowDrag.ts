import { useEffect, type RefObject } from "react";
import { startPopupDragging } from "../lib/tauri";

/**
 * Onde o duplo clique tem outro uso e não move a janela: digitar, clicar em botões, selecionar
 * uma palavra da tradução ou da explicação, e as alças de redimensionar.
 */
const NOT_DRAGGABLE =
  "textarea, input, button, a, .popup__translation, .popup__explanation-text, .popup__grip";

interface DragStart {
  button: number;
  detail: number;
  target: EventTarget | null;
}

/** O segundo clique de um duplo clique, segurado, numa área sem outro uso. */
export function shouldStartDrag({ button, detail, target }: DragStart): boolean {
  if (button !== 0 || detail !== 2) {
    return false;
  }
  return !(target instanceof Element && target.closest(NOT_DRAGGABLE));
}

/** Duplo clique e segurar dentro de `ref` move a janela, que não tem barra de título. */
export function useWindowDrag(ref: RefObject<HTMLElement | null>): void {
  useEffect(() => {
    const element = ref.current;
    if (!element) {
      return;
    }

    const onMouseDown = (event: MouseEvent) => {
      if (!shouldStartDrag(event)) {
        return;
      }
      // Sem isso o duplo clique também selecionaria texto ao redor.
      event.preventDefault();
      // Se o sistema recusar, a janela só não se move: não há o que avisar ao usuário.
      startPopupDragging().catch(() => {});
    };

    element.addEventListener("mousedown", onMouseDown);
    return () => element.removeEventListener("mousedown", onMouseDown);
  }, [ref]);
}

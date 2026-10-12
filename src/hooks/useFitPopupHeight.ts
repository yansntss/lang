import { useEffect, type RefObject } from "react";
import { fitPopupHeight } from "../lib/tauri";

/**
 * A altura que o popup precisa para mostrar o conteúdo inteiro: tudo o que não é a área do
 * resultado (margens, campo de texto) mais a altura natural do conteúdo do resultado.
 */
export function neededHeight(popup: HTMLElement, result: HTMLElement, content: HTMLElement) {
  return popup.offsetHeight - result.clientHeight + content.offsetHeight;
}

/**
 * Avisa o backend da altura que o conteúdo pede, para a janela crescer sozinha (e voltar ao
 * tamanho escolhido pelo usuário quando o conteúdo diminui). Reavalia a cada mudança do
 * conteúdo e do campo de texto (que cresce com o que é digitado) e quando a janela recebe o
 * foco, pois ela é reexibida no tamanho escolhido.
 */
export function useFitPopupHeight(
  popupRef: RefObject<HTMLElement | null>,
  inputRef: RefObject<HTMLElement | null>,
  resultRef: RefObject<HTMLElement | null>,
  contentRef: RefObject<HTMLElement | null>,
): void {
  useEffect(() => {
    const popup = popupRef.current;
    const input = inputRef.current;
    const result = resultRef.current;
    const content = contentRef.current;
    if (!popup || !input || !result || !content) {
      return;
    }

    const fit = () => {
      const height = neededHeight(popup, result, content);
      if (height > 0) {
        // Não crescer é só estético: o conteúdo continua acessível pela rolagem.
        fitPopupHeight(height).catch(() => {});
      }
    };

    fit();
    window.addEventListener("focus", fit);
    const observer = typeof ResizeObserver === "undefined" ? undefined : new ResizeObserver(fit);
    observer?.observe(content);
    observer?.observe(input);
    return () => {
      window.removeEventListener("focus", fit);
      observer?.disconnect();
    };
  }, [popupRef, inputRef, resultRef, contentRef]);
}

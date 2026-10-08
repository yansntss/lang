import { useEffect, useRef } from "react";
import { getSettings } from "../lib/tauri";
import type { Theme } from "../lib/types";

/** "Sistema" remove o override e deixa valer `prefers-color-scheme`. */
export function applyTheme(theme: Theme): void {
  const root = document.documentElement;
  if (theme === "system") {
    delete root.dataset.theme;
  } else {
    root.dataset.theme = theme;
  }
}

/**
 * Aplica o tema salvo à janela, ao abrir e sempre que ela recebe foco (a janela de
 * configurações pode tê-lo mudado enquanto esta estava oculta). Se não conseguir ler as
 * configurações, mantém o tema do sistema: é só aparência.
 */
export function useTheme(): void {
  // Foco repetido gera leituras em sequência; só a última resposta vale.
  const latest = useRef(0);

  useEffect(() => {
    let disposed = false;
    const refresh = () => {
      latest.current += 1;
      const thisRead = latest.current;
      getSettings().then(
        (settings) => {
          if (!disposed && thisRead === latest.current) applyTheme(settings.theme);
        },
        () => {},
      );
    };

    refresh();
    window.addEventListener("focus", refresh);
    return () => {
      disposed = true;
      window.removeEventListener("focus", refresh);
    };
  }, []);
}

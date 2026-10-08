import { useEffect, useState } from "react";
import { hasSecret } from "../lib/tauri";
import type { SecretKind } from "../lib/types";

/**
 * Diz se a chave `kind` está configurada. Consulta de novo sempre que a janela recebe foco:
 * o usuário pode ter cadastrado a chave nas configurações enquanto o popup estava oculto.
 */
export function useHasSecret(kind: SecretKind): boolean {
  const [configured, setConfigured] = useState(false);

  useEffect(() => {
    let disposed = false;
    const refresh = () => {
      hasSecret(kind).then(
        (value) => {
          if (!disposed) setConfigured(value);
        },
        () => {
          if (!disposed) setConfigured(false);
        },
      );
    };

    refresh();
    window.addEventListener("focus", refresh);
    return () => {
      disposed = true;
      window.removeEventListener("focus", refresh);
    };
  }, [kind]);

  return configured;
}

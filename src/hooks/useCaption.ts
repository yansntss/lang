import { useCallback, useEffect, useRef, useState } from "react";
import { errorMessage, onCaption, onCaptionStopped, startCaption, stopCaption } from "../lib/tauri";

interface CaptionHandlers {
  /** Uma fala nova foi lida da tela. */
  onLine: (text: string, autoTranslate: boolean) => void;
  onError: (message: string) => void;
}

export interface UseCaption {
  running: boolean;
  start: () => void;
  stop: () => void;
  /** O popup foi reaberto: o backend já encerrou a legenda anterior. */
  forget: () => void;
}

/** Liga e desliga a legenda ao vivo e entrega cada fala nova lida da tela. */
export function useCaption({ onLine, onError }: CaptionHandlers): UseCaption {
  const [running, setRunning] = useState(false);
  // Muda a cada parada (pelo usuário, sozinha ou por um reset): um `start` que termine depois
  // dela não religa o estado.
  const epoch = useRef(0);
  const handlers = useRef({ onLine, onError });
  useEffect(() => {
    handlers.current = { onLine, onError };
  });

  useEffect(() => {
    let disposed = false;
    const stops: Array<() => void> = [];
    const keep = (stop: () => void) => {
      if (disposed) stop();
      else stops.push(stop);
    };
    const fail = (error: unknown) => handlers.current.onError(errorMessage(error));

    onCaption((text, autoTranslate) => handlers.current.onLine(text, autoTranslate)).then(
      keep,
      fail,
    );
    onCaptionStopped((notice) => {
      epoch.current += 1;
      setRunning(false);
      handlers.current.onError(notice);
    }).then(keep, fail);

    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
    };
  }, []);

  const start = useCallback(() => {
    const started = epoch.current;
    startCaption().then(
      () => {
        if (epoch.current === started) setRunning(true);
      },
      (error: unknown) => handlers.current.onError(errorMessage(error)),
    );
  }, []);

  const stop = useCallback(() => {
    epoch.current += 1;
    setRunning(false);
    stopCaption().catch((error: unknown) => handlers.current.onError(errorMessage(error)));
  }, []);

  const forget = useCallback(() => {
    epoch.current += 1;
    setRunning(false);
  }, []);

  return { running, start, stop, forget };
}

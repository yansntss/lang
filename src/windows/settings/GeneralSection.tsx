import { useRef, useState } from "react";
import { errorMessage, setAutostart, updateSettings } from "../../lib/tauri";
import type { Preferences, SettingsView } from "../../lib/types";

interface GeneralSectionProps {
  settings: SettingsView;
  /** Recebe só o trecho que o comando alterou, para não desfazer o que outra seção mudou. */
  onChanged: (patch: Partial<SettingsView>) => void;
}

function preferencesOf(view: SettingsView): Preferences {
  return {
    theme: view.theme,
    englishVariant: view.englishVariant,
    portugueseVariant: view.portugueseVariant,
    explainModel: view.explainModel,
    captureInEditors: view.captureInEditors,
    autoTranslateSelection: view.autoTranslateSelection,
  };
}

/**
 * Preferências gerais. Cada alteração é salva na hora, uma de cada vez: enquanto uma está em
 * andamento as outras são ignoradas, então nenhuma parte da tela fica diferente do que foi
 * salvo. Se falhar, a tela mostra o motivo e mantém o valor anterior.
 */
export default function GeneralSection({ settings, onChanged }: GeneralSectionProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Síncrono: o estado só muda no próximo render, e dois eventos podem chegar antes dele.
  const busyRef = useRef(false);

  const run = async (action: () => Promise<void>) => {
    if (busyRef.current) {
      return;
    }
    busyRef.current = true;
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (failure: unknown) {
      setError(errorMessage(failure));
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  };

  const change = (patch: Partial<Preferences>) =>
    run(async () => {
      const saved = await updateSettings({ ...preferencesOf(settings), ...patch });
      onChanged(preferencesOf(saved));
    });

  const changeAutostart = (enabled: boolean) =>
    run(async () => {
      await setAutostart(enabled);
      onChanged({ autostart: enabled });
    });

  return (
    <section aria-labelledby="general-title" aria-busy={busy} className="settings__section">
      <h2 id="general-title">Geral</h2>

      <label className="settings__check">
        <input
          type="checkbox"
          checked={settings.autostart}
          onChange={(event) => void changeAutostart(event.target.checked)}
        />
        Iniciar com o Windows
      </label>

      <div className="settings__field">
        <label htmlFor="theme">Tema</label>
        <select
          id="theme"
          value={settings.theme}
          onChange={(event) => void change({ theme: event.target.value as Preferences["theme"] })}
        >
          <option value="system">Seguir o sistema</option>
          <option value="light">Claro</option>
          <option value="dark">Escuro</option>
        </select>
      </div>

      <div className="settings__field">
        <label htmlFor="english-variant">Inglês</label>
        <select
          id="english-variant"
          value={settings.englishVariant}
          onChange={(event) =>
            void change({ englishVariant: event.target.value as Preferences["englishVariant"] })
          }
        >
          <option value="EN-US">Americano (EN-US)</option>
          <option value="EN-GB">Britânico (EN-GB)</option>
        </select>
      </div>

      <div className="settings__field">
        <label htmlFor="portuguese-variant">Português</label>
        <select
          id="portuguese-variant"
          value={settings.portugueseVariant}
          onChange={(event) =>
            void change({
              portugueseVariant: event.target.value as Preferences["portugueseVariant"],
            })
          }
        >
          <option value="PT-BR">Brasileiro (PT-BR)</option>
          <option value="PT-PT">Europeu (PT-PT)</option>
        </select>
      </div>

      <div className="settings__field">
        <label htmlFor="explain-model">Modelo do Explicar</label>
        <select
          id="explain-model"
          value={settings.explainModel}
          onChange={(event) =>
            void change({ explainModel: event.target.value as Preferences["explainModel"] })
          }
        >
          <option value="claude-haiku-5-5">Haiku (rápido e barato)</option>
          <option value="claude-sonnet-5-5">Sonnet (mais detalhado)</option>
        </select>
      </div>

      <label className="settings__check">
        <input
          type="checkbox"
          checked={settings.autoTranslateSelection}
          aria-describedby="auto-translate-hint"
          onChange={(event) => void change({ autoTranslateSelection: event.target.checked })}
        />
        Traduzir a seleção assim que o atalho abre o tradutor
      </label>
      <p id="auto-translate-hint" className="settings__hint">
        Desligado, o texto selecionado só é enviado ao DeepL depois que você aperta Enter.
      </p>

      <label className="settings__check">
        <input
          type="checkbox"
          checked={settings.captureInEditors}
          aria-describedby="editors-hint"
          onChange={(event) => void change({ captureInEditors: event.target.checked })}
        />
        Capturar a seleção em editores de código (VS Code, Cursor, JetBrains…)
      </label>
      <p id="editors-hint" className="settings__hint">
        Atenção: o terminal embutido desses editores trata o Ctrl+C simulado como interrupção e
        pode encerrar um comando em execução. Terminais comuns continuam bloqueados.
      </p>

      {error && (
        <p role="alert" className="settings__feedback settings__feedback--error">
          {error}
        </p>
      )}
    </section>
  );
}

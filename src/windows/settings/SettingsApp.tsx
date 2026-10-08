import { useCallback, useEffect, useState } from "react";
import { applyTheme } from "../../hooks/useTheme";
import { errorMessage, getSettings } from "../../lib/tauri";
import type { SettingsView } from "../../lib/types";
import "../../styles/base.css";
import GeneralSection from "./GeneralSection";
import SecretSection from "./SecretSection";
import ShortcutSection from "./ShortcutSection";
import "./settings.css";

const DEEPL_HINT =
  "A chave fica guardada no Gerenciador de Credenciais do Windows e nunca é exibida de volta. " +
  "Chaves do plano gratuito terminam em :fx.";
const ANTHROPIC_HINT =
  "Opcional: só o botão Explicar usa. A chave fica guardada no Gerenciador de Credenciais do " +
  "Windows e nunca é exibida de volta.";

export default function SettingsApp() {
  const [settings, setSettings] = useState<SettingsView | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getSettings().then(
      (loaded) => {
        if (!cancelled) setSettings(loaded);
      },
      (failure: unknown) => {
        if (!cancelled) setLoadError(errorMessage(failure));
      },
    );
    return () => {
      cancelled = true;
    };
  }, []);

  // Cada comando devolve só o trecho que mexeu; juntar sobre o estado mais recente impede que
  // a resposta de uma seção desfaça a de outra.
  const handleChanged = useCallback((patch: Partial<SettingsView>) => {
    setSettings((current) => (current ? { ...current, ...patch } : current));
  }, []);

  const theme = settings?.theme;
  useEffect(() => {
    if (theme) applyTheme(theme);
  }, [theme]);

  return (
    <main className="settings">
      <h1>Configurações</h1>
      <SecretSection kind="deepl" title="DeepL" article="do" hint={DEEPL_HINT} />
      <SecretSection kind="anthropic" title="Anthropic" article="da" hint={ANTHROPIC_HINT} />
      {settings ? (
        <>
          <ShortcutSection shortcut={settings.shortcut} onChanged={handleChanged} />
          <GeneralSection settings={settings} onChanged={handleChanged} />
        </>
      ) : loadError ? (
        <p role="alert" className="settings__feedback settings__feedback--error">
          {loadError}
        </p>
      ) : (
        <p className="settings__hint">Carregando as configurações…</p>
      )}
    </main>
  );
}

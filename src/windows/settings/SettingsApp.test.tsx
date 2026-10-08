import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as tauri from "../../lib/tauri";
import type { SettingsView } from "../../lib/types";
import SettingsApp from "./SettingsApp";

vi.mock("../../lib/tauri", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../lib/tauri")>()),
  hasSecret: vi.fn(),
  setSecret: vi.fn(),
  deleteSecret: vi.fn(),
  testSecret: vi.fn(),
  getSettings: vi.fn(),
  updateSettings: vi.fn(),
  setShortcut: vi.fn(),
  setAutostart: vi.fn(),
}));

const hasSecret = vi.mocked(tauri.hasSecret);
const setSecret = vi.mocked(tauri.setSecret);
const deleteSecret = vi.mocked(tauri.deleteSecret);
const testSecret = vi.mocked(tauri.testSecret);
const getSettings = vi.mocked(tauri.getSettings);
const updateSettings = vi.mocked(tauri.updateSettings);
const setShortcut = vi.mocked(tauri.setShortcut);
const setAutostart = vi.mocked(tauri.setAutostart);

const NOT_CONFIGURED = "Chave não configurada";
const CONFIGURED = "Chave configurada ✓";
const BACKEND_ERROR = { code: "settings", message: "Não foi possível salvar as configurações." };

const BASE: SettingsView = {
  shortcut: "Ctrl+Alt+T",
  autostart: false,
  theme: "system",
  englishVariant: "EN-US",
  portugueseVariant: "PT-BR",
  explainModel: "claude-haiku-5-5",
  captureInEditors: false,
  autoTranslateSelection: true,
};

const section = (name: string) => screen.getByRole("region", { name });
const deepl = () => section("DeepL");
const anthropic = () => section("Anthropic");
const shortcut = () => section("Atalho global");
const general = () => section("Geral");

async function renderSettled() {
  render(<SettingsApp />);
  await screen.findByRole("region", { name: "Geral" });
  await waitFor(() => expect(within(deepl()).queryByText("Verificando…")).not.toBeInTheDocument());
}

beforeEach(() => {
  delete document.documentElement.dataset.theme;
  hasSecret.mockReset().mockResolvedValue(false);
  setSecret.mockReset().mockResolvedValue(undefined);
  deleteSecret.mockReset().mockResolvedValue(undefined);
  testSecret.mockReset();
  getSettings.mockReset().mockResolvedValue(BASE);
  updateSettings.mockReset();
  setShortcut.mockReset();
  setAutostart.mockReset().mockResolvedValue(undefined);
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

describe("SettingsApp — chaves", () => {
  it("sends the key only once when the form is submitted twice in a row", async () => {
    const user = userEvent.setup();
    await renderSettled();
    const field = within(deepl()).getByLabelText("Chave da API do DeepL");
    await user.type(field, "abc:fx");
    const form = field.closest("form") as HTMLFormElement;

    fireEvent.submit(form);
    fireEvent.submit(form);

    await within(deepl()).findByText(CONFIGURED);
    expect(setSecret).toHaveBeenCalledExactlyOnceWith("deepl", "abc:fx");
  });

  it("does not let a late initial status overwrite a key that was just saved", async () => {
    const initial = deferred<boolean>();
    hasSecret.mockImplementation((kind) =>
      kind === "deepl" ? initial.promise : Promise.resolve(false),
    );
    const user = userEvent.setup();
    render(<SettingsApp />);
    const region = await screen.findByRole("region", { name: "DeepL" });

    await user.type(within(region).getByLabelText("Chave da API do DeepL"), "abc:fx");
    await user.click(within(region).getByRole("button", { name: "Salvar chave" }));
    await within(region).findByText(CONFIGURED);
    initial.resolve(false);
    await Promise.resolve();

    expect(within(region).getByText(CONFIGURED)).toBeInTheDocument();
  });

  it("renders the heading and one section per key", async () => {
    await renderSettled();

    expect(screen.getByRole("heading", { name: "Configurações" })).toBeInTheDocument();
    expect(deepl()).toBeInTheDocument();
    expect(anthropic()).toBeInTheDocument();
  });

  it("asks the backend whether each key is stored", async () => {
    await renderSettled();

    expect(hasSecret).toHaveBeenCalledWith("deepl");
    expect(hasSecret).toHaveBeenCalledWith("anthropic");
  });

  it("shows which keys are already configured", async () => {
    hasSecret.mockImplementation(async (kind) => kind === "deepl");

    await renderSettled();

    expect(await within(deepl()).findByText(CONFIGURED)).toBeInTheDocument();
    expect(await within(anthropic()).findByText(NOT_CONFIGURED)).toBeInTheDocument();
  });

  it("uses a masked field that does not autocomplete", async () => {
    await renderSettled();

    const field = within(deepl()).getByLabelText("Chave da API do DeepL");
    expect(field).toHaveAttribute("type", "password");
    expect(field).toHaveAttribute("autocomplete", "off");
    expect(within(anthropic()).getByLabelText("Chave da API da Anthropic")).toBeInTheDocument();
  });

  it("keeps the save button disabled until a key is typed", async () => {
    const user = userEvent.setup();
    await renderSettled();
    const save = within(deepl()).getByRole("button", { name: "Salvar chave" });
    const field = within(deepl()).getByLabelText("Chave da API do DeepL");

    expect(save).toBeDisabled();
    await user.type(field, "   ");
    expect(save).toBeDisabled();
    await user.type(field, "abc:fx");
    expect(save).toBeEnabled();
  });

  it("saves the key, clears the field and shows the configured state", async () => {
    const user = userEvent.setup();
    await renderSettled();
    const field = within(deepl()).getByLabelText("Chave da API do DeepL");

    await user.type(field, "abc:fx");
    await user.click(within(deepl()).getByRole("button", { name: "Salvar chave" }));

    expect(await within(deepl()).findByText(CONFIGURED)).toBeInTheDocument();
    expect(setSecret).toHaveBeenCalledExactlyOnceWith("deepl", "abc:fx");
    expect(field).toHaveValue("");
    expect(within(deepl()).getByRole("status")).toHaveTextContent("Chave salva.");
  });

  it("saves the Anthropic key under its own kind", async () => {
    const user = userEvent.setup();
    await renderSettled();

    await user.type(within(anthropic()).getByLabelText("Chave da API da Anthropic"), "sk-ant-x");
    await user.click(within(anthropic()).getByRole("button", { name: "Salvar chave" }));

    await waitFor(() => expect(setSecret).toHaveBeenCalledWith("anthropic", "sk-ant-x"));
    expect(deepl()).toHaveTextContent(NOT_CONFIGURED);
  });

  it("shows the backend error when saving fails and stays unconfigured", async () => {
    setSecret.mockRejectedValue({
      code: "secret_store",
      message: "Falha ao acessar o armazenamento seguro de chaves.",
    });
    const user = userEvent.setup();
    await renderSettled();

    await user.type(within(deepl()).getByLabelText("Chave da API do DeepL"), "abc:fx");
    await user.click(within(deepl()).getByRole("button", { name: "Salvar chave" }));

    const alert = await within(deepl()).findByRole("alert");
    expect(alert).toHaveTextContent("armazenamento seguro");
    expect(alert).toHaveTextContent("Digite a chave novamente.");
    expect(within(deepl()).getByText(NOT_CONFIGURED)).toBeInTheDocument();
  });

  it("removes the stored key", async () => {
    hasSecret.mockResolvedValue(true);
    const user = userEvent.setup();
    await renderSettled();
    await within(deepl()).findByText(CONFIGURED);

    await user.click(within(deepl()).getByRole("button", { name: "Remover chave" }));

    expect(await within(deepl()).findByText(NOT_CONFIGURED)).toBeInTheDocument();
    expect(deleteSecret).toHaveBeenCalledExactlyOnceWith("deepl");
  });

  it("offers neither removal nor testing when no key is stored", async () => {
    await renderSettled();

    expect(
      within(deepl()).queryByRole("button", { name: "Remover chave" }),
    ).not.toBeInTheDocument();
    expect(
      within(deepl()).queryByRole("button", { name: "Testar chave" }),
    ).not.toBeInTheDocument();
  });

  it("reports a failure to read the key status", async () => {
    hasSecret.mockRejectedValue({ code: "secret_store", message: "Falha ao acessar o cofre." });
    render(<SettingsApp />);
    const region = await screen.findByRole("region", { name: "DeepL" });

    expect(await within(region).findByRole("alert")).toHaveTextContent(
      "Falha ao acessar o cofre.",
    );
  });

  it("tests the DeepL key and shows this month's usage", async () => {
    hasSecret.mockImplementation(async (kind) => kind === "deepl");
    testSecret.mockResolvedValue({ usage: { used: 1234, limit: 500000 } });
    const user = userEvent.setup();
    await renderSettled();
    await within(deepl()).findByText(CONFIGURED);

    await user.click(within(deepl()).getByRole("button", { name: "Testar chave" }));

    const result = await within(deepl()).findByRole("status");
    expect(testSecret).toHaveBeenCalledExactlyOnceWith("deepl");
    expect(result).toHaveTextContent("Chave válida ✓");
    expect(result).toHaveTextContent(/Uso neste mês: 1\D?234 de 500\D?000 caracteres/);
  });

  it("tests the Anthropic key, which reports no usage", async () => {
    hasSecret.mockImplementation(async (kind) => kind === "anthropic");
    testSecret.mockResolvedValue({ usage: null });
    const user = userEvent.setup();
    await renderSettled();
    await within(anthropic()).findByText(CONFIGURED);

    await user.click(within(anthropic()).getByRole("button", { name: "Testar chave" }));

    expect(await within(anthropic()).findByRole("status")).toHaveTextContent("Chave válida ✓");
    expect(testSecret).toHaveBeenCalledExactlyOnceWith("anthropic");
  });

  it("shows why the key test failed", async () => {
    hasSecret.mockResolvedValue(true);
    testSecret.mockRejectedValue({
      code: "invalid_api_key",
      message: "A chave de API do DeepL é inválida ou foi recusada.",
    });
    const user = userEvent.setup();
    await renderSettled();
    await within(deepl()).findByText(CONFIGURED);

    await user.click(within(deepl()).getByRole("button", { name: "Testar chave" }));

    expect(await within(deepl()).findByRole("alert")).toHaveTextContent("inválida ou foi recusada");
  });
});

describe("SettingsApp — atalho", () => {
  it("shows the current shortcut", async () => {
    await renderSettled();

    expect(within(shortcut()).getByText("Ctrl+Alt+T", { selector: "kbd" })).toBeInTheDocument();
  });

  it("records the next combination, applies it and shows the new shortcut", async () => {
    setShortcut.mockResolvedValue({ ...BASE, shortcut: "Ctrl+Alt+K" });
    const user = userEvent.setup();
    await renderSettled();
    await user.click(within(shortcut()).getByRole("button", { name: "Alterar atalho" }));

    expect(screen.getByText(/Aperte a nova combinação/)).toBeInTheDocument();
    fireEvent.keyDown(window, { code: "ControlLeft", ctrlKey: true });
    fireEvent.keyDown(window, { code: "KeyK", ctrlKey: true, altKey: true });

    expect(await screen.findByText("Atalho alterado para Ctrl+Alt+K.")).toBeInTheDocument();
    expect(setShortcut).toHaveBeenCalledExactlyOnceWith("Ctrl+Alt+K");
    expect(within(shortcut()).getByText("Ctrl+Alt+K", { selector: "kbd" })).toBeInTheDocument();
    expect(screen.queryByText(/Aperte a nova combinação/)).not.toBeInTheDocument();
  });

  it("keeps the current shortcut and shows why when the system refuses the new one", async () => {
    setShortcut.mockRejectedValue({
      code: "invalid_input",
      message: "Não foi possível usar esse atalho. Outro app pode já estar usando.",
    });
    const user = userEvent.setup();
    await renderSettled();
    await user.click(within(shortcut()).getByRole("button", { name: "Alterar atalho" }));

    fireEvent.keyDown(window, { code: "KeyK", ctrlKey: true, altKey: true });

    expect(await within(shortcut()).findByRole("alert")).toHaveTextContent(
      "Outro app pode já estar usando",
    );
    expect(within(shortcut()).getByText("Ctrl+Alt+T", { selector: "kbd" })).toBeInTheDocument();
  });

  it("does not call the backend for a key the shortcut cannot use", async () => {
    const user = userEvent.setup();
    await renderSettled();
    await user.click(within(shortcut()).getByRole("button", { name: "Alterar atalho" }));

    fireEvent.keyDown(window, { code: "Enter", ctrlKey: true });

    expect(await within(shortcut()).findByRole("alert")).toHaveTextContent(
      "letra, um número ou F1 a F12",
    );
    expect(setShortcut).not.toHaveBeenCalled();
  });

  it("asks for Ctrl, Alt or Win before calling the backend when only a letter is pressed", async () => {
    const user = userEvent.setup();
    await renderSettled();
    await user.click(within(shortcut()).getByRole("button", { name: "Alterar atalho" }));

    fireEvent.keyDown(window, { code: "KeyK" });

    expect(await within(shortcut()).findByRole("alert")).toHaveTextContent(
      "Use Ctrl, Alt ou Win",
    );
    expect(setShortcut).not.toHaveBeenCalled();
  });

  it("lets Tab, Enter and Space through while recording so the keyboard is never trapped", async () => {
    const user = userEvent.setup();
    await renderSettled();
    await user.click(within(shortcut()).getByRole("button", { name: "Alterar atalho" }));

    for (const code of ["Tab", "Enter", "Space"]) {
      const notPrevented = fireEvent.keyDown(window, { code });
      expect(notPrevented, code).toBe(true);
    }

    expect(screen.getByText(/Aperte a nova combinação/)).toBeInTheDocument();
    expect(setShortcut).not.toHaveBeenCalled();
  });

  it("swallows a combination while recording so it does not act on the window", async () => {
    const user = userEvent.setup();
    await renderSettled();
    await user.click(within(shortcut()).getByRole("button", { name: "Alterar atalho" }));

    const notPrevented = fireEvent.keyDown(window, { code: "Tab", ctrlKey: true });

    expect(notPrevented).toBe(false);
  });

  it("ignores the repeats of a held key", async () => {
    const user = userEvent.setup();
    await renderSettled();
    await user.click(within(shortcut()).getByRole("button", { name: "Alterar atalho" }));

    fireEvent.keyDown(window, { code: "KeyK", ctrlKey: true, altKey: true, repeat: true });

    expect(setShortcut).not.toHaveBeenCalled();
    expect(screen.getByText(/Aperte a nova combinação/)).toBeInTheDocument();
  });

  it("cancels the recording with Escape", async () => {
    const user = userEvent.setup();
    await renderSettled();
    await user.click(within(shortcut()).getByRole("button", { name: "Alterar atalho" }));

    fireEvent.keyDown(window, { code: "Escape" });

    await waitFor(() =>
      expect(screen.queryByText(/Aperte a nova combinação/)).not.toBeInTheDocument(),
    );
    expect(setShortcut).not.toHaveBeenCalled();
  });

  it("cancels the recording with the same button", async () => {
    const user = userEvent.setup();
    await renderSettled();

    await user.click(within(shortcut()).getByRole("button", { name: "Alterar atalho" }));
    await user.click(within(shortcut()).getByRole("button", { name: "Cancelar" }));

    expect(screen.queryByText(/Aperte a nova combinação/)).not.toBeInTheDocument();
  });

  it("only listens to the keyboard while recording", async () => {
    await renderSettled();

    fireEvent.keyDown(window, { code: "KeyK", ctrlKey: true, altKey: true });

    expect(setShortcut).not.toHaveBeenCalled();
  });
});

describe("SettingsApp — geral", () => {
  it("shows the saved preferences", async () => {
    getSettings.mockResolvedValue({
      ...BASE,
      autostart: true,
      theme: "dark",
      englishVariant: "EN-GB",
      portugueseVariant: "PT-PT",
      explainModel: "claude-sonnet-5-5",
      captureInEditors: true,
      autoTranslateSelection: false,
    });

    await renderSettled();

    expect(within(general()).getByLabelText("Iniciar com o Windows")).toBeChecked();
    expect(within(general()).getByLabelText("Tema")).toHaveValue("dark");
    expect(within(general()).getByLabelText("Inglês")).toHaveValue("EN-GB");
    expect(within(general()).getByLabelText("Português")).toHaveValue("PT-PT");
    expect(within(general()).getByLabelText("Modelo do Explicar")).toHaveValue(
      "claude-sonnet-5-5",
    );
    expect(within(general()).getByLabelText(/Capturar a seleção em editores/)).toBeChecked();
    expect(within(general()).getByLabelText(/Traduzir a seleção assim que/)).not.toBeChecked();
  });

  it("applies the saved theme to the window", async () => {
    getSettings.mockResolvedValue({ ...BASE, theme: "dark" });

    await renderSettled();

    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("saves a preference with all the others unchanged and applies the new theme", async () => {
    updateSettings.mockResolvedValue({ ...BASE, theme: "light" });
    const user = userEvent.setup();
    await renderSettled();

    await user.selectOptions(within(general()).getByLabelText("Tema"), "light");

    expect(updateSettings).toHaveBeenCalledExactlyOnceWith({
      theme: "light",
      englishVariant: "EN-US",
      portugueseVariant: "PT-BR",
      explainModel: "claude-haiku-5-5",
      captureInEditors: false,
      autoTranslateSelection: true,
    });
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe("light"));
  });

  it("goes back to the system theme by removing the override", async () => {
    getSettings.mockResolvedValue({ ...BASE, theme: "dark" });
    updateSettings.mockResolvedValue({ ...BASE, theme: "system" });
    const user = userEvent.setup();
    await renderSettled();

    await user.selectOptions(within(general()).getByLabelText("Tema"), "system");

    await waitFor(() => expect(document.documentElement.dataset.theme).toBeUndefined());
  });

  it("changes the language variants and the Explain model", async () => {
    updateSettings.mockImplementation(async (preferences) => ({ ...BASE, ...preferences }));
    const user = userEvent.setup();
    await renderSettled();

    await user.selectOptions(within(general()).getByLabelText("Inglês"), "EN-GB");
    await user.selectOptions(within(general()).getByLabelText("Português"), "PT-PT");
    await user.selectOptions(
      within(general()).getByLabelText("Modelo do Explicar"),
      "claude-sonnet-5-5",
    );

    expect(updateSettings).toHaveBeenLastCalledWith({
      theme: "system",
      englishVariant: "EN-GB",
      portugueseVariant: "PT-PT",
      explainModel: "claude-sonnet-5-5",
      captureInEditors: false,
      autoTranslateSelection: true,
    });
  });

  it("turns the two capture options on and off", async () => {
    updateSettings.mockImplementation(async (preferences) => ({ ...BASE, ...preferences }));
    const user = userEvent.setup();
    await renderSettled();

    await user.click(within(general()).getByLabelText(/Capturar a seleção em editores/));
    await user.click(within(general()).getByLabelText(/Traduzir a seleção assim que/));

    expect(updateSettings).toHaveBeenLastCalledWith(
      expect.objectContaining({ captureInEditors: true, autoTranslateSelection: false }),
    );
  });

  it("warns that editors run their integrated terminal on Ctrl+C", async () => {
    await renderSettled();

    expect(within(general()).getByText(/terminal embutido desses editores/)).toBeInTheDocument();
  });

  it("starts with Windows through the dedicated command", async () => {
    const user = userEvent.setup();
    await renderSettled();

    await user.click(within(general()).getByLabelText("Iniciar com o Windows"));

    expect(setAutostart).toHaveBeenCalledExactlyOnceWith(true);
    await waitFor(() =>
      expect(within(general()).getByLabelText("Iniciar com o Windows")).toBeChecked(),
    );
    expect(updateSettings).not.toHaveBeenCalled();
  });

  it("keeps the previous value and shows the error when saving fails", async () => {
    updateSettings.mockRejectedValue(BACKEND_ERROR);
    const user = userEvent.setup();
    await renderSettled();

    await user.selectOptions(within(general()).getByLabelText("Tema"), "dark");

    expect(await within(general()).findByRole("alert")).toHaveTextContent(BACKEND_ERROR.message);
    expect(within(general()).getByLabelText("Tema")).toHaveValue("system");
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });

  it("keeps the autostart switch as it was when it fails", async () => {
    setAutostart.mockRejectedValue(BACKEND_ERROR);
    const user = userEvent.setup();
    await renderSettled();

    await user.click(within(general()).getByLabelText("Iniciar com o Windows"));

    expect(await within(general()).findByRole("alert")).toHaveTextContent(BACKEND_ERROR.message);
    expect(within(general()).getByLabelText("Iniciar com o Windows")).not.toBeChecked();
  });

  it("ignores a second change made while the first is still being saved", async () => {
    const first = deferred<SettingsView>();
    updateSettings.mockReturnValueOnce(first.promise);
    const user = userEvent.setup();
    await renderSettled();

    await user.selectOptions(within(general()).getByLabelText("Tema"), "dark");
    await user.selectOptions(within(general()).getByLabelText("Inglês"), "EN-GB");
    first.resolve({ ...BASE, theme: "dark" });

    await waitFor(() => expect(document.documentElement.dataset.theme).toBe("dark"));
    expect(updateSettings).toHaveBeenCalledExactlyOnceWith(
      expect.objectContaining({ theme: "dark", englishVariant: "EN-US" }),
    );
    expect(within(general()).getByLabelText("Inglês")).toHaveValue("EN-US");
  });

  it("does not undo a shortcut change when a preference answer arrives later", async () => {
    const slow = deferred<SettingsView>();
    updateSettings.mockReturnValueOnce(slow.promise);
    setShortcut.mockResolvedValue({ ...BASE, shortcut: "Ctrl+Alt+K" });
    const user = userEvent.setup();
    await renderSettled();

    await user.selectOptions(within(general()).getByLabelText("Tema"), "dark");
    await user.click(within(shortcut()).getByRole("button", { name: "Alterar atalho" }));
    fireEvent.keyDown(window, { code: "KeyK", ctrlKey: true, altKey: true });
    await screen.findByText("Atalho alterado para Ctrl+Alt+K.");
    slow.resolve({ ...BASE, theme: "dark" });

    await waitFor(() => expect(document.documentElement.dataset.theme).toBe("dark"));
    expect(within(shortcut()).getByText("Ctrl+Alt+K", { selector: "kbd" })).toBeInTheDocument();
  });

  it("shows the load error and hides the shortcut and general sections", async () => {
    getSettings.mockRejectedValue(BACKEND_ERROR);

    render(<SettingsApp />);

    expect(await screen.findByText(BACKEND_ERROR.message)).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Geral" })).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Atalho global" })).not.toBeInTheDocument();
    expect(deepl()).toBeInTheDocument();
  });
});

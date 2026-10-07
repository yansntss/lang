import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as tauri from "../../lib/tauri";
import SettingsApp from "./SettingsApp";

vi.mock("../../lib/tauri", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../lib/tauri")>()),
  hasSecret: vi.fn(),
  setSecret: vi.fn(),
  deleteSecret: vi.fn(),
}));

const hasSecret = vi.mocked(tauri.hasSecret);
const setSecret = vi.mocked(tauri.setSecret);
const deleteSecret = vi.mocked(tauri.deleteSecret);

const NOT_CONFIGURED = "Chave não configurada";
const CONFIGURED = "Chave configurada ✓";

const keyField = () => screen.getByLabelText("Chave da API do DeepL");
const saveButton = () => screen.getByRole("button", { name: "Salvar chave" });

beforeEach(() => {
  hasSecret.mockReset().mockResolvedValue(false);
  setSecret.mockReset().mockResolvedValue(undefined);
  deleteSecret.mockReset().mockResolvedValue(undefined);
});

describe("SettingsApp", () => {
  it("renders the settings heading", async () => {
    render(<SettingsApp />);

    expect(screen.getByRole("heading", { name: "Configurações" })).toBeInTheDocument();
    await screen.findByText(NOT_CONFIGURED);
  });

  it("asks the backend whether the DeepL key is stored", async () => {
    render(<SettingsApp />);

    await screen.findByText(NOT_CONFIGURED);

    expect(hasSecret).toHaveBeenCalledExactlyOnceWith("deepl");
  });

  it("shows that the key is already configured", async () => {
    hasSecret.mockResolvedValue(true);

    render(<SettingsApp />);

    expect(await screen.findByText(CONFIGURED)).toBeInTheDocument();
  });

  it("uses a masked field that does not autocomplete", async () => {
    render(<SettingsApp />);
    await screen.findByText(NOT_CONFIGURED);

    expect(keyField()).toHaveAttribute("type", "password");
    expect(keyField()).toHaveAttribute("autocomplete", "off");
  });

  it("keeps the save button disabled until a key is typed", async () => {
    const user = userEvent.setup();
    render(<SettingsApp />);
    await screen.findByText(NOT_CONFIGURED);

    expect(saveButton()).toBeDisabled();
    await user.type(keyField(), "   ");
    expect(saveButton()).toBeDisabled();
    await user.type(keyField(), "abc:fx");
    expect(saveButton()).toBeEnabled();
  });

  it("saves the key, clears the field and shows the configured state", async () => {
    const user = userEvent.setup();
    render(<SettingsApp />);
    await screen.findByText(NOT_CONFIGURED);

    await user.type(keyField(), "abc:fx");
    await user.click(saveButton());

    expect(await screen.findByText(CONFIGURED)).toBeInTheDocument();
    expect(setSecret).toHaveBeenCalledExactlyOnceWith("deepl", "abc:fx");
    expect(keyField()).toHaveValue("");
    expect(screen.getByRole("status")).toHaveTextContent("Chave salva.");
  });

  it("shows the backend error when saving fails and stays unconfigured", async () => {
    setSecret.mockRejectedValue({
      code: "secret_store",
      message: "Falha ao acessar o armazenamento seguro de chaves.",
    });
    const user = userEvent.setup();
    render(<SettingsApp />);
    await screen.findByText(NOT_CONFIGURED);

    await user.type(keyField(), "abc:fx");
    await user.click(saveButton());

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("armazenamento seguro");
    expect(alert).toHaveTextContent("Digite a chave novamente.");
    expect(screen.getByText(NOT_CONFIGURED)).toBeInTheDocument();
  });

  it("removes the stored key", async () => {
    hasSecret.mockResolvedValue(true);
    const user = userEvent.setup();
    render(<SettingsApp />);
    await screen.findByText(CONFIGURED);

    await user.click(screen.getByRole("button", { name: "Remover chave" }));

    expect(await screen.findByText(NOT_CONFIGURED)).toBeInTheDocument();
    expect(deleteSecret).toHaveBeenCalledExactlyOnceWith("deepl");
  });

  it("does not offer removal when no key is stored", async () => {
    render(<SettingsApp />);
    await screen.findByText(NOT_CONFIGURED);

    expect(screen.queryByRole("button", { name: "Remover chave" })).not.toBeInTheDocument();
  });

  it("reports a failure to read the key status", async () => {
    hasSecret.mockRejectedValue({ code: "secret_store", message: "Falha ao acessar o cofre." });

    render(<SettingsApp />);

    expect(await screen.findByRole("alert")).toHaveTextContent("Falha ao acessar o cofre.");
  });
});

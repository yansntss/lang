import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as tauri from "../../lib/tauri";
import type { AvailableUpdate, UpdateEvent } from "../../lib/types";
import UpdateSection from "./UpdateSection";

vi.mock("../../lib/tauri", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../lib/tauri")>()),
  checkUpdate: vi.fn(),
  installUpdate: vi.fn(),
}));

const checkUpdate = vi.mocked(tauri.checkUpdate);
const installUpdate = vi.mocked(tauri.installUpdate);

const UPDATE: AvailableUpdate = {
  version: "0.2.0",
  notes: "Correções e melhorias.",
  publishedAt: Date.UTC(2026, 9, 8, 12),
};
const BACKEND_ERROR = { code: "update", message: "Não foi possível concluir a atualização." };
const MEGABYTE = 1024 * 1024;

const checkButton = () => screen.getByRole("button", { name: "Verificar atualizações" });
const installButton = () => screen.getByRole("button", { name: "Instalar e reiniciar" });

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

beforeEach(() => {
  checkUpdate.mockReset();
  installUpdate.mockReset();
});

describe("UpdateSection", () => {
  it("shows the running version and does not look for updates by itself", () => {
    render(<UpdateSection version="0.1.0" />);

    expect(screen.getByText("Versão atual: 0.1.0")).toBeInTheDocument();
    expect(checkUpdate).not.toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: "Instalar e reiniciar" })).not.toBeInTheDocument();
  });

  it("says so when the app is already up to date", async () => {
    checkUpdate.mockResolvedValue(null);
    const user = userEvent.setup();
    render(<UpdateSection version="0.1.0" />);

    await user.click(checkButton());

    expect(await screen.findByText("Você já está na versão mais recente.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Instalar e reiniciar" })).not.toBeInTheDocument();
  });

  it("shows the new version, its date and notes, and offers to install", async () => {
    checkUpdate.mockResolvedValue(UPDATE);
    const user = userEvent.setup();
    render(<UpdateSection version="0.1.0" />);

    await user.click(checkButton());

    expect(await screen.findByText(/Versão 0\.2\.0 disponível/)).toHaveTextContent("2026");
    expect(screen.getByText("Correções e melhorias.")).toBeInTheDocument();
    expect(installButton()).toBeEnabled();
  });

  it("shows the release notes as plain text, never as HTML", async () => {
    checkUpdate.mockResolvedValue({
      ...UPDATE,
      notes: "<img src=x onerror=alert(1)> **negrito** [link](http://x)",
    });
    const user = userEvent.setup();
    render(<UpdateSection version="0.1.0" />);

    await user.click(checkButton());

    expect(
      await screen.findByText("<img src=x onerror=alert(1)> **negrito** [link](http://x)"),
    ).toBeInTheDocument();
    expect(document.querySelector("img, a, strong")).toBeNull();
  });

  it("works without notes or a publish date", async () => {
    checkUpdate.mockResolvedValue({ version: "0.2.0", notes: null, publishedAt: null });
    const user = userEvent.setup();
    render(<UpdateSection version="0.1.0" />);

    await user.click(checkButton());

    expect(await screen.findByText("Versão 0.2.0 disponível.")).toBeInTheDocument();
  });

  it("shows why the check failed and lets the user try again", async () => {
    checkUpdate.mockRejectedValueOnce({
      code: "network",
      message: "Não foi possível conectar ao serviço. Verifique sua conexão.",
    });
    checkUpdate.mockResolvedValueOnce(null);
    const user = userEvent.setup();
    render(<UpdateSection version="0.1.0" />);

    await user.click(checkButton());
    expect(await screen.findByRole("alert")).toHaveTextContent("Verifique sua conexão");
    expect(screen.queryByRole("button", { name: "Instalar e reiniciar" })).not.toBeInTheDocument();
    await user.click(checkButton());

    expect(await screen.findByText("Você já está na versão mais recente.")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("does not check twice when the button is clicked in quick succession", async () => {
    const pending = deferred<AvailableUpdate | null>();
    checkUpdate.mockReturnValue(pending.promise);
    render(<UpdateSection version="0.1.0" />);

    fireEvent.click(checkButton());
    fireEvent.click(checkButton());
    pending.resolve(null);

    await screen.findByText("Você já está na versão mais recente.");
    expect(checkUpdate).toHaveBeenCalledOnce();
  });

  it("disables the check button while checking", async () => {
    const pending = deferred<AvailableUpdate | null>();
    checkUpdate.mockReturnValue(pending.promise);
    const user = userEvent.setup();
    render(<UpdateSection version="0.1.0" />);

    await user.click(checkButton());

    expect(screen.getByText("Verificando…")).toBeInTheDocument();
    expect(checkButton()).toBeDisabled();
    await act(async () => pending.resolve(null));
  });
});

describe("UpdateSection — instalação", () => {
  async function renderWithUpdate() {
    checkUpdate.mockResolvedValue(UPDATE);
    const user = userEvent.setup();
    render(<UpdateSection version="0.1.0" />);
    await user.click(checkButton());
    await screen.findByText(/Versão 0\.2\.0 disponível/);
    return user;
  }

  it("shows the download progress as the events arrive", async () => {
    let emit: (event: UpdateEvent) => void = () => {};
    const finish = deferred<void>();
    installUpdate.mockImplementation((onEvent) => {
      emit = onEvent;
      return finish.promise;
    });
    const user = await renderWithUpdate();

    await user.click(installButton());
    act(() => emit({ kind: "progress", downloaded: MEGABYTE, total: 4 * MEGABYTE }));

    expect(await screen.findByText(/Baixando… 1\.0 de 4\.0 MB \(25%\)/)).toBeInTheDocument();
    expect(screen.getByRole("progressbar", { name: "Progresso do download" })).toHaveAttribute(
      "value",
      "25",
    );
    expect(screen.queryByRole("button", { name: "Instalar e reiniciar" })).not.toBeInTheDocument();
    expect(checkButton()).toBeDisabled();
    await act(async () => finish.resolve());
  });

  it("shows the progress without a percentage when the size is unknown", async () => {
    let emit: (event: UpdateEvent) => void = () => {};
    const finish = deferred<void>();
    installUpdate.mockImplementation((onEvent) => {
      emit = onEvent;
      return finish.promise;
    });
    const user = await renderWithUpdate();

    await user.click(installButton());
    act(() => emit({ kind: "progress", downloaded: 2 * MEGABYTE, total: null }));

    const text = await screen.findByText(/Baixando… 2\.0 MB/);
    expect(text).not.toHaveTextContent("%");
    await act(async () => finish.resolve());
  });

  it("tells the user the app is about to close when the installer takes over", async () => {
    let emit: (event: UpdateEvent) => void = () => {};
    const finish = deferred<void>();
    installUpdate.mockImplementation((onEvent) => {
      emit = onEvent;
      return finish.promise;
    });
    const user = await renderWithUpdate();

    await user.click(installButton());
    act(() => emit({ kind: "installing" }));

    expect(await screen.findByText(/vai fechar e abrir de novo sozinho/)).toBeInTheDocument();
    await act(async () => finish.resolve());
  });

  it("keeps the install offer and shows the error when the installation fails", async () => {
    installUpdate.mockRejectedValue(BACKEND_ERROR);
    const user = await renderWithUpdate();

    await user.click(installButton());

    expect(await screen.findByRole("alert")).toHaveTextContent(BACKEND_ERROR.message);
    expect(installButton()).toBeEnabled();
    expect(screen.getByText(/Versão 0\.2\.0 disponível/)).toBeInTheDocument();
  });

  it("can try again after a failed installation", async () => {
    installUpdate.mockRejectedValueOnce(BACKEND_ERROR);
    installUpdate.mockResolvedValueOnce(undefined);
    const user = await renderWithUpdate();

    await user.click(installButton());
    await screen.findByRole("alert");
    await user.click(installButton());

    await waitFor(() => expect(installUpdate).toHaveBeenCalledTimes(2));
    expect(await screen.findByText(/vai fechar e abrir de novo sozinho/)).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("does not start two installations when the button is clicked in quick succession", async () => {
    const finish = deferred<void>();
    installUpdate.mockReturnValue(finish.promise);
    await renderWithUpdate();
    const button = installButton();

    fireEvent.click(button);
    fireEvent.click(button);

    expect(installUpdate).toHaveBeenCalledOnce();
    await act(async () => finish.resolve());
  });
});

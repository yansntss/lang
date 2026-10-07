import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import SettingsApp from "./SettingsApp";

describe("SettingsApp", () => {
  it("renders the settings heading", () => {
    render(<SettingsApp />);

    expect(screen.getByRole("heading", { name: "Configurações" })).toBeInTheDocument();
  });
});

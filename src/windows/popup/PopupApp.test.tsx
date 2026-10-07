import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import PopupApp from "./PopupApp";

describe("PopupApp", () => {
  it("renders the translator heading", () => {
    render(<PopupApp />);

    expect(screen.getByRole("heading", { name: "Tradutor" })).toBeInTheDocument();
  });
});

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import PopupApp from "./PopupApp";

const root = document.getElementById("root");
if (!root) {
  throw new Error("Elemento #root não encontrado em popup.html");
}

createRoot(root).render(
  <StrictMode>
    <PopupApp />
  </StrictMode>,
);

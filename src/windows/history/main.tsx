import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import HistoryApp from "./HistoryApp";

const root = document.getElementById("root");
if (!root) {
  throw new Error("Elemento #root não encontrado em history.html");
}

createRoot(root).render(
  <StrictMode>
    <HistoryApp />
  </StrictMode>,
);

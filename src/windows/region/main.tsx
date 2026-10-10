import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import RegionApp from "./RegionApp";

const root = document.getElementById("root");
if (!root) {
  throw new Error("Elemento #root não encontrado em region.html");
}

createRoot(root).render(
  <StrictMode>
    <RegionApp />
  </StrictMode>,
);

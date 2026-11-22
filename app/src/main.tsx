import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

// Block the browser context menu & accidental reloads so it feels like a native app.
if (!import.meta.env.DEV) {
  document.addEventListener("contextmenu", (e) => {
    const t = e.target as HTMLElement;
    if (!t.closest("input, textarea")) e.preventDefault();
  });
  document.addEventListener("keydown", (e) => {
    if (e.key === "F5" || ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "r")) e.preventDefault();
  });
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

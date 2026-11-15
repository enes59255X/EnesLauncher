import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

// Sağ Tık Menüsünü (İncele, Yazdır, Farklı Kaydet vb.) Kökten Engelle
window.addEventListener("contextmenu", (e) => {
  e.preventDefault();
  return false;
});

// Geliştirici Kısayollarını (F12, Ctrl+Shift+I, Ctrl+U, Ctrl+P) Engelle
window.addEventListener("keydown", (e) => {
  if (
    e.key === "F12" ||
    (e.ctrlKey && e.shiftKey && (e.key === "I" || e.key === "i" || e.key === "J" || e.key === "j")) ||
    (e.ctrlKey && (e.key === "u" || e.key === "U" || e.key === "p" || e.key === "P"))
  ) {
    e.preventDefault();
    return false;
  }
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);


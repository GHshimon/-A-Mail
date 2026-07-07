import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles/global.css";

// macOS 実機ではフォールバックの疑似 traffic light を隠すため、プラットフォームを
// data 属性に記録する(Overlay の本物の信号機を使うため)。
if (
  typeof navigator !== "undefined" &&
  /Mac|iPhone|iPad/.test(navigator.platform)
) {
  document.documentElement.setAttribute("data-platform", "macos");
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

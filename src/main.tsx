import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import "./styles.css";

async function bootstrap() {
  if (import.meta.env.VITE_XAC_MOCK_IPC === "1") {
    await import("./testSupport/mockIpc");
  }

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>
  );
}

void bootstrap();

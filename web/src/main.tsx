import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { loadEngine } from "./engine";
import "./styles.css";

const root = createRoot(document.getElementById("root")!);

loadEngine()
  .then(() =>
    root.render(
      <StrictMode>
        <App />
      </StrictMode>,
    ),
  )
  .catch((e) => {
    root.render(<p className="fatal">Could not load the calculation engine: {String(e)}</p>);
  });

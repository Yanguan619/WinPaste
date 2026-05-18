import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import CompactPreviewWindow from "./features/clipboard/components/CompactPreviewWindow";
import StickyWindow from "./features/sticky/components/StickyWindow";
import "./index.css";
import "./styles/components/index.css";
import "./styles/themes/load";

const params = new URLSearchParams(window.location.search);
const isCompactPreview = params.get("window") === "compact-preview";
const isSticky = params.get("window") === "sticky";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {isSticky ? <StickyWindow /> : isCompactPreview ? <CompactPreviewWindow /> : <App />}
  </React.StrictMode>,
);

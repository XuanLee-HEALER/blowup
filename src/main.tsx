import { HashRouter } from "react-router-dom";
import App from "./App";
import { mountReactRoot } from "./lib/mountReactRoot";

mountReactRoot(
  <HashRouter>
    <App />
  </HashRouter>
);

// Fade out splash screen once React has rendered
requestAnimationFrame(() => {
  const splash = document.getElementById("splash");
  if (splash) {
    splash.classList.add("fade");
    setTimeout(() => splash.remove(), 350);
  }
});

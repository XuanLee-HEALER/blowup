import { HashRouter } from "react-router-dom";
import App from "./App";
import { mountReactRoot } from "./lib/mountReactRoot";

mountReactRoot(
  <HashRouter>
    <App />
  </HashRouter>
);

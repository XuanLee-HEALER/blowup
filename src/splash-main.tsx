import React from "react";
import ReactDOM from "react-dom/client";
import SplashRoot from "./splash/SplashRoot";

const root = document.getElementById("root");
if (!root) throw new Error("missing #root element");

ReactDOM.createRoot(root).render(
  <React.StrictMode>
    <SplashRoot />
  </React.StrictMode>
);

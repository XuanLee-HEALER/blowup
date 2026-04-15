import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import Splash from "./Splash";

export default function SplashRoot() {
  const [fading, setFading] = useState(false);

  return (
    <Splash
      className={fading ? "fade" : ""}
      onComplete={() => {
        setFading(true);
        setTimeout(() => {
          invoke("close_splashscreen").catch((e) => {
            console.error("close_splashscreen failed:", e);
          });
        }, 350);
      }}
    />
  );
}

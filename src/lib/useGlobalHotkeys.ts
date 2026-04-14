import { useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { SPACES } from "./space";

/**
 * Global keyboard shortcuts registered once at the AppLayout level.
 *
 *   Cmd+1 / Cmd+2 / Cmd+3   switch space (library / discover / knowledge)
 *   Cmd+,                   open settings overlay
 *   Esc                     close context panel (handled by ContextPanel itself
 *                           via document listener — kept here as a no-op note)
 *
 * Cmd+F is space-local (each space's toolbar search input registers its own
 * focus), so it is NOT registered globally.
 */
export function useGlobalHotkeys() {
  const navigate = useNavigate();

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // Only intercept Cmd/Ctrl combinations.
      const meta = e.metaKey || e.ctrlKey;
      if (!meta) return;

      // Cmd+1/2/3
      if (e.key >= "1" && e.key <= "3") {
        const idx = parseInt(e.key, 10) - 1;
        const target = SPACES[idx];
        if (target) {
          e.preventDefault();
          navigate(target.route);
        }
        return;
      }

      // Cmd+,
      if (e.key === ",") {
        e.preventDefault();
        navigate("/settings");
        return;
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [navigate]);
}

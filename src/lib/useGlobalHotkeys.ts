import { useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { SPACES } from "./space";

/** True when the focused element is a text-editing surface where
 *  Cmd+1 / Cmd+, etc. would otherwise stomp on user input. */
function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target instanceof HTMLInputElement) return target.type !== "checkbox" && target.type !== "radio";
  if (target instanceof HTMLTextAreaElement) return true;
  if (target instanceof HTMLSelectElement) return true;
  return target.isContentEditable;
}

/**
 * Global keyboard shortcuts:
 *
 *   Cmd+1 / Cmd+2 / Cmd+3   switch space (library / discover / knowledge)
 *   Cmd+,                   open settings overlay
 *
 * Suppressed while an editable element has focus so the user's
 * typing context is never destroyed by a stray modifier collision.
 */
export function useGlobalHotkeys() {
  const navigate = useNavigate();

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey)) return;
      if (isEditableTarget(e.target)) return;

      if (e.key >= "1" && e.key <= "3") {
        const target = SPACES[parseInt(e.key, 10) - 1];
        if (target) {
          e.preventDefault();
          navigate(target.route);
        }
        return;
      }

      if (e.key === ",") {
        e.preventDefault();
        navigate("/settings");
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [navigate]);
}

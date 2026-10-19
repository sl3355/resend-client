import { useEffect, type RefObject } from "react";
export function useDialogFocus(ref: RefObject<HTMLElement | null>) {
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const focusable = () =>
      Array.from(
        ref.current?.querySelectorAll<HTMLElement>(
          'button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled),[tabindex="0"],[contenteditable="true"]',
        ) || [],
      ).filter((el) => el.getClientRects().length > 0);
    const timer = requestAnimationFrame(() => {
      const auto = ref.current?.querySelector<HTMLElement>("[autofocus]");
      (auto || focusable()[0])?.focus();
    });
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Tab") return;
      const elements = focusable();
      if (!elements.length) {
        e.preventDefault();
        return;
      }
      const first = elements[0],
        last = elements[elements.length - 1];
      if (
        e.shiftKey &&
        (document.activeElement === first ||
          !ref.current?.contains(document.activeElement))
      ) {
        e.preventDefault();
        last.focus();
      } else if (
        !e.shiftKey &&
        (document.activeElement === last ||
          !ref.current?.contains(document.activeElement))
      ) {
        e.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => {
      cancelAnimationFrame(timer);
      document.removeEventListener("keydown", onKey);
      previous?.focus();
    };
  }, [ref]);
}

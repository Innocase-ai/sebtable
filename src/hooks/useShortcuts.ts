import { useEffect } from "react";
import { useUiStore } from "../stores/uiStore";

export function useShortcuts() {
  const openModal = useUiStore((s) => s.openModal);
  const toggleAi = useUiStore((s) => s.toggleAi);
  const toggleSearch = useUiStore((s) => s.toggleSearch);
  const modal = useUiStore((s) => s.modal);

  useEffect(() => {
    const isTypingTarget = (el: HTMLElement | null) =>
      !!el && ["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName);
    const h = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        toggleSearch();
        // focus sera géré par la palette elle-même
        setTimeout(() => document.querySelector<HTMLInputElement>('[data-shortcut="global-search"]')?.focus(), 50);
      }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "n") {
        if (!modal && !isTypingTarget(target)) {
          e.preventDefault();
          openModal("createTable");
        }
      }
      if ((e.ctrlKey || e.metaKey) && e.key === ",") {
        e.preventDefault();
        openModal("settings");
      }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "i") {
        if (e.shiftKey) {
          e.preventDefault();
          toggleAi();
        }
      }
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [openModal, toggleAi, toggleSearch, modal]);
}

// What every window over the app does with the keyboard: it says it is a
// dialog, takes the focus when it opens (a field marked `autofocus`, else
// the first field, else the first button that is not the close one), keeps
// Tab inside it and gives the focus back to where it was when it closes.

const FOCUSABLE =
  'button:not([disabled]), [href], input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

function visible(el: HTMLElement) {
  return el.offsetParent !== null || el.getClientRects().length > 0;
}

export function dialog(node: HTMLElement) {
  const before = document.activeElement as HTMLElement | null;
  node.setAttribute("role", node.getAttribute("role") ?? "dialog");
  node.setAttribute("aria-modal", "true");
  if (!node.hasAttribute("tabindex")) node.tabIndex = -1;
  const heading = node.querySelector("h2");
  if (heading && !node.hasAttribute("aria-labelledby")) {
    heading.id ||= `dlg-${Math.random().toString(36).slice(2, 8)}`;
    node.setAttribute("aria-labelledby", heading.id);
  }

  const items = () => [...node.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(visible);
  // After the window's content is drawn.
  const timer = setTimeout(() => {
    if (node.contains(document.activeElement)) return;
    const all = items();
    const first =
      node.querySelector<HTMLElement>("[autofocus], [data-default]") ??
      all.find((e) => e.matches("input:not([type=checkbox]), select, textarea")) ??
      all.find((e) => !e.classList.contains("close")) ??
      node;
    first.focus({ preventScroll: true });
  }, 0);

  function onkeydown(e: KeyboardEvent) {
    if (e.key !== "Tab") return;
    const all = items();
    if (!all.length) return;
    const first = all[0];
    const last = all[all.length - 1];
    const at = document.activeElement;
    if (e.shiftKey && (at === first || at === node)) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && at === last) {
      e.preventDefault();
      first.focus();
    }
  }
  node.addEventListener("keydown", onkeydown);

  return {
    destroy() {
      clearTimeout(timer);
      node.removeEventListener("keydown", onkeydown);
      if (before && document.contains(before)) before.focus({ preventScroll: true });
    },
  };
}

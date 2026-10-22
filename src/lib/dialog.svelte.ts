// In-app dialogs (instead of the OS ones), so the title shows once and
// the safe button can be focused by default.

export interface DialogOptions {
  title: string;
  text?: string;
  /** Detail lines, e.g. errors per file. */
  items?: string[];
  ok?: string;
  /** Omit for a plain notice with a single button. */
  cancel?: string;
  /** Styles the confirm button as destructive. */
  danger?: boolean;
}

interface Pending extends DialogOptions {
  resolve: (ok: boolean) => void;
}

export const dialog = $state<{ current: Pending | null }>({ current: null });

export function confirm(options: DialogOptions): Promise<boolean> {
  dialog.current?.resolve(false);
  return new Promise((resolve) => (dialog.current = { ...options, resolve }));
}

export async function notify(options: Omit<DialogOptions, "cancel">) {
  await confirm(options);
}

export function closeDialog(ok: boolean) {
  const current = dialog.current;
  dialog.current = null;
  current?.resolve(ok);
}

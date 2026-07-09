/**
 * Production-safe diagnostics.
 * Never spam the user-facing console in release builds; dev can opt in via localStorage.
 */

const ENABLED =
  import.meta.env.DEV ||
  (typeof localStorage !== "undefined" && localStorage.getItem("grok-desktop-debug") === "1");

export function debugLog(...args: unknown[]): void {
  if (ENABLED) {
    // eslint-disable-next-line no-console
    console.debug("[grok-desktop]", ...args);
  }
}

export function debugWarn(...args: unknown[]): void {
  if (ENABLED) {
    // eslint-disable-next-line no-console
    console.warn("[grok-desktop]", ...args);
  }
}

/** Map backend/IPC errors into short, actionable copy for toasts. */
export function humanizeError(err: unknown): string {
  const raw = String(err ?? "Unknown error")
    .replace(/^Error:\s*/i, "")
    .replace(/^"[^"]*":\s*/i, "")
    .trim();

  if (/could not find `?grok`?/i.test(raw) || /not found on PATH/i.test(raw)) {
    return "Grok CLI not found. Install Grok Build, run grok login, or set the binary path in Settings.";
  }
  if (
    /Configured grok binary not found/i.test(raw) ||
    /must point to an executable named/i.test(raw)
  ) {
    return "Invalid Grok binary path. Point Settings at grok.exe (or leave blank to use PATH).";
  }
  if (/Working directory does not exist/i.test(raw)) {
    return "Project folder is missing. Open a valid project from the sidebar.";
  }
  if (/Image path must be under/i.test(raw)) {
    return "That image path is not allowed. Attach images with the + button or paste a screenshot.";
  }
  if (/exceeds 20 MB/i.test(raw) || /payload too large/i.test(raw)) {
    return "Image is too large (max 20 MB). Try a smaller screenshot or compress it.";
  }
  if (/Unsupported image/i.test(raw) || /does not look like/i.test(raw)) {
    return "Unsupported image type. Use PNG, JPEG, GIF, WebP, or BMP.";
  }
  if (/already running/i.test(raw)) {
    return "Grok is still working. Wait for it to finish or press Stop.";
  }
  if (/Empty message/i.test(raw)) {
    return "Type a message or attach an image before sending.";
  }
  if (/Chat history limit/i.test(raw)) {
    return "This chat is very long. Start a new chat to continue.";
  }
  if (/Cancelled/i.test(raw)) {
    return "Stopped.";
  }
  if (/exited with code/i.test(raw)) {
    return "Grok finished with an error. Turn on Verbose for details, or try again.";
  }
  if (/No active session/i.test(raw)) {
    return "Session not ready. Pick a project folder or restart the app.";
  }
  // Cap length for toast UI.
  return raw.length > 220 ? `${raw.slice(0, 217)}…` : raw || "Something went wrong.";
}

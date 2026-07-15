/**
 * Chat session store — messages, streaming buffer, session lifecycle.
 * Agent Transparency Mode: default Hidden (black box); Verbose streams raw output.
 */
import { writable, get } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { persistSettings, settings, type AppSettings } from "$lib/stores/settings";
import { debugWarn, humanizeError } from "$lib/log";
import { cleanTerminalText } from "$lib/text";

export interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
  images: string[];
  timestamp: string;
  status?: string | null;
  streaming?: boolean;
  /** Full captured CLI/agent stream for this turn (always stored). */
  rawContent?: string;
  /** Per-message reveal of raw agent details (independent of global verbose). */
  rawVisible?: boolean;
  /** High-level status label while streaming in Hidden mode. */
  phaseLabel?: string;
}

export interface ChatSession {
  id: string;
  project_id: string | null;
  title: string;
  created_at: string;
  updated_at: string;
  messages: ChatMessage[];
}

export interface GrokStatus {
  state: string;
  detail: string;
  yolo: boolean;
  model: string;
  running: boolean;
}

export interface PendingAttachment {
  id: string;
  path: string;
  filename: string;
  mime: string;
  kind: "image" | "video" | "audio" | "file";
  sizeBytes: number;
  previewUrl?: string;
}

export interface PermissionOption {
  id: string;
  name: string;
  kind: "allow_once" | "allow_always" | "reject_once" | "reject_always" | string;
}

export interface PermissionRequest {
  request_id: string;
  title: string;
  tool_call: Record<string, unknown>;
  options: PermissionOption[];
}

export interface GrokQuestionOption {
  label: string;
  description: string;
  preview?: string | null;
  id?: string | null;
}

export interface GrokQuestion {
  question: string;
  options: GrokQuestionOption[];
  multiSelect?: boolean | null;
  id?: string | null;
}

export interface InteractionRequest {
  request_id: string;
  kind: "question" | "plan_approval";
  session_id: string;
  tool_call_id: string;
  mode?: "default" | "plan" | null;
  questions: GrokQuestion[];
  plan_content?: string | null;
}

export const currentChat = writable<ChatSession | null>(null);
export const chatList = writable<ChatSession[]>([]);
export const pendingAttachments = writable<PendingAttachment[]>([]);
export const pendingPermission = writable<PermissionRequest | null>(null);
export const pendingInteraction = writable<InteractionRequest | null>(null);
export const isRunning = writable(false);
export const runningSince = writable<number | null>(null);
export const status = writable<GrokStatus>({
  state: "idle",
  detail: "Idle",
  yolo: false,
  model: "",
  running: false,
});
/** Full raw stream for the in-flight turn (stdout + annotated stderr). */
export const streamBuffer = writable("");
/**
 * Agent Transparency Mode.
 * false (default) = Hidden black box — high-level status only while running.
 * true = Verbose — stream raw agent/CLI output into the assistant bubble.
 */
export const verboseMode = writable(false);
export const errorToast = writable<string | null>(null);
export const selectedModel = writable("grok-4.5");
export const reasoningEffort = writable<"low" | "medium" | "high">("high");
export const yoloEnabled = writable(false);

let unlisteners: UnlistenFn[] = [];

/** Soft cap on in-memory raw stream to avoid unbounded RAM growth on long agent runs. */
const MAX_STREAM_CHARS = 1_500_000;
/** Throttle verbose UI rewrites (ms) — still capture every line in streamBuffer. */
const UI_FLUSH_MS = 80;
let uiFlushTimer: ReturnType<typeof setTimeout> | null = null;
let uiFlushPending = false;
let activeSessionKey: string | null = null;

async function invokeWithRetry<T>(command: string, args: Record<string, unknown>): Promise<T> {
  let lastError: unknown;
  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      return await invoke<T>(command, args);
    } catch (error) {
      lastError = error;
      if (attempt < 2) await new Promise((resolve) => setTimeout(resolve, 75 * (attempt + 1)));
    }
  }
  throw lastError;
}

function uid(): string {
  return crypto.randomUUID?.() ?? `m_${Date.now()}_${Math.random().toString(36).slice(2)}`;
}

function appendStream(line: string): string {
  const cleanLine = cleanTerminalText(line);
  streamBuffer.update((b) => {
    const next = b ? `${b}\n${cleanLine}` : cleanLine;
    if (next.length <= MAX_STREAM_CHARS) return next;
    // Keep the tail (most recent output).
    return next.slice(next.length - MAX_STREAM_CHARS);
  });
  return get(streamBuffer);
}

function appendStreamChunk(chunk: string): string {
  const cleanChunk = cleanTerminalText(chunk);
  streamBuffer.update((buffer) => {
    const next = `${buffer}${cleanChunk}`;
    return next.length <= MAX_STREAM_CHARS ? next : next.slice(next.length - MAX_STREAM_CHARS);
  });
  return get(streamBuffer);
}

function streamingPreview(raw: string): string {
  const preview = extractFinalReply(raw);
  if (!preview) return "";
  if (/^(thinking…?|session ready(?: \(continuing\))?|ready|done)$/i.test(preview.trim())) {
    return "";
  }
  return preview;
}

/** Apply streamBuffer to the streaming assistant bubble (throttled in verbose mode). */
function flushStreamingUi(force = false): void {
  const run = () => {
    uiFlushPending = false;
    uiFlushTimer = null;
    const verbose = get(verboseMode);
    const buf = get(streamBuffer);
    const preview = verbose ? buf : streamingPreview(buf);
    currentChat.update((chat) => {
      if (!chat) return chat;
      const msgs = [...chat.messages];
      const last = msgs[msgs.length - 1];
      if (last?.role === "assistant" && last.streaming) {
        msgs[msgs.length - 1] = {
          ...last,
          rawContent: buf,
          content: verbose ? buf : preview || last.content || "",
          phaseLabel: preview ? undefined : last.phaseLabel || "Thinking…",
        };
        return { ...chat, messages: msgs };
      }
      return chat;
    });
  };

  if (force) {
    if (uiFlushTimer) {
      clearTimeout(uiFlushTimer);
      uiFlushTimer = null;
    }
    run();
    return;
  }

  if (uiFlushTimer) {
    uiFlushPending = true;
    return;
  }
  run();
  uiFlushTimer = setTimeout(() => {
    uiFlushTimer = null;
    if (uiFlushPending) run();
  }, UI_FLUSH_MS);
}

export function showError(msg: string | unknown) {
  const text = typeof msg === "string" ? humanizeError(msg) : humanizeError(msg);
  errorToast.set(text);
  setTimeout(() => {
    if (get(errorToast) === text) errorToast.set(null);
  }, 6500);
}

/** Persist and apply Verbose / Hidden mode. */
export async function setVerboseMode(on: boolean): Promise<void> {
  verboseMode.set(on);
  const next: AppSettings = { ...get(settings), verbose_mode: on };
  try {
    await persistSettings(next);
  } catch (e) {
    debugWarn("setVerboseMode persist", e);
  }
}

/**
 * Build a user-facing final reply from raw agent output.
 * Strips obvious tool/CLI noise when possible; falls back to full buffer.
 */
export function extractFinalReply(raw: string): string {
  const text = cleanTerminalText(raw).trim();
  if (!text) return "";

  const lines = text.split(/\r?\n/);
  const noise =
    /^(tool\s*call|running tool|invoking|function call|always-approve|permission|stderr:|\[stderr\]|DEBUG|trace)/i;
  const stderrToolError =
    /^\[stderr\].*(?:tool error:\s*tool_output_error|error_kind="?tool_output_error"?|tool_output_error)/i;
  const cleaned = lines.filter((line) => {
    const trimmed = line.trim();
    return !noise.test(trimmed) && !stderrToolError.test(trimmed);
  });
  const body = cleaned.join("\n").trim();
  return body || text;
}

/** Map status detail into a calm phase label for Hidden-mode bubbles. */
function phaseFromStatus(detail: string, running: boolean): string {
  if (!running) return "Ready";
  const d = cleanTerminalText(detail || "").trim();
  if (!d || d === "Idle" || d === "Ready" || d === "Done") return "Thinking…";
  return d;
}

/** Detect user intent to reveal raw agent details for the last assistant turn. */
export function maybeHandleRevealRequest(prompt: string): boolean {
  const p = prompt.trim().toLowerCase();
  const triggers = [
    "show raw",
    "show the raw",
    "raw output",
    "show tool",
    "show the tool",
    "tool calls",
    "show diff",
    "show the diff",
    "show agent details",
    "reveal raw",
    "verbose for this",
    "show details",
  ];
  if (!triggers.some((t) => p.includes(t))) return false;

  let revealed = false;
  currentChat.update((chat) => {
    if (!chat) return chat;
    const msgs = [...chat.messages];
    for (let i = msgs.length - 1; i >= 0; i--) {
      if (msgs[i].role === "assistant" && (msgs[i].rawContent || msgs[i].content)) {
        msgs[i] = { ...msgs[i], rawVisible: true };
        revealed = true;
        break;
      }
    }
    return { ...chat, messages: msgs };
  });

  if (revealed) {
    status.update((s) => ({ ...s, detail: "Agent details revealed for last reply" }));
  } else {
    showError("No agent details available yet for this chat.");
  }
  return revealed;
}

export function toggleMessageRaw(messageId: string): void {
  currentChat.update((chat) => {
    if (!chat) return chat;
    return {
      ...chat,
      messages: chat.messages.map((m) =>
        m.id === messageId ? { ...m, rawVisible: !m.rawVisible } : m,
      ),
    };
  });
}

export async function bindGrokEvents(): Promise<void> {
  for (const u of unlisteners) u();
  unlisteners = [];

  unlisteners.push(
    await listen<string>("grok-stdout", (ev) => {
      appendStream(ev.payload);
      // Throttle UI rewrites in both modes. Hidden mode previews cleaned answer
      // text when it exists; Verbose mode shows the full raw stream.
      flushStreamingUi(false);
    }),
  );

  unlisteners.push(
    await listen<InteractionRequest>("grok-interaction-request", (ev) => {
      pendingInteraction.set(ev.payload);
    }),
  );

  unlisteners.push(
    await listen<string>("grok-interaction-resolved", (ev) => {
      pendingInteraction.update((request) => (request?.request_id === ev.payload ? null : request));
    }),
  );

  unlisteners.push(
    await listen<string>("grok-stdout-chunk", (ev) => {
      appendStreamChunk(ev.payload);
      flushStreamingUi(false);
    }),
  );

  unlisteners.push(
    await listen<string>("grok-stderr", (ev) => {
      const line = cleanTerminalText(ev.payload);
      // Always keep stderr in the raw buffer (prefixed) for later reveal.
      appendStream(`[stderr] ${line}`);

      if (get(verboseMode)) {
        status.update((s) => ({ ...s, detail: line.slice(0, 120) }));
      }
      flushStreamingUi(false);
      // Hidden mode: stderr only affects status via classify_status on backend events.
    }),
  );

  unlisteners.push(
    await listen<GrokStatus>("grok-status", (ev) => {
      status.set(ev.payload);
      isRunning.set(!!ev.payload.running);
      if (ev.payload.running) {
        runningSince.update((value) => value ?? Date.now());
      } else {
        runningSince.set(null);
      }

      const phase = phaseFromStatus(ev.payload.detail, !!ev.payload.running);
      if (!get(verboseMode) && ev.payload.running) {
        currentChat.update((chat) => {
          if (!chat) return chat;
          const msgs = [...chat.messages];
          const last = msgs[msgs.length - 1];
          if (last?.role === "assistant" && last.streaming) {
            msgs[msgs.length - 1] = {
              ...last,
              phaseLabel: phase,
              // Display calm phase as content while streaming in Hidden mode.
              content: last.content || "",
            };
            return { ...chat, messages: msgs };
          }
          return chat;
        });
      }
    }),
  );

  unlisteners.push(
    await listen<PermissionRequest>("grok-permission-request", (ev) => {
      pendingPermission.set(ev.payload);
    }),
  );

  unlisteners.push(
    await listen<string>("grok-permission-resolved", (ev) => {
      pendingPermission.update((request) => (request?.request_id === ev.payload ? null : request));
    }),
  );

  unlisteners.push(
    await listen<{
      exit_code: number;
      cancelled: boolean;
      success: boolean;
      stop_reason?: string | null;
    }>("grok-done", async (ev) => {
      pendingPermission.set(null);
      pendingInteraction.set(null);
      const permissionBlocked = ev.payload.stop_reason === "permission_cancelled";
      const agentCancelled =
        !ev.payload.cancelled && !ev.payload.success && ev.payload.stop_reason === "cancelled";
      isRunning.set(false);
      runningSince.set(null);
      status.update((s) => ({
        ...s,
        state: ev.payload.cancelled ? "cancelled" : ev.payload.success ? "ready" : "error",
        detail: permissionBlocked
          ? "Action blocked by approval mode"
          : ev.payload.cancelled || agentCancelled
            ? "Cancelled"
            : ev.payload.success
              ? "Done"
              : `Exit code ${ev.payload.exit_code}`,
        running: false,
      }));
      flushStreamingUi(true);
      const buffer = get(streamBuffer);
      streamBuffer.set("");
      const verbose = get(verboseMode);

      currentChat.update((chat) => {
        if (!chat) return chat;
        const msgs = [...chat.messages];
        const last = msgs[msgs.length - 1];
        if (last?.role === "assistant" && last.streaming) {
          let finalContent: string;
          if (ev.payload.cancelled) {
            finalContent = "Cancelled.";
          } else if (permissionBlocked) {
            finalContent =
              "This turn reached an action that the current approval mode could not authorize, so Grok stopped without completing the request. No changes were made. Use Ask before actions for normal project exploration, or Full access only when you trust the project, then retry.";
          } else if (agentCancelled) {
            finalContent = "Grok cancelled the turn before completing the request. Please retry.";
          } else if (!ev.payload.success && !buffer) {
            finalContent = `Grok exited with code ${ev.payload.exit_code}.`;
          } else if (verbose) {
            finalContent = buffer || last.content;
          } else {
            // Hidden: product-facing reply; raw kept for reveal.
            finalContent = extractFinalReply(buffer) || last.content || "No response.";
          }

          msgs[msgs.length - 1] = {
            ...last,
            content: finalContent,
            rawContent: buffer || last.rawContent || finalContent,
            streaming: false,
            phaseLabel: undefined,
            status: ev.payload.success ? "ok" : "error",
            // Keep raw collapsed unless already verbose for this session feel.
            rawVisible: verbose ? true : last.rawVisible === true,
          };
          return { ...chat, messages: msgs };
        }
        return chat;
      });

      const chat = get(currentChat);
      if (chat) {
        const last = chat.messages[chat.messages.length - 1];
        if (last?.role === "assistant" && !last.streaming) {
          try {
            // Persist product content; raw is session-local for now (JSON shape may omit extra fields).
            await invokeWithRetry("append_chat_message", {
              chatId: chat.id,
              messageId: last.id,
              role: "assistant",
              content: last.content,
              images: [],
              status: last.status ?? null,
            });
          } catch (e) {
            debugWarn("persist assistant", e);
            showError(
              `The response is visible but could not be saved to chat history. ${humanizeError(e)}`,
            );
          }
        }
      }
    }),
  );

  unlisteners.push(
    await listen("tray-new-chat", async () => {
      const chat = get(currentChat);
      await createNewChat(chat?.project_id ?? null);
    }),
  );

  unlisteners.push(
    await listen("tray-toggle-yolo", async () => {
      const next = !get(yoloEnabled);
      yoloEnabled.set(next);
      try {
        await invoke("set_session_yolo", { yolo: next });
      } catch {
        /* session may not be started yet */
      }
    }),
  );
}

export async function resolvePermission(optionId: string | null): Promise<void> {
  const request = get(pendingPermission);
  if (!request) return;
  try {
    await invoke("resolve_grok_permission", {
      requestId: request.request_id,
      optionId,
    });
    pendingPermission.set(null);
  } catch (error) {
    showError(error);
  }
}

export async function resolveInteraction(response: Record<string, unknown>): Promise<void> {
  const request = get(pendingInteraction);
  if (!request) return;
  try {
    await invoke("resolve_grok_interaction", {
      requestId: request.request_id,
      response,
    });
    pendingInteraction.set(null);
  } catch (error) {
    showError(error);
  }
}

export async function refreshChatList(projectId: string | null): Promise<void> {
  try {
    const list = await invoke<ChatSession[]>("list_chats", {
      projectId,
    });
    chatList.set(list);
  } catch (e) {
    debugWarn("refreshChatList", e);
  }
}

async function haltActiveTurn(): Promise<void> {
  if (get(isRunning)) {
    try {
      await invoke("stop_session");
    } catch (e) {
      debugWarn("haltActiveTurn", e);
    }
  }
  isRunning.set(false);
  runningSince.set(null);
  streamBuffer.set("");
  await discardPendingAttachments();
}

async function discardPendingAttachments(): Promise<void> {
  const attachments = get(pendingAttachments);
  pendingAttachments.set([]);
  await Promise.allSettled(
    attachments.map((attachment) => invoke("discard_temp_image", { path: attachment.path })),
  );
}

export async function createNewChat(projectId: string | null): Promise<ChatSession> {
  await haltActiveTurn();
  const session = await invoke<ChatSession>("new_chat", {
    projectId,
    title: null,
  });
  currentChat.set({ ...session, messages: session.messages ?? [] });
  await refreshChatList(projectId);
  return session;
}

export async function openChat(chatId: string): Promise<void> {
  await haltActiveTurn();
  const session = await invoke<ChatSession>("load_chat", { chatId });
  currentChat.set({
    ...session,
    messages: (session.messages ?? []).map((m) => ({
      ...m,
      role: m.role as ChatMessage["role"],
    })),
  });
}

export async function deleteChat(chatId: string): Promise<void> {
  await haltActiveTurn();
  const active = get(currentChat);
  const projectId = active?.project_id ?? null;

  try {
    await invoke("delete_chat", { chatId });
    await refreshChatList(projectId);
    if (active?.id !== chatId) return;

    const next = get(chatList)[0];
    if (next) {
      await openChat(next.id);
    } else {
      await createNewChat(projectId);
    }
  } catch (e) {
    showError(e);
  }
}

export async function resolveDefaultCwd(): Promise<string> {
  try {
    return await invoke<string>("get_app_data_dir");
  } catch {
    return ".";
  }
}

export async function ensureSession(model: string, yolo: boolean, cwd: string): Promise<void> {
  const chat = get(currentChat);
  let workdir = cwd.trim();
  if (!workdir || workdir === ".") {
    workdir = await resolveDefaultCwd();
  }
  const key = JSON.stringify({ model, yolo, workdir, chatId: chat?.id ?? null });
  if (activeSessionKey === key) return;
  await invoke("start_grok_session", {
    model,
    yolo,
    cwd: workdir,
    chatId: chat?.id ?? null,
  });
  activeSessionKey = key;
}

export async function sendUserMessage(
  prompt: string,
  cwd: string,
  attachmentPathsOverride?: string[],
): Promise<void> {
  // Local reveal intent — no agent round-trip required.
  if (maybeHandleRevealRequest(prompt)) {
    return;
  }

  const attachmentPaths =
    attachmentPathsOverride ?? get(pendingAttachments).map((attachment) => attachment.path);
  const model = get(selectedModel);
  const yolo = get(yoloEnabled);

  if (!prompt.trim() && attachmentPaths.length === 0) return;
  if (get(isRunning)) {
    showError("Wait for the current response to finish, or stop it.");
    return;
  }
  isRunning.set(true);
  runningSince.set(Date.now());

  let chat = get(currentChat);
  if (!chat) {
    try {
      isRunning.set(false);
      runningSince.set(null);
      chat = await createNewChat(null);
      isRunning.set(true);
      runningSince.set(Date.now());
    } catch (e) {
      isRunning.set(false);
      runningSince.set(null);
      showError(String(e));
      return;
    }
  }

  try {
    await ensureSession(model, yolo, cwd);
  } catch (e) {
    isRunning.set(false);
    runningSince.set(null);
    showError(String(e));
    return;
  }

  const userMsg: ChatMessage = {
    id: uid(),
    role: "user",
    content: prompt,
    images: attachmentPaths,
    timestamp: new Date().toISOString(),
  };

  const assistantMsg: ChatMessage = {
    id: uid(),
    role: "assistant",
    content: "",
    images: [],
    timestamp: new Date().toISOString(),
    streaming: true,
    phaseLabel: "Thinking…",
    rawContent: "",
    rawVisible: get(verboseMode),
  };

  currentChat.update((c) => {
    if (!c) return c;
    return { ...c, messages: [...c.messages, userMsg, assistantMsg] };
  });
  streamBuffer.set("");
  status.update((s) => ({
    ...s,
    state: "running",
    detail: "Thinking…",
    running: true,
  }));

  try {
    const updated = await invokeWithRetry<ChatSession>("append_chat_message", {
      chatId: chat.id,
      messageId: userMsg.id,
      role: "user",
      content: prompt,
      images: attachmentPaths,
      status: null,
    });
    // Sync auto-title / timestamps into UI without wiping the streaming bubble.
    currentChat.update((c) => {
      if (!c || c.id !== updated.id) return c;
      return {
        ...c,
        title: updated.title || c.title,
        updated_at: updated.updated_at,
      };
    });
    void refreshChatList(chat.project_id);
  } catch (e) {
    isRunning.set(false);
    runningSince.set(null);
    const message = `Could not save this message. Nothing was sent to Grok. ${humanizeError(e)}`;
    showError(message);
    currentChat.update((c) => {
      if (!c) return c;
      const messages = [...c.messages];
      const last = messages[messages.length - 1];
      if (last?.role === "assistant" && last.streaming) {
        messages[messages.length - 1] = {
          ...last,
          content: "Message was not sent because chat history could not be saved.",
          streaming: false,
          phaseLabel: undefined,
          status: "error",
        };
      }
      return { ...c, messages };
    });
    return;
  }

  try {
    await invoke("send_message", {
      prompt,
      attachmentPaths,
    });
    if (!attachmentPathsOverride) pendingAttachments.set([]);
  } catch (e) {
    isRunning.set(false);
    runningSince.set(null);
    const msg = String(e);
    // grok-done already finalizes the bubble; only toast unexpected failures.
    // "Cancelled" and non-zero exit are expected completion paths with UI state already set.
    if (msg.includes("Cancelled")) {
      return;
    }
    const stillStreaming = (() => {
      const c = get(currentChat);
      const last = c?.messages[c.messages.length - 1];
      return !!(last?.role === "assistant" && last.streaming);
    })();
    if (stillStreaming) {
      showError(msg);
      currentChat.update((c) => {
        if (!c) return c;
        const msgs = [...c.messages];
        const last = msgs[msgs.length - 1];
        if (last?.role === "assistant" && last.streaming) {
          msgs[msgs.length - 1] = {
            ...last,
            content: last.content || `Error: ${msg}`,
            streaming: false,
            phaseLabel: undefined,
            status: "error",
          };
        }
        return { ...c, messages: msgs };
      });
    } else if (!msg.includes("exited with code")) {
      // Non-exit errors (spawn failure, etc.) after bubble finalized — still surface once.
      showError(msg);
    }
  }
}

export async function stopGeneration(): Promise<void> {
  try {
    await invoke("stop_session");
  } catch (e) {
    debugWarn("stop", e);
    showError(e);
  }
  isRunning.set(false);
  runningSince.set(null);
}

export function addPendingAttachment(attachment: PendingAttachment) {
  pendingAttachments.update((list) => {
    if (list.some((item) => item.path === attachment.path)) return list;
    return [...list, attachment];
  });
}

export function removePendingAttachment(id: string) {
  const attachment = get(pendingAttachments).find((item) => item.id === id);
  pendingAttachments.update((list) => list.filter((item) => item.id !== id));
  if (attachment) {
    void invoke("discard_temp_image", { path: attachment.path }).catch((e) =>
      debugWarn("discard pending image", e),
    );
  }
}

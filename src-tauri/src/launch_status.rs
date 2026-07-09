//! Launch / WebView load status — honest reporting, no false success.
//!
//! The UI is considered ready only after the Svelte app calls `report_ui_ready`.
//! Page-load finished alone is NOT success (WebView error pages also "finish").

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, WebviewWindow};

/// Shared launch telemetry for the main window.
pub struct LaunchStatus {
    /// Frontend JS finished booting and called report_ui_ready.
    pub ui_ready: AtomicBool,
    /// Detected connection / load failure (error page or timeout).
    pub load_failed: AtomicBool,
    /// Human-readable phase for the splash / console.
    pub phase: Mutex<String>,
    /// Last error detail (if any).
    pub last_error: Mutex<Option<String>>,
}

impl Default for LaunchStatus {
    fn default() -> Self {
        Self {
            ui_ready: AtomicBool::new(false),
            load_failed: AtomicBool::new(false),
            phase: Mutex::new("Starting…".into()),
            last_error: Mutex::new(None),
        }
    }
}

#[derive(Clone, Serialize)]
pub struct LaunchStatusSnapshot {
    pub phase: String,
    pub ui_ready: bool,
    pub load_failed: bool,
    pub last_error: Option<String>,
    pub success: bool,
}

impl LaunchStatus {
    pub fn snapshot(&self) -> LaunchStatusSnapshot {
        let ui_ready = self.ui_ready.load(Ordering::SeqCst);
        let load_failed = self.load_failed.load(Ordering::SeqCst);
        LaunchStatusSnapshot {
            phase: self
                .phase
                .lock()
                .map(|p| p.clone())
                .unwrap_or_else(|_| "Unknown".into()),
            ui_ready,
            load_failed,
            last_error: self.last_error.lock().ok().and_then(|e| e.clone()),
            // Never claim success if the WebView failed or UI never checked in.
            success: ui_ready && !load_failed,
        }
    }

    pub fn set_phase(&self, app: &AppHandle, phase: impl Into<String>) {
        let phase = phase.into();
        if let Ok(mut p) = self.phase.lock() {
            *p = phase.clone();
        }
        let snap = self.snapshot();
        let _ = app.emit("launch-status", snap);
    }

    pub fn mark_ready(&self, app: &AppHandle) {
        self.ui_ready.store(true, Ordering::SeqCst);
        self.load_failed.store(false, Ordering::SeqCst);
        if let Ok(mut e) = self.last_error.lock() {
            *e = None;
        }
        self.set_phase(app, "UI ready");
    }

    pub fn mark_failed(&self, app: &AppHandle, reason: impl Into<String>) {
        let reason = reason.into();
        self.load_failed.store(true, Ordering::SeqCst);
        self.ui_ready.store(false, Ordering::SeqCst);
        if let Ok(mut e) = self.last_error.lock() {
            *e = Some(reason.clone());
        }
        self.set_phase(app, format!("Connection failed — {reason}"));
    }
}

/// Detect Chromium / WebView2 connection-error pages by URL or document content.
pub fn looks_like_connection_error(url: &str, title: &str, body_sample: &str) -> bool {
    let u = url.to_ascii_lowercase();
    let t = title.to_ascii_lowercase();
    let b = body_sample.to_ascii_lowercase();

    if u.contains("chrome-error://")
        || u.contains("chromewebdata")
        || u.starts_with("data:text/html,") && b.contains("grok desktop could not load")
    {
        return true;
    }
    if t.contains("can't reach")
        || t.contains("cannot reach")
        || t.contains("not available")
        || t.contains("this site can’t be reached")
        || t.contains("this site can't be reached")
        || t.contains("err_")
    {
        return true;
    }
    if b.contains("err_connection_refused")
        || b.contains("err_name_not_resolved")
        || b.contains("err_connection_timed_out")
        || b.contains("err_address_unreachable")
        || b.contains("took too long to respond")
        || b.contains("refused to connect")
        || b.contains("can't reach this page")
        || b.contains("cannot reach this page")
        || b.contains("this site can’t be reached")
        || b.contains("this site can't be reached")
    {
        return true;
    }
    false
}

/// Inject a clear, self-contained error page into the WebView (no external assets).
pub fn inject_load_error_page(window: &WebviewWindow, detail: &str) {
    let detail_js = serde_json::to_string(detail).unwrap_or_else(|_| "\"Unknown error\"".into());
    let script = format!(
        r#"(function() {{
  try {{
    var detail = {detail_js};
    var html = '<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"/>'
      + '<meta name="viewport" content="width=device-width, initial-scale=1"/>'
      + '<title>Grok Desktop — UI failed to load</title>'
      + '<style>'
      + 'body{{margin:0;font-family:Segoe UI,system-ui,sans-serif;background:#0c0c0e;color:#ececf1;'
      + 'display:flex;min-height:100vh;align-items:center;justify-content:center;padding:2rem;}}'
      + '.card{{max-width:520px;border:1px solid #2a2a32;border-radius:14px;padding:1.5rem 1.75rem;'
      + 'background:#16161a;box-shadow:0 16px 40px rgba(0,0,0,.45);}}'
      + 'h1{{margin:0 0 .5rem;font-size:1.25rem;color:#38bdf8;}}'
      + 'p{{line-height:1.5;color:#b8b8c8;margin:.55rem 0;}}'
      + 'code{{background:#1e1e24;padding:.1rem .35rem;border-radius:4px;font-size:.9em;}}'
      + 'ul{{margin:.4rem 0 1rem;padding-left:1.2rem;color:#b8b8c8;line-height:1.45;}}'
      + 'button{{margin-right:.5rem;margin-top:.25rem;border:none;border-radius:8px;padding:.5rem 1rem;'
      + 'font-weight:600;cursor:pointer;font-family:inherit;}}'
      + '.primary{{background:linear-gradient(135deg,#7dd3fc,#0ea5e9);color:#03131f;}}'
      + '.ghost{{background:#1e1e24;color:#ececf1;border:1px solid #2a2a32;}}'
      + '.detail{{font-size:.8rem;color:#8b8b9a;word-break:break-word;}}'
      + '</style></head><body><div class="card">'
      + '<h1>Could not load the Grok Desktop UI</h1>'
      + '<p>The window opened, but the UI never finished loading.</p>'
      + '<p class="detail">Detail: ' + detail.replace(/</g,'&lt;') + '</p>'
      + '<p><strong>Most common cause (Tauri 2):</strong> a <em>debug</em> binary always '
      + 'tries <code>http://127.0.0.1:1420</code>. If Vite is not running, you get this page.</p>'
      + '<p><strong>Fix that works:</strong></p>'
      + '<ul>'
      + '<li>Close this window completely</li>'
      + '<li>Run <code>Grok-Desktop.bat</code> or <code>npm start</code></li>'
      + '<li>That builds a <strong>RELEASE</strong> binary that loads <code>build\\</code> offline '
      + '(no port 1420)</li>'
      + '<li>Wait for the first release compile to finish</li>'
      + '</ul>'
      + '<p><strong>Hot-reload only:</strong> <code>npm run start:dev</code> and leave that console open.</p>'
      + '<button class="primary" onclick="location.reload()">Retry load</button>'
      + '<button class="ghost" onclick="window.__TAURI__&&window.__TAURI__.core&&window.__TAURI__.core.invoke&&window.__TAURI__.core.invoke(\'retry_ui_load\')">Retry via app</button>'
      + '</div></body></html>';
    document.open();
    document.write(html);
    document.close();
  }} catch (e) {{
    console.error('inject error page failed', e);
  }}
}})();"#
    );
    let _ = window.eval(&script);
}

/// JS probe run after PageLoadEvent::Finished — reports connection-error pages to Rust.
pub fn connection_probe_script() -> &'static str {
    r#"(function() {
  try {
    var title = document.title || '';
    var body = (document.body && (document.body.innerText || document.body.textContent)) || '';
    body = String(body).slice(0, 500);
    var url = location.href || '';
    var bad = /chrome-error:|chromewebdata/i.test(url)
      || /can't reach|cannot reach|can't be reached|ERR_/i.test(title)
      || /ERR_CONNECTION|refused to connect|took too long|can't reach this page|cannot reach this page/i.test(body);
    if (bad && window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
      window.__TAURI__.core.invoke('report_ui_failed', {
        reason: (title || 'connection error') + ' @ ' + url
      });
    }
  } catch (e) {}
})();"#
}

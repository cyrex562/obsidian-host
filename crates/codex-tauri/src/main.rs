// Prevents a console window from appearing on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod paths;

use anyhow::Context;
use codex::config::AppConfig;
use paths::{create_dirs, resolve_platform_paths};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tracing::{error, info, warn};

// ── Tray icon pixel data ──────────────────────────────────────────────────────

const TRAY_ICON_YELLOW: &[u8] = include_bytes!("../icons/tray-yellow.png");
const TRAY_ICON_GREEN: &[u8] = include_bytes!("../icons/tray-green.png");
const TRAY_ICON_RED: &[u8] = include_bytes!("../icons/tray-red.png");

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Open a native directory picker dialog and return the selected path.
///
/// Called from the Vue frontend via `window.__TAURI__.core.invoke()`.
#[tauri::command]
async fn open_directory_dialog(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel::<Option<String>>();
    app.dialog()
        .file()
        .set_title("Select Vault Directory")
        .pick_folder(move |folder| {
            let path = folder.map(|f| f.to_string());
            let _ = tx.send(path);
        });
    rx.await.unwrap_or(None)
}

/// Send a native desktop notification.
///
/// Falls back silently when the platform does not support notifications or when
/// permission has not been granted — the caller should not treat this as fatal.
#[tauri::command]
async fn notify(app: AppHandle, title: String, body: String) -> Result<(), String> {
    use tauri_plugin_notification::NotificationExt;
    app.notification()
        .builder()
        .title(&title)
        .body(&body)
        .show()
        .map_err(|e| e.to_string())
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_deep_link::init())
        .invoke_handler(tauri::generate_handler![open_directory_dialog, notify])
        .setup(|app| run_setup(app).map_err(|e| e.into()))
        .run(tauri::generate_context!())
        .expect("error while running Codex");
}

/// Main setup logic extracted from the Tauri setup hook.
///
/// Returning `anyhow::Result` makes it easy to use `?` throughout; the
/// closure converts the error to `Box<dyn std::error::Error>` at the boundary.
fn run_setup(app: &mut tauri::App) -> anyhow::Result<()> {
    let handle = app.handle().clone();

    // 1. Resolve platform directories (XDG / Library / AppData) and create them.
    let paths = resolve_platform_paths(&handle)?;
    create_dirs(&paths)?;
    info!(
        "App directories: config={:?} data={:?}",
        paths.config_dir, paths.data_dir
    );

    // 2. Load or create configuration (first-launch branch).
    let config_file = paths.config_dir.join("config.toml");
    let config = if config_file.exists() {
        info!("Loading existing config from {:?}", config_file);
        AppConfig::load_from_dirs(&paths)?
    } else {
        warn!("No config.toml found — first launch. Writing default config.");
        // Create the default vault directory before writing config.
        std::fs::create_dir_all(&paths.default_vault_dir)
            .context("Failed to create default vault directory")?;
        AppConfig::write_default(&paths)?
    };

    // 3. Set up the system tray (yellow = starting).
    setup_tray(app)?;

    // 4. Register deep-link handler for codex:// URLs.
    setup_deep_links(&handle)?;

    // 5. Propagate server startup errors back to this thread.
    let (err_tx, err_rx) = std::sync::mpsc::channel::<String>();

    // 6. Spawn the Actix server on a dedicated OS thread with its own runtime.
    //    Tauri must own the main thread on Linux/macOS; Actix is kept separate.
    let config_for_server = config.clone();
    std::thread::spawn(move || {
        if let Err(e) =
            actix_web::rt::System::new().block_on(async { codex::run(config_for_server).await })
        {
            error!("Server thread exited with error: {e:#}");
            let _ = err_tx.send(format!("{e:#}"));
        }
    });

    // 7. Get the main WebView window (title defined in tauri.conf.json).
    let window = handle
        .get_webview_window("main")
        .context("main webview window not found")?;

    // 8. Poll /api/health asynchronously; navigate the WebView on success.
    let port = config.server.port;
    let handle_for_poll = handle.clone();
    tauri::async_runtime::spawn(async move {
        poll_until_healthy_then_navigate(port, window, handle_for_poll, err_rx).await;
    });

    Ok(())
}

// ── System tray ───────────────────────────────────────────────────────────────

fn setup_tray(app: &tauri::App) -> anyhow::Result<()> {
    let open_item = MenuItem::with_id(app, "open", "Open Codex", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&open_item, &separator, &quit_item])?;

    let yellow_icon = tauri::image::Image::from_bytes(TRAY_ICON_YELLOW)
        .context("Failed to load starting tray icon")?;

    TrayIconBuilder::with_id("main-tray")
        .icon(yellow_icon)
        .tooltip("Codex — Starting…")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            "quit" => {
                info!("Quit requested from tray menu");
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Left-click on the tray icon brings the window to focus.
            if let TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
        })
        .build(app)
        .context("Failed to build system tray")?;

    Ok(())
}

/// Update the tray icon and tooltip to reflect the current server status.
///
/// `status` is one of `"starting"`, `"healthy"`, or `"error"`.
fn update_tray_status(app: &AppHandle, status: &str) {
    let Some(tray) = app.tray_by_id("main-tray") else {
        return;
    };

    let (icon_bytes, tooltip) = match status {
        "healthy" => (TRAY_ICON_GREEN, "Codex — Running"),
        "error" => (TRAY_ICON_RED, "Codex — Error"),
        _ => (TRAY_ICON_YELLOW, "Codex — Starting…"),
    };

    if let Ok(icon) = tauri::image::Image::from_bytes(icon_bytes) {
        let _ = tray.set_icon(Some(icon));
    }
    let _ = tray.set_tooltip(Some(tooltip));
}

// ── Deep links ────────────────────────────────────────────────────────────────

fn setup_deep_links(handle: &AppHandle) -> anyhow::Result<()> {
    use tauri_plugin_deep_link::DeepLinkExt;

    // Register the codex:// scheme at runtime (required on Linux/Windows;
    // on macOS registration is done via Info.plist bundled at build time).
    #[cfg(not(target_os = "macos"))]
    handle
        .deep_link()
        .register("codex")
        .context("Failed to register codex:// deep link scheme")?;

    let handle_clone = handle.clone();
    handle.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            info!("Deep link received: {url}");
            // Navigate the main window to the path encoded in the codex:// URL.
            // E.g. codex://open/vault/abc/file/note.md → /vault/abc/file/note.md
            if let Some(win) = handle_clone.get_webview_window("main") {
                let nav_path = deep_link_to_app_path(url.as_str());
                let js = format!("window.location.hash = {nav_path:?}");
                let _ = win.eval(&js);
                let _ = win.show();
                let _ = win.set_focus();
            }
        }
    });

    Ok(())
}

/// Convert a `codex://` URL to an in-app hash-router path.
///
/// `codex://open/vault/abc/file/note.md` → `#/vault/abc/file/note.md`
///
/// Any URL that doesn't match `codex://open/...` is mapped to `#/`.
pub(crate) fn deep_link_to_app_path(url: &str) -> String {
    // Strip the scheme and host part ("codex://open"), keep the path.
    let stripped = url
        .strip_prefix("codex://open")
        .or_else(|| url.strip_prefix("codex://"))
        .unwrap_or("/");
    let path = if stripped.is_empty() { "/" } else { stripped };
    format!("#{path}")
}

// ── Health polling ────────────────────────────────────────────────────────────

/// Poll `GET /api/health` every 100 ms for up to 10 s.
///
/// On a successful response, navigates the WebView to the running Codex app
/// and updates the tray icon to green.
/// On timeout or a server startup error received via `err_rx`, shows an inline
/// error screen and updates the tray icon to red.
async fn poll_until_healthy_then_navigate(
    port: u16,
    window: tauri::WebviewWindow,
    app: AppHandle,
    err_rx: std::sync::mpsc::Receiver<String>,
) {
    let url = format!("http://localhost:{port}/api/health");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .expect("failed to build reqwest client");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);

    loop {
        // Check for an error reported by the server thread.
        if let Ok(err) = err_rx.try_recv() {
            let message = classify_server_error(&err, port);
            update_tray_status(&app, "error");
            show_error_screen(&window, &message);
            return;
        }

        if std::time::Instant::now() > deadline {
            update_tray_status(&app, "error");
            show_error_screen(
                &window,
                "Codex did not become ready within 10 seconds.\nCheck the application logs for details.",
            );
            return;
        }

        match client.get(&url).send().await {
            Ok(r) if r.status().is_success() => {
                info!("Server healthy — navigating WebView to http://localhost:{port}");
                update_tray_status(&app, "healthy");
                let _ = window.eval(&format!(
                    "window.location.replace('http://localhost:{port}')"
                ));
                return;
            }
            _ => {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }
}

/// Classify a server startup error string into a user-readable message.
pub(crate) fn classify_server_error(raw: &str, port: u16) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("address already in use")
        || lower.contains("already in use")
        || lower.contains("os error 98")  // EADDRINUSE on Linux
        || lower.contains("os error 48")  // EADDRINUSE on macOS
        || lower.contains("only one usage")
    // Windows WSAEADDRINUSE
    {
        format!(
            "Port {port} is already in use.\n\
            Another instance of Codex may be running, or a different program is \
            occupying that port.\n\nClose the other application and relaunch Codex."
        )
    } else if lower.contains("permission denied") || lower.contains("os error 13") {
        format!(
            "Permission denied when binding to port {port}.\n\
            Ports below 1024 require elevated privileges. \
            Change the port in your config.toml to a value above 1024."
        )
    } else {
        format!("Server failed to start:\n{raw}")
    }
}

/// Replace the WebView content with a simple error screen.
fn show_error_screen(window: &tauri::WebviewWindow, message: &str) {
    let escaped = message
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "<br>");
    let js = format!(
        r#"document.body.innerHTML = '<div style="display:flex;height:100vh;\
flex-direction:column;align-items:center;justify-content:center;\
font-family:system-ui,sans-serif;color:#c00;padding:24px;text-align:center">\
<h2 style="margin-bottom:12px">Codex failed to start</h2>\
<p style="max-width:480px;color:#444">{escaped}</p></div>';"#
    );
    let _ = window.eval(&js);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_port_in_use_linux() {
        let msg = classify_server_error(
            "Os { code: 98, kind: AddrInUse, message: \"Address already in use\" }",
            8080,
        );
        assert!(
            msg.contains("Port 8080 is already in use"),
            "expected port-conflict message, got: {msg}"
        );
        assert!(
            msg.contains("relaunch Codex"),
            "expected suggestion, got: {msg}"
        );
    }

    #[test]
    fn classify_port_in_use_macos() {
        let msg = classify_server_error("os error 48", 8080);
        assert!(
            msg.contains("Port 8080 is already in use"),
            "macOS EADDRINUSE: {msg}"
        );
    }

    #[test]
    fn classify_port_in_use_windows() {
        let msg = classify_server_error("Only one usage of each socket address", 8080);
        assert!(
            msg.contains("Port 8080 is already in use"),
            "Windows WSAEADDRINUSE: {msg}"
        );
    }

    #[test]
    fn classify_permission_denied() {
        let msg = classify_server_error("permission denied binding port 80", 80);
        assert!(
            msg.contains("Permission denied"),
            "expected permission message, got: {msg}"
        );
        assert!(
            msg.contains("config.toml"),
            "expected config hint, got: {msg}"
        );
    }

    #[test]
    fn classify_generic_error_is_passthrough() {
        let raw = "some unexpected database initialization failure";
        let msg = classify_server_error(raw, 8080);
        assert!(msg.contains(raw), "raw error should be included: {msg}");
        assert!(msg.starts_with("Server failed to start:"));
    }

    #[test]
    fn classify_preserves_port_in_messages() {
        for port in [80u16, 443, 3000, 8080, 51234] {
            let msg = classify_server_error("address already in use", port);
            assert!(
                msg.contains(&port.to_string()),
                "port {port} not in message: {msg}"
            );
        }
    }

    // ── deep link tests ───────────────────────────────────────────────────────

    #[test]
    fn deep_link_open_vault_file() {
        let path = deep_link_to_app_path("codex://open/vault/abc/file/note.md");
        assert_eq!(path, "#/vault/abc/file/note.md");
    }

    #[test]
    fn deep_link_bare_scheme() {
        let path = deep_link_to_app_path("codex://");
        assert_eq!(path, "#/");
    }

    #[test]
    fn deep_link_open_root() {
        let path = deep_link_to_app_path("codex://open");
        assert_eq!(path, "#/");
    }

    #[test]
    fn deep_link_open_with_query() {
        let path = deep_link_to_app_path("codex://open/search?q=hello");
        assert_eq!(path, "#/search?q=hello");
    }

    #[test]
    fn deep_link_unknown_host_prefix_stripped() {
        // Anything under codex:// that doesn't start with /open falls through
        // to the codex:// fallback.
        let path = deep_link_to_app_path("codex://vault/xyz");
        assert_eq!(path, "#/vault/xyz");
    }
}

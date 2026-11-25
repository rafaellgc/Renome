//! The window's commands: settings.

use super::*;

#[tauri::command]
pub(crate) async fn update_check(app: AppHandle) -> Result<Option<update::Found>, String> {
    update::check(&app).await
}

/// How this copy is updated: by itself, from the release page, or by the
/// system (Flatpak), whose copies do not look for versions.
#[tauri::command]
pub(crate) fn update_way() -> update::Way {
    update::way()
}

#[tauri::command]
pub(crate) async fn update_install(app: AppHandle) -> Result<(), String> {
    update::install(&app).await
}

#[tauri::command]
pub(crate) async fn server_check(server: mediaserver::Server) -> Result<(), String> {
    server.check().await
}

#[tauri::command]
pub(crate) async fn server_read(
    server: Value,
    key: String,
) -> Result<(Value, usize, usize), String> {
    // Pairs kept, paths here paired with themselves (see `mediaserver::read`,
    // the command line's `servers` too).
    mediaserver::read(server, &key).await
}

/// A new media server for the options.
#[tauri::command]
pub(crate) fn server_new() -> Value {
    mediaserver::new_server("jellyfin", "")
}

/// The servers' keys (one secret) read, or with one server's set.
#[tauri::command]
pub(crate) fn server_keys(raw: String) -> serde_json::Map<String, Value> {
    mediaserver::keys_of(&raw)
}

#[tauri::command]
pub(crate) fn server_key_with(raw: String, id: String, key: String) -> String {
    mediaserver::with_key(&raw, &id, &key)
}

/// Tells the servers in the options (`servers`, with their keys,
/// `serverKeys`) about the folders the renamed files left and went to
/// (the choice of servers is the command line's too).
#[tauri::command]
pub(crate) async fn server_tell(
    servers: Value,
    keys: String,
    done: Vec<renome_core::Operation>,
) -> Vec<renome_services::flow::after::Line> {
    // What is said of it (`flow::said::told`, the command line's too).
    let answers = mediaserver::tell_all(mediaserver::callable(&servers, &keys), &done).await;
    renome_services::flow::said::told(&answers)
}

/// Whether this system's web view can print books to PDF.
#[tauri::command]
pub(crate) fn pdf_print_available() -> bool {
    cfg!(target_os = "linux")
}

/// Prints the page `html` to the PDF file `pdf` with a hidden web view.
pub(crate) async fn print_to_pdf(
    app: &AppHandle,
    html: &Path,
    pdf: &Path,
    page: renome_core::bookconv::print::Page,
) -> Result<(), String> {
    use tauri::webview::PageLoadEvent;
    use tauri::{WebviewUrl, WebviewWindowBuilder};
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let url = tauri::Url::from_file_path(html).map_err(|_| "pdfPrint".to_string())?;
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let tx = std::sync::Mutex::new(Some(tx));
    let label = format!(
        "print{}",
        N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let win = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url))
        .visible(false)
        .on_page_load(move |_, p| {
            if p.event() == PageLoadEvent::Finished
                && let Some(t) = tx.locked().take()
            {
                let _ = t.send(());
            }
        })
        .build()
        .map_err(|e| e.to_string())?;
    let loaded = tokio::time::timeout(std::time::Duration::from_secs(120), rx).await;
    if !matches!(loaded, Ok(Ok(()))) {
        let _ = win.destroy();
        return Err("pdfPrint".into());
    }
    let (dtx, drx) = tokio::sync::oneshot::channel::<Result<(), String>>();
    let dtx = std::sync::Arc::new(std::sync::Mutex::new(Some(dtx)));
    let out = pdf.to_path_buf();
    let started = print_webview(&win, out, page, dtx.clone());
    if let Err(e) = started {
        let _ = win.destroy();
        return Err(e);
    }
    let r = tokio::time::timeout(std::time::Duration::from_secs(600), drx).await;
    let _ = win.destroy();
    match r {
        Ok(Ok(r)) => r,
        _ => Err("pdfPrint".into()),
    }
}

pub(crate) type PrintDone =
    std::sync::Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<Result<(), String>>>>>;

#[cfg(target_os = "linux")]
pub(crate) fn print_webview(
    win: &tauri::WebviewWindow,
    out: PathBuf,
    page: renome_core::bookconv::print::Page,
    done: PrintDone,
) -> Result<(), String> {
    win.with_webview(move |pw| {
        use webkit2gtk::PrintOperationExt;
        let send = move |r: Result<(), String>| {
            if let Some(t) = done.locked().take() {
                let _ = t.send(r);
            }
        };
        let Ok(uri) = tauri::Url::from_file_path(&out) else {
            send(Err("pdfPrint".into()));
            return;
        };
        let op = webkit2gtk::PrintOperation::new(&pw.inner());
        let settings = gtk::PrintSettings::new();
        // The printer that writes files has a name in the desktop's
        // language: GTK's own translation of it.
        let name = gtk::glib::dgettext(Some("gtk30"), "Print to File");
        settings.set_printer(name.as_str());
        settings.set(gtk::PRINT_SETTINGS_OUTPUT_FILE_FORMAT, Some("pdf"));
        settings.set(gtk::PRINT_SETTINGS_OUTPUT_URI, Some(uri.as_str()));
        op.set_print_settings(&settings);
        let setup = gtk::PageSetup::new();
        if page == renome_core::bookconv::print::Page::A5 {
            setup.set_paper_size(&gtk::PaperSize::new(Some("iso_a5")));
        } else {
            setup.set_paper_size(&gtk::PaperSize::new_custom(
                "renome-screen",
                "Reader",
                page.width,
                page.height,
                gtk::Unit::Mm,
            ));
        }
        setup.set_top_margin(page.top, gtk::Unit::Mm);
        setup.set_bottom_margin(page.bottom, gtk::Unit::Mm);
        setup.set_left_margin(page.side, gtk::Unit::Mm);
        setup.set_right_margin(page.side, gtk::Unit::Mm);
        op.set_page_setup(&setup);
        let send = std::rc::Rc::new(std::cell::RefCell::new(Some(send)));
        let s1 = send.clone();
        // The operation is kept until it ends.
        let keep = op.clone();
        op.connect_finished(move |_| {
            let _ = &keep;
            if let Some(f) = s1.borrow_mut().take() {
                f(Ok(()));
            }
        });
        let s2 = send;
        op.connect_failed(move |_, e| {
            if let Some(f) = s2.borrow_mut().take() {
                f(Err(format!("pdfPrint: {e}")));
            }
        });
        op.print();
    })
    .map_err(|e| e.to_string())
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn print_webview(
    _: &tauri::WebviewWindow,
    _: PathBuf,
    _: renome_core::bookconv::print::Page,
    _: PrintDone,
) -> Result<(), String> {
    Err("pdfPrint".into())
}

/// Saved settings; `None` on first run. The format is defined by the UI.
/// Secrets come apart ([`load_secrets`]), unless the file still has them.
#[tauri::command]
pub(crate) fn load_settings(fallback_lang: String) -> Result<Value, String> {
    let _file = SETTINGS_FILE.locked();
    match read_settings_file() {
        Some(saved) => Ok(renome_services::options::whole(Some(saved))),
        // The first run: the system's language, saved.
        None => {
            let mut first = renome_services::options::whole(None);
            let mut sys = renome_services::system_language();
            if sys.is_empty() {
                // The webview's language where the system says none.
                sys = fallback_lang
                    .chars()
                    .take(2)
                    .collect::<String>()
                    .to_lowercase();
            }
            first["lang"] = Value::from(renome_services::options::ui_lang(&sys).unwrap_or("en"));
            write_settings_file(&first)?;
            Ok(first)
        }
    }
}

/// Saves the settings but their secrets, which [`save_secrets`] saves:
/// those in the file are kept as they are.
#[tauri::command]
pub(crate) async fn save_settings(mut settings: Value) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _file = SETTINGS_FILE.locked();
        let old = read_settings_file()
            .and_then(|v| v.as_object().map(secrets::of))
            .unwrap_or_default();
        if let Some(obj) = settings.as_object_mut() {
            for k in secrets::FIELDS {
                obj.remove(k);
            }
            obj.extend(old);
            if let Some(Value::Array(rules)) = obj.get_mut("rules") {
                renome_core::catalog::normalize_steps(rules);
            }
        }
        ANSWERS.set_days(cache_days(Some(&settings)));
        write_settings_file(&settings)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The secrets from the system's password store, which may wait for the
/// user to unlock it or allow it, and how that went: `stored` (then
/// `secrets` are those), `refused` (the store did not answer: what it holds
/// stays there, untouched) or `missing` (no store: those in the file
/// hold). Secrets still in the file (from before the store was used, or
/// saved there while it did not answer, so the newest) move to the store.
#[tauri::command]
pub(crate) async fn load_secrets() -> serde_json::Value {
    tauri::async_runtime::spawn_blocking(renome_services::load_secrets)
        .await
        .unwrap_or_else(|_| serde_json::json!({ "status": "refused" }))
}

/// Saves the secrets in the system's password store; in the settings
/// file when the store does not take them (or was not read: then what it
/// holds stays as it is). Whether the store took them.
#[tauri::command]
pub(crate) async fn save_secrets(secrets: serde_json::Map<String, Value>) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || renome_services::save_secrets(secrets))
        .await
        .map_err(|e| e.to_string())?
}

/// System language (`pt_BR.UTF-8` → `pt`), from the locale variables.
#[tauri::command]
pub(crate) fn system_language() -> String {
    renome_services::system_language()
}

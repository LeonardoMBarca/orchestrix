mod daemon_startup;
mod editor;
mod host;
mod journal;
#[cfg(feature = "test-fixtures")]
mod native_reply_fixture;
use editor::{EditorRequest, Grant};
use host::{Host, HostRequest};
use serde_json::Value;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

fn fit_initial_window(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    let Some(monitor) = window.current_monitor()? else {
        return Ok(());
    };
    let area = monitor.work_area();
    let scale = monitor.scale_factor();
    let inner = window.inner_size()?;
    let outer = window.outer_size()?;
    // Tauri's config uses logical pixels. Monitor bounds and these window sizes
    // are physical pixels; keep decoration and taskbar space in that same unit.
    let frame_width = outer.width.saturating_sub(inner.width);
    let frame_height = outer.height.saturating_sub(inner.height);
    let margin = (8.0 * scale).round() as u32;
    let available_width = area
        .size
        .width
        .saturating_sub(frame_width + margin * 2)
        .max(1);
    let available_height = area
        .size
        .height
        .saturating_sub(frame_height + margin * 2)
        .max(1);
    let width = inner.width.min(available_width);
    let height = inner.height.min(available_height);
    window.set_min_size(Some(tauri::PhysicalSize::new(
        ((900.0 * scale).round() as u32).min(available_width),
        ((620.0 * scale).round() as u32).min(available_height),
    )))?;
    window.set_size(tauri::PhysicalSize::new(width, height))?;
    window.set_position(tauri::PhysicalPosition::new(
        area.position.x + (area.size.width.saturating_sub(width + frame_width) / 2) as i32,
        area.position.y + (area.size.height.saturating_sub(height + frame_height) / 2) as i32,
    ))
}

#[tauri::command]
async fn connect_daemon(state: tauri::State<'_, Arc<Host>>) -> Result<Value, String> {
    let host = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || host.connect())
        .await
        .map_err(|_| "native-task-failed")?
}
#[tauri::command]
async fn observe_daemon_startup(state: tauri::State<'_, Arc<Host>>) -> Result<Value, String> {
    let host = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || host.observe_startup())
        .await
        .map_err(|_| "native-task-failed")?
}
#[tauri::command]
fn native_qa_window_metrics(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Arc<Host>>,
) -> Result<Value, String> {
    if !state.native_qa_enabled() {
        return Err("native-validation-unavailable".into());
    }
    let monitor = window
        .current_monitor()
        .map_err(|_| "monitor-unavailable")?
        .ok_or("monitor-unavailable")?;
    let inner = window.inner_size().map_err(|_| "window-size-unavailable")?;
    let outer = window.outer_size().map_err(|_| "window-size-unavailable")?;
    let scale = window
        .scale_factor()
        .map_err(|_| "window-scale-unavailable")?;
    Ok(
        serde_json::json!({"syntheticQa":true,"pid":std::process::id(),"scaleFactor":scale,"innerPhysical":inner,"outerPhysical":outer,"innerLogical":{"width":inner.width as f64/scale,"height":inner.height as f64/scale},"monitorPhysical":monitor.size(),"workAreaPhysical":monitor.work_area(),"maximized":window.is_maximized().map_err(|_| "window-state-unavailable")?}),
    )
}
#[tauri::command]
async fn desktop_request(
    state: tauri::State<'_, Arc<Host>>,
    request: HostRequest,
) -> Result<Value, String> {
    let host = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(feature = "test-fixtures")]
        let inspected = match &request {
            HostRequest::InspectIntegration { integration_id } => Some(integration_id.clone()),
            _ => None,
        };
        // handle() has returned and dropped its guards before any QA-only hold.
        let response = host.handle(request)?;
        #[cfg(feature = "test-fixtures")]
        if let Some(integration_id) = inspected {
            native_reply_fixture::after_inspect(host.native_qa_root(), &integration_id, &response)?;
        }
        Ok(response)
    })
    .await
    .map_err(|_| "native-task-failed")?
}
#[tauri::command]
async fn desktop_poll(
    state: tauri::State<'_, Arc<Host>>,
    cursor: orchestrix_core::persistence::DaemonCursor,
) -> Result<Value, String> {
    let host = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || host.poll(cursor))
        .await
        .map_err(|_| "native-task-failed")?
}
#[tauri::command]
async fn editor_request(
    state: tauri::State<'_, Arc<Host>>,
    request: EditorRequest,
) -> Result<Value, String> {
    let host = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || host.edit(request))
        .await
        .map_err(|_| "native-task-failed")?
}
#[tauri::command]
async fn choose_document(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Host>>,
    kind: String,
) -> Result<Option<Grant>, String> {
    let host = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let file = match kind.as_str() {
            "file" => app.dialog().file().blocking_pick_file(),
            "folder" => app.dialog().file().blocking_pick_folder(),
            _ => return Err("dialog-kind".into()),
        };
        match file {
            None => Ok(None),
            Some(file) => {
                let path = file.into_path().map_err(|_| "native-dialog-path")?;
                host.editor
                    .lock()
                    .map_err(|_| "editor-lock".to_owned())?
                    .grant_dialog_selection(path)
                    .map(Some)
            }
        }
    })
    .await
    .map_err(|_| "native-task-failed")?
}
#[tauri::command]
async fn close_desktop(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Arc<Host>>,
) -> Result<(), String> {
    let host = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || host.close_preference())
        .await
        .map_err(|_| "native-task-failed")??;
    window.destroy().map_err(|_| "window-close".into())
}
#[tauri::command]
async fn choose_policy_import(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Host>>,
) -> Result<Option<Value>, String> {
    let host = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(file) = app
            .dialog()
            .file()
            .add_filter("Orchestrix policy", &["json"])
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let path = file.into_path().map_err(|_| "native-dialog-path")?;
        host.select_policy_import(&path).map(Some)
    })
    .await
    .map_err(|_| "native-task-failed")?
}
#[tauri::command]
async fn save_policy_export(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Host>>,
    export_ref: String,
) -> Result<Option<Value>, String> {
    let host = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(file) = app
            .dialog()
            .file()
            .set_file_name("orchestrix-policy.json")
            .add_filter("Orchestrix policy", &["json"])
            .blocking_save_file()
        else {
            return Ok(None);
        };
        let path = file.into_path().map_err(|_| "native-dialog-path")?;
        host.write_policy_export(&export_ref, &path).map(Some)
    })
    .await
    .map_err(|_| "native-task-failed")?
}
#[tauri::command]
fn close_desktop_without_suspend(window: tauri::WebviewWindow) -> Result<(), String> {
    window.destroy().map_err(|_| "window-close".into())
}
#[tauri::command]
fn open_help(state: tauri::State<'_, Arc<Host>>) -> Result<(), String> {
    let path = state.directory.join("help.html");
    std::fs::write(&path, include_str!("../resources/help.html")).map_err(|_| "help-document")?;
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "shell32")]
        unsafe extern "system" {
            fn ShellExecuteW(
                window: *mut std::ffi::c_void,
                operation: *const u16,
                file: *const u16,
                parameters: *const u16,
                directory: *const u16,
                show: i32,
            ) -> *mut std::ffi::c_void;
        }
        let operation: Vec<u16> = "open\0".encode_utf16().collect();
        let target: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                operation.as_ptr(),
                target.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        } as isize;
        if result <= 32 {
            return Err("help-browser-unavailable".into());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("help-platform-not-validated".into())
    }
}
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                fit_initial_window(&window)?;
            }
            let directory = Host::data_directory(app.path().app_local_data_dir()?.join("Core"))
                .map_err(std::io::Error::other)?;
            app.manage(Arc::new(
                Host::new(directory).map_err(std::io::Error::other)?,
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            connect_daemon,
            observe_daemon_startup,
            native_qa_window_metrics,
            desktop_request,
            desktop_poll,
            choose_document,
            choose_policy_import,
            save_policy_export,
            editor_request,
            close_desktop,
            close_desktop_without_suspend,
            open_help
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.emit("desktop-close-requested", ());
            }
        })
        .run(tauri::generate_context!())
        .expect("Desktop initialization failed");
}

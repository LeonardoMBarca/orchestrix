fn main() {
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "connect_daemon",
            "observe_daemon_startup",
            "native_qa_window_metrics",
            "desktop_request",
            "desktop_poll",
            "choose_document",
            "choose_policy_import",
            "save_policy_export",
            "editor_request",
            "close_desktop",
            "close_desktop_without_suspend",
            "open_help",
        ]));
    tauri_build::try_build(attributes).expect("Desktop capability generation failed");
}

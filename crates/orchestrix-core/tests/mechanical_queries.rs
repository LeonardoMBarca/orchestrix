use orchestrix_core::mechanical::{Query, QueryKind, execute};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "orchestrix-mechanical-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        assert_eq!(
            self.0.parent(),
            Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
        );
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn explicit_local_queries_need_authority_and_stay_inside_the_directory() {
    let directory = Directory::new();
    fs::write(directory.0.join("document.md"), "authorized local document").unwrap();
    let mut query = Query {
        authorized: false,
        authorization_ref: "manual-authority".into(),
        query: QueryKind::FileExists {
            authorized_directory: directory.0.clone(),
            path: "document.md".into(),
        },
    };
    assert!(execute(None, &query).is_err());
    query.authorized = true;
    let result = execute(None, &query).unwrap();
    assert_eq!(result["exists"], true);
    assert_eq!(result["modelCalled"], false);
    query.query = QueryKind::FileExists {
        authorized_directory: directory.0.clone(),
        path: "../escape.md".into(),
    };
    assert!(execute(None, &query).is_err());
    query.query = QueryKind::FileExists {
        authorized_directory: directory.0.clone(),
        path: "absent.md".into(),
    };
    assert_eq!(execute(None, &query).unwrap()["exists"], false);
}
#[test]
fn cli_help_has_no_database_or_provider_side_effect_and_missing_status_does_not_create_a_database()
{
    let directory = Directory::new();
    let binary = env!("CARGO_BIN_EXE_orchestrix");
    let help = std::process::Command::new(binary)
        .arg("help")
        .current_dir(&directory.0)
        .output()
        .unwrap();
    assert!(help.status.success());
    assert!(
        String::from_utf8(help.stdout)
            .unwrap()
            .contains("--allow-live yes")
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
    let missing = directory.0.join("absent.sqlite");
    let output = std::process::Command::new(binary)
        .args(["status", "--database"])
        .arg(&missing)
        .args(["--id", "absent"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!missing.exists());
}

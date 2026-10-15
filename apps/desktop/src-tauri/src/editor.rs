//! Host-only, session-scoped file grants. Renderer paths never confer authority.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
};

const MAX_FILE: u64 = 1_048_576;
const MAX_ENTRIES: usize = 1000;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Grant {
    pub id: String,
    pub name: String,
    pub directory: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub relative_path: String,
    pub directory: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: String,
    pub name: String,
    pub text: String,
    pub version: String,
    pub origin: &'static str,
    pub authorized_path: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum EditorRequest {
    Tree {
        grant: String,
        relative: String,
    },
    Open {
        grant: String,
        relative: String,
    },
    Save {
        document: String,
        base: String,
        text: String,
    },
    Create {
        grant: String,
        name: String,
    },
    Release {
        document: String,
    },
}
struct Allowed {
    path: PathBuf,
    directory: bool,
}
struct Opened {
    path: PathBuf,
    grant: String,
}
#[derive(Default)]
pub struct Editor {
    grants: HashMap<String, Allowed>,
    documents: HashMap<String, Opened>,
}

fn fail(code: &'static str) -> String {
    code.into()
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn token() -> Result<String, String> {
    #[cfg(windows)]
    {
        #[link(name = "bcrypt")]
        unsafe extern "system" {
            fn BCryptGenRandom(
                algorithm: *mut std::ffi::c_void,
                buffer: *mut u8,
                length: u32,
                flags: u32,
            ) -> i32;
        }
        let mut bytes = [0u8; 24];
        // System-preferred CSPRNG; no credential material or renderer IDs involved.
        if unsafe { BCryptGenRandom(std::ptr::null_mut(), bytes.as_mut_ptr(), 24, 2) } < 0 {
            return Err(fail("grant-random"));
        }
        Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
    }
    #[cfg(not(windows))]
    {
        Err(fail("editor-platform-not-validated"))
    }
}
fn native_reparse(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}
fn ordinary(path: &Path) -> Result<fs::Metadata, String> {
    let meta =
        fs::symlink_metadata(path).map_err(|_| fail("file-unavailable-or-permission-denied"))?;
    if native_reparse(&meta) {
        return Err(fail("reparse-point-not-authorized"));
    }
    Ok(meta)
}
fn relative(path: &str) -> Result<PathBuf, String> {
    let path = Path::new(path);
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
        || path.to_string_lossy().contains(':')
    {
        return Err(fail("relative-path-required"));
    }
    Ok(path.to_path_buf())
}
fn locked(path: &Path, write: bool) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true).write(write);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1);
    }
    options
        .open(path)
        .map_err(|_| fail("file-unavailable-or-permission-denied"))
}
fn read(file: &mut File) -> Result<(String, String), String> {
    if file.metadata().map_err(|_| fail("file-metadata"))?.len() > MAX_FILE {
        return Err(fail("file-size-limit"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| fail("file-read"))?;
    if bytes.len() as u64 > MAX_FILE || bytes.contains(&0) {
        return Err(fail("text-file-required"));
    }
    let version = hash(&bytes);
    Ok((
        String::from_utf8(bytes).map_err(|_| fail("utf8-text-required"))?,
        version,
    ))
}

impl Editor {
    /// Called only with the successful result of a native dialog on the host.
    pub fn grant_dialog_selection(&mut self, selected: PathBuf) -> Result<Grant, String> {
        let meta = ordinary(&selected)?;
        if !meta.is_file() && !meta.is_dir() {
            return Err(fail("ordinary-file-required"));
        }
        let path = selected
            .canonicalize()
            .map_err(|_| fail("file-unavailable-or-permission-denied"))?;
        let grant = Grant {
            id: token()?,
            name: selected
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
            directory: meta.is_dir(),
        };
        self.grants.insert(
            grant.id.clone(),
            Allowed {
                path,
                directory: grant.directory,
            },
        );
        Ok(grant)
    }
    pub fn directory(&self, id: &str) -> Result<PathBuf, String> {
        let grant = self.grants.get(id).ok_or_else(|| fail("grant-required"))?;
        if !grant.directory || !ordinary(&grant.path)?.is_dir() {
            return Err(fail("directory-grant-required"));
        }
        Ok(grant.path.clone())
    }
    pub fn file(&self, id: &str) -> Result<PathBuf, String> {
        let grant = self.grants.get(id).ok_or_else(|| fail("grant-required"))?;
        if grant.directory || !ordinary(&grant.path)?.is_file() {
            return Err(fail("file-grant-required"));
        }
        Ok(grant.path.clone())
    }
    pub fn context_file(&self, id: &str, child: &str) -> Result<PathBuf, String> {
        let path = self.resolve(id, child)?;
        if !ordinary(&path)?.is_file() {
            return Err(fail("file-grant-required"));
        }
        Ok(path)
    }
    /// No renderer path parameter; identity originates from a previously granted open handle.
    pub fn document_path(&self, id: &str) -> Result<PathBuf, String> {
        self.documents
            .get(id)
            .map(|d| d.path.clone())
            .ok_or_else(|| fail("open-document-required"))
    }
    /// Resolve a possible mutation target from a native directory grant before
    /// create_new can change disk. This does not claim that creation succeeded.
    pub fn creation_path(&self, grant: &str, name: &str) -> Result<PathBuf, String> {
        let child = relative(name)?;
        if child.components().count() != 1 {
            return Err(fail("single-file-name-required"));
        }
        Ok(self.directory(grant)?.join(child))
    }
    fn resolve(&self, id: &str, child: &str) -> Result<PathBuf, String> {
        let grant = self.grants.get(id).ok_or_else(|| fail("grant-required"))?;
        let mut path = grant.path.clone();
        ordinary(&path)?;
        if !child.is_empty() {
            if !grant.directory {
                return Err(fail("file-grant-has-no-children"));
            }
            for part in relative(child)?.components() {
                path.push(part);
                ordinary(&path)?;
            }
        }
        let resolved = path
            .canonicalize()
            .map_err(|_| fail("file-unavailable-or-permission-denied"))?;
        if resolved != path {
            return Err(fail("file-path-changed"));
        }
        Ok(path)
    }
    pub fn execute(&mut self, request: EditorRequest) -> Result<serde_json::Value, String> {
        match request {
            EditorRequest::Tree {
                grant,
                relative: child,
            } => {
                let root = self.resolve(&grant, &child)?;
                if !root.is_dir() {
                    return Err(fail("directory-grant-required"));
                }
                let mut entries = Vec::new();
                for item in fs::read_dir(root)
                    .map_err(|_| fail("directory-unavailable-or-permission-denied"))?
                    .take(MAX_ENTRIES + 1)
                {
                    if entries.len() == MAX_ENTRIES {
                        return Err(fail("directory-entry-limit"));
                    }
                    let item = item.map_err(|_| fail("directory-read"))?;
                    let meta =
                        fs::symlink_metadata(item.path()).map_err(|_| fail("directory-read"))?;
                    if native_reparse(&meta) || (!meta.is_dir() && !meta.is_file()) {
                        continue;
                    }
                    let name = item.file_name().to_string_lossy().into_owned();
                    if name == ".git" {
                        continue;
                    }
                    entries.push(Entry {
                        relative_path: if child.is_empty() {
                            name.clone()
                        } else {
                            format!("{child}/{name}")
                        },
                        name,
                        directory: meta.is_dir(),
                    });
                }
                entries.sort_by(|a, b| {
                    b.directory
                        .cmp(&a.directory)
                        .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                });
                serde_json::to_value(entries).map_err(|_| fail("editor-schema"))
            }
            EditorRequest::Open { grant, relative } => {
                let path = self.resolve(&grant, &relative)?;
                if !ordinary(&path)?.is_file() {
                    return Err(fail("text-file-required"));
                }
                let mut file = locked(&path, false)?;
                verify_handle(&file, &path)?;
                let (text, version) = read(&mut file)?;
                let id = token()?;
                let result = Document {
                    id: id.clone(),
                    name: path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into(),
                    text,
                    version,
                    origin: "authorized-file",
                    authorized_path: path.to_string_lossy().into(),
                };
                self.documents.insert(id, Opened { path, grant });
                serde_json::to_value(result).map_err(|_| fail("editor-schema"))
            }
            EditorRequest::Save {
                document,
                base,
                text,
            } => {
                if text.len() as u64 > MAX_FILE || text.contains('\0') {
                    return Err(fail("file-size-or-text-limit"));
                }
                let opened = self
                    .documents
                    .get(&document)
                    .ok_or_else(|| fail("open-document-required"))?;
                let allowed = self
                    .grants
                    .get(&opened.grant)
                    .ok_or_else(|| fail("grant-required"))?;
                if !opened.path.starts_with(&allowed.path) {
                    return Err(fail("grant-required"));
                }
                ordinary(&opened.path)?;
                // On Windows the open handle denies other writers and renames throughout
                // compare + write. A watcher or an earlier path hash is insufficient.
                let mut file = locked(&opened.path, true)?;
                verify_handle(&file, &opened.path)?;
                let (_, current) = read(&mut file)?;
                if current != base {
                    return Err(fail("save-conflict-external-change"));
                }
                file.seek(SeekFrom::Start(0))
                    .map_err(|_| fail("file-write"))?;
                file.write_all(text.as_bytes())
                    .map_err(|_| fail("file-write-buffer-retained"))?;
                file.set_len(text.len() as u64)
                    .map_err(|_| fail("file-write-buffer-retained"))?;
                file.sync_all()
                    .map_err(|_| fail("file-write-buffer-retained"))?;
                Ok(serde_json::json!({"version": hash(text.as_bytes())}))
            }
            EditorRequest::Create { grant, name } => {
                let path = self.creation_path(&grant, &name)?;
                let root = path
                    .parent()
                    .ok_or_else(|| fail("directory-grant-required"))?;
                let _directory = pin_directory(root)?;
                let mut options = OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(windows)]
                {
                    use std::os::windows::fs::OpenOptionsExt;
                    options.share_mode(1);
                }
                let file = options
                    .open(&path)
                    .map_err(|_| fail("create-file-exists-or-permission-denied"))?;
                verify_handle(&file, &path)?;
                file.sync_all().map_err(|_| fail("file-write"))?;
                drop(file);
                self.execute(EditorRequest::Open {
                    grant,
                    relative: name,
                })
            }
            EditorRequest::Release { document } => {
                self.documents.remove(&document);
                Ok(serde_json::Value::Null)
            }
        }
    }
}

#[cfg(windows)]
fn verify_handle(file: &File, expected: &Path) -> Result<(), String> {
    use std::os::windows::{ffi::OsStringExt, io::AsRawHandle};
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFinalPathNameByHandleW(
            file: *mut std::ffi::c_void,
            path: *mut u16,
            size: u32,
            flags: u32,
        ) -> u32;
    }
    let mut buffer = vec![0u16; 32768];
    let length = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle(),
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            0,
        )
    };
    if length == 0 || length as usize >= buffer.len() {
        return Err(fail("file-handle-identity"));
    }
    let actual = PathBuf::from(std::ffi::OsString::from_wide(&buffer[..length as usize]));
    if actual != expected {
        return Err(fail("file-path-changed"));
    }
    Ok(())
}
#[cfg(not(windows))]
fn verify_handle(_: &File, _: &Path) -> Result<(), String> {
    Err(fail("editor-platform-not-validated"))
}
fn pin_directory(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1).custom_flags(0x02000000);
    }
    let file = options
        .open(path)
        .map_err(|_| fail("directory-unavailable-or-permission-denied"))?;
    verify_handle(&file, path)?;
    Ok(file)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn grants_are_explicit_and_external_writes_do_not_get_overwritten() {
        let root = std::env::temp_dir().join(format!("orchestrix-editor-{}", token().unwrap()));
        fs::create_dir(&root).unwrap();
        fs::write(root.join(".env"), "SECRET=local-only\n").unwrap();
        let mut editor = Editor::default();
        assert!(
            editor
                .execute(EditorRequest::Open {
                    grant: root.to_string_lossy().into(),
                    relative: ".env".into()
                })
                .is_err()
        );
        let grant = editor.grant_dialog_selection(root.clone()).unwrap();
        assert!(
            editor
                .execute(EditorRequest::Open {
                    grant: grant.id.clone(),
                    relative: "../outside".into()
                })
                .is_err()
        );
        let document = editor
            .execute(EditorRequest::Open {
                grant: grant.id.clone(),
                relative: ".env".into(),
            })
            .unwrap();
        fs::write(root.join(".env"), "SECRET=external\n").unwrap();
        let request = EditorRequest::Save {
            document: document["id"].as_str().unwrap().into(),
            base: document["version"].as_str().unwrap().into(),
            text: "SECRET=user-buffer\n".into(),
        };
        assert_eq!(
            editor.execute(request).unwrap_err(),
            "save-conflict-external-change"
        );
        assert_eq!(
            fs::read_to_string(root.join(".env")).unwrap(),
            "SECRET=external\n"
        );
        let created = editor
            .execute(EditorRequest::Create {
                grant: grant.id.clone(),
                name: "notes.md".into(),
            })
            .unwrap();
        assert_eq!(created["text"], "");
        assert!(
            editor
                .execute(EditorRequest::Create {
                    grant: grant.id,
                    name: "notes.md".into()
                })
                .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn save_locks_identity_and_retains_text_on_permission_denial() {
        let root = std::env::temp_dir().join(format!("orchestrix-editor-{}", token().unwrap()));
        fs::create_dir(&root).unwrap();
        let path = root.join("a.txt");
        fs::write(&path, "before").unwrap();
        let mut editor = Editor::default();
        let grant = editor.grant_dialog_selection(path.clone()).unwrap();
        let doc = editor
            .execute(EditorRequest::Open {
                grant: grant.id,
                relative: String::new(),
            })
            .unwrap();
        let _lock = locked(&path, true).unwrap();
        assert!(
            editor
                .execute(EditorRequest::Save {
                    document: doc["id"].as_str().unwrap().into(),
                    base: doc["version"].as_str().unwrap().into(),
                    text: "buffer survives".into()
                })
                .is_err()
        );
        drop(_lock);
        let saved = editor
            .execute(EditorRequest::Save {
                document: doc["id"].as_str().unwrap().into(),
                base: doc["version"].as_str().unwrap().into(),
                text: "after".into(),
            })
            .unwrap();
        assert_eq!(saved["version"], hash(b"after"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "after");
        fs::remove_dir_all(root).unwrap();
    }
}

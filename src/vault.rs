use crate::i18n::t;
use keepass::{
    Database, DatabaseKey,
    config::DatabaseVersion,
    db::{EntryId, EntryMut},
};
use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;

pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone)]
pub struct Vault {
    pub db: Database,
    pub path: PathBuf,
    key: DatabaseKey,
    disk: Option<Vec<u8>>,
}

#[derive(Default)]
pub struct Draft {
    pub id: Option<EntryId>,
    pub token: bool,
    pub title: String,
    pub username: String,
    pub secret: Zeroizing<String>,
    pub url: String,
    pub notes: Zeroizing<String>,
}

impl Draft {
    pub fn apply(&self, entry: &mut EntryMut<'_>) {
        entry.set_unprotected("Title", self.title.trim());
        entry.set_unprotected("UserName", &self.username);
        entry.set_protected("Password", self.secret.as_str());
        entry.set_unprotected("URL", &self.url);
        entry.set_protected("Notes", self.notes.as_str());
        entry.set_unprotected("MyPass.Type", if self.token { "token" } else { "account" });
    }
}

impl Vault {
    pub fn create(path: PathBuf, password: &str) -> Result<Self> {
        if password.chars().count() < 12 {
            return Err(t("主密码至少需要 12 个字符，建议使用容易记住的长短语。").into());
        }
        let mut db = Database::new();
        let version = match db.config.kdf_config {
            keepass::config::KdfConfig::Argon2 { version, .. } => version,
            _ => unreachable!(),
        };
        db.config.kdf_config = keepass::config::KdfConfig::Argon2id {
            memory: 64 * 1024 * 1024,
            iterations: 3,
            parallelism: 1,
            version,
        };
        db.root_mut().name = "MyPass".into();
        let mut vault = Self {
            db,
            path,
            key: DatabaseKey::new().with_password(password),
            disk: None,
        };
        vault.save()?;
        Ok(vault)
    }

    pub fn open(path: PathBuf, password: &str) -> Result<Self> {
        let disk = fs::read(&path).map_err(|e| format!("{}{}", t("无法读取密码库："), e))?;
        let key = DatabaseKey::new().with_password(password);
        let mut db = Database::open(&mut Cursor::new(&disk), key.clone()).map_err(|_| {
            t("无法解锁：请检查主密码。首版仅支持使用主密码的 KDBX4 密码库。").to_string()
        })?;
        if !matches!(db.config.version, DatabaseVersion::KDB4(0 | 1)) {
            return Err(t("首版仅支持 KDBX4；请在 KeePassXC 中将副本另存为 KDBX4 后打开。").into());
        }
        // The writer emits KDBX 4.1; KeePassXC may write a 4.0 header.
        db.config.version = DatabaseVersion::KDB4(1);
        Ok(Self {
            db,
            path,
            key,
            disk: Some(disk),
        })
    }

    pub fn save(&mut self) -> Result<()> {
        let mut bytes = Vec::new();
        self.db
            .save(&mut bytes, self.key.clone())
            .map_err(|e| format!("{}{}", t("加密失败："), e))?;
        // Verify the encrypted output before touching the existing file.
        Database::open(&mut Cursor::new(&bytes), self.key.clone())
            .map_err(|_| t("保存验证失败，原文件未修改。").to_string())?;
        if let Some(previous) = &self.disk {
            let current =
                fs::read(&self.path).map_err(|e| format!("{}{}", t("无法读取原文件："), e))?;
            if &current != previous {
                return Err(
                    t("密码库已被其他程序修改。请取消编辑并重新打开，以免覆盖更改。").into(),
                );
            }
            let backup = PathBuf::from(format!("{}.bak", self.path.display()));
            atomic_write(&backup, previous, false)?;
        } else if self.path.exists() {
            return Err(t("这个文件已存在，请选择新的文件名。").into());
        }
        atomic_write(&self.path, &bytes, self.disk.is_none())?;
        self.disk = Some(bytes);
        Ok(())
    }

    pub fn put(&mut self, draft: &Draft) -> Result<()> {
        if draft.title.trim().is_empty() || draft.secret.is_empty() {
            return Err(t("请填写名称和密码 / Token。").into());
        }
        if let Some(id) = draft.id {
            let mut entry = self.db.entry_mut(id).ok_or(t("该条目不存在。"))?;
            let mut tracked = entry.track_changes();
            draft.apply(&mut tracked.as_mut());
        } else {
            draft.apply(&mut self.db.root_mut().add_entry());
        }
        self.save()
    }

    pub fn delete(&mut self, id: EntryId) -> Result<()> {
        self.db
            .entry_mut(id)
            .ok_or(t("该条目不存在。"))?
            .track_changes()
            .remove();
        self.save()
    }

    pub fn draft(&self, id: EntryId) -> Option<Draft> {
        let e = self.db.entry(id)?;
        Some(Draft {
            id: Some(id),
            token: e.get("MyPass.Type") == Some("token"),
            title: e.get_title().unwrap_or_default().into(),
            username: e.get_username().unwrap_or_default().into(),
            secret: Zeroizing::new(e.get_password().unwrap_or_default().into()),
            url: e.get_url().unwrap_or_default().into(),
            notes: Zeroizing::new(e.get("Notes").unwrap_or_default().into()),
        })
    }

    #[cfg(debug_assertions)]
    pub fn preview() -> Self {
        let mut db = Database::new();
        for (title, username, token) in [
            ("GitHub", "demo@example.com", false),
            ("Google", "demo@example.com", false),
            ("OpenAI API", "", true),
            ("Steam", "demo_user", false),
            ("GitHub API", "", true),
        ] {
            Draft {
                title: title.into(),
                username: username.into(),
                token,
                secret: Zeroizing::new("fictional-preview-only".into()),
                ..Default::default()
            }
            .apply(&mut db.root_mut().add_entry());
        }
        Self {
            db,
            path: PathBuf::new(),
            key: DatabaseKey::new().with_password("fictional-preview-only"),
            disk: None,
        }
    }
}

fn atomic_write(path: &Path, bytes: &[u8], new: bool) -> Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| format!("{}{}", t("无法创建临时文件："), e))?;
    temp.write_all(bytes)
        .and_then(|_| temp.as_file().sync_all())
        .map_err(|e| format!("{}{}", t("写入失败："), e))?;
    let result = if new {
        temp.persist_noclobber(path)
    } else {
        temp.persist(path)
    };
    result.map_err(|e| format!("{}{}", t("无法保存文件："), e.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const PASS: &str = "test-only master password 123";

    #[test]
    fn encrypted_roundtrip_edit_delete_wrong_key_and_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.kdbx");
        let mut vault = Vault::create(path.clone(), PASS).unwrap();
        let mut draft = Draft {
            title: "测试 API".into(),
            token: true,
            secret: Zeroizing::new("test-token-never-real-123".into()),
            ..Default::default()
        };
        vault.put(&draft).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert!(
            !bytes
                .windows(draft.secret.len())
                .any(|w| w == draft.secret.as_bytes())
        );
        assert!(Vault::open(path.clone(), "wrong password").is_err());
        let mut reopened = Vault::open(path.clone(), PASS).unwrap();
        let id = reopened.db.iter_all_entries().next().unwrap().id();
        assert_eq!(
            reopened.draft(id).unwrap().secret.as_str(),
            draft.secret.as_str()
        );
        assert!(reopened.draft(id).unwrap().token);
        draft.id = Some(id);
        draft.title = "更新名称".into();
        reopened.put(&draft).unwrap();
        assert_eq!(
            Vault::open(path.clone(), PASS)
                .unwrap()
                .draft(id)
                .unwrap()
                .title,
            "更新名称"
        );
        assert!(Vault::open(PathBuf::from(format!("{}.bak", path.display())), PASS).is_ok());
        reopened.delete(id).unwrap();
        assert_eq!(Vault::open(path, PASS).unwrap().db.num_entries(), 0);
    }

    #[test]
    fn rejects_overwrite_and_detects_external_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.kdbx");
        let mut vault = Vault::create(path.clone(), PASS).unwrap();
        assert!(Vault::create(path.clone(), PASS).is_err());
        fs::write(&path, b"external change").unwrap();
        assert!(vault.save().is_err());
        assert_eq!(fs::read(path).unwrap(), b"external change");
    }

    #[test]
    fn account_fields_and_history_survive_updates() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("account.kdbx");
        let mut vault = Vault::create(path.clone(), PASS).unwrap();
        vault
            .put(&Draft {
                title: "示例账号".into(),
                username: "demo@example.com".into(),
                secret: Zeroizing::new("test-password-123".into()),
                url: "https://example.com".into(),
                notes: Zeroizing::new("测试备注".into()),
                ..Default::default()
            })
            .unwrap();
        let id = vault.db.iter_all_entries().next().unwrap().id();
        let mut draft = vault.draft(id).unwrap();
        assert!(!draft.token);
        assert_eq!(draft.username, "demo@example.com");
        assert_eq!(draft.url, "https://example.com");
        assert_eq!(draft.notes.as_str(), "测试备注");
        *draft.secret = "updated-test-password".into();
        vault.put(&draft).unwrap();
        let reopened = Vault::open(path, PASS).unwrap();
        assert_eq!(
            reopened.draft(id).unwrap().secret.as_str(),
            "updated-test-password"
        );
        assert_eq!(
            reopened
                .db
                .entry(id)
                .unwrap()
                .history
                .as_ref()
                .unwrap()
                .get_entries()
                .len(),
            1
        );
    }

    #[test]
    #[ignore = "requires an external KeePassXC CLI via KEEPASSXC_CLI"]
    fn keepassxc_reads_and_edits_our_database() {
        use std::{
            io::Write,
            process::{Command, Stdio},
        };
        let cli = PathBuf::from(
            std::env::var_os("KEEPASSXC_CLI")
                .expect("Set KEEPASSXC_CLI to the path of an external keepassxc-cli.exe"),
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("interop.kdbx");
        let mut vault = Vault::create(path.clone(), PASS).unwrap();
        vault
            .put(&Draft {
                title: "Interop".into(),
                token: true,
                secret: Zeroizing::new("fake-token-for-compatibility-only".into()),
                ..Default::default()
            })
            .unwrap();
        let mut child = Command::new(&cli)
            .args(["show", "-q", "-s", "-a", "Password"])
            .arg(&path)
            .arg("Interop")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        writeln!(child.stdin.take().unwrap(), "{PASS}").unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("fake-token-for-compatibility-only")
        );
        let mut child = Command::new(&cli)
            .args(["edit", "-q", "-u", "changed-by-keepassxc"])
            .arg(&path)
            .arg("Interop")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        writeln!(child.stdin.take().unwrap(), "{PASS}").unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut reopened = Vault::open(path, PASS).unwrap();
        let id = reopened.db.iter_all_entries().next().unwrap().id();
        let draft = reopened.draft(id).unwrap();
        assert_eq!(draft.username, "changed-by-keepassxc");
        assert!(draft.token);
        reopened.put(&draft).unwrap();
    }
}

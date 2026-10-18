use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    English,
    Chinese,
}

impl Language {
    fn from_tag(tag: &str) -> Self {
        if tag
            .split(['-', '_'])
            .next()
            .is_some_and(|s| s.eq_ignore_ascii_case("zh"))
        {
            Self::Chinese
        } else {
            Self::English
        }
    }
}

static LANGUAGE: OnceLock<Language> = OnceLock::new();

pub fn init() {
    LANGUAGE.get_or_init(|| {
        #[cfg(debug_assertions)]
        if let Some(tag) =
            std::env::args().find_map(|a| a.strip_prefix("--language=").map(str::to_owned))
        {
            return Language::from_tag(&tag);
        }
        Language::from_tag(&system_language())
    });
}

#[cfg(windows)]
fn system_language() -> String {
    use windows_sys::Win32::Globalization::{
        GetUserDefaultUILanguage, GetUserPreferredUILanguages, MUI_LANGUAGE_NAME,
    };
    let mut count = 0;
    let mut size = 0;
    // Windows returns an ordered UTF-16 multi-string, including its terminating NULs.
    // First query the required capacity; then pass a buffer of exactly that capacity.
    unsafe {
        if GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME,
            &mut count,
            std::ptr::null_mut(),
            &mut size,
        ) != 0
            && size > 1
        {
            let mut buffer = vec![0u16; size as usize];
            if GetUserPreferredUILanguages(
                MUI_LANGUAGE_NAME,
                &mut count,
                buffer.as_mut_ptr(),
                &mut size,
            ) != 0
            {
                let end = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
                if end > 0 {
                    return String::from_utf16_lossy(&buffer[..end]);
                }
            }
        }
        // Use the UI language ID, never the region/date-format locale, as fallback.
        if GetUserDefaultUILanguage() & 0x03ff == 0x04 {
            "zh".into()
        } else {
            "en".into()
        }
    }
}

#[cfg(not(windows))]
fn system_language() -> String {
    "en".into()
}

pub fn t(source: &'static str) -> &'static str {
    init();
    translate(*LANGUAGE.get().unwrap(), source)
}

fn translate(language: Language, source: &'static str) -> &'static str {
    if language == Language::Chinese {
        return source;
    }
    english(source)
}

// UI messages live here; database field names and user-entered content are never translated.
fn english(source: &'static str) -> &'static str {
    match source {
        "当前是界面预览，不会保存示例数据。" => {
            "This is a preview. Sample data will not be saved."
        }
        "操作未完成，请重试。" => "The operation did not finish. Please try again.",
        "已保存" => "Saved",
        "无法访问剪贴板" => "Cannot access the clipboard.",
        "复制失败，请重试" => "Copy failed. Please try again.",
        "已复制 · 30 秒后清除剪贴板" => "Copied · Clipboard clears in 30 seconds",
        "KeePass 密码库" => "KeePass database",
        "创建你的密码库" => "Create your vault",
        "欢迎回来" => "Welcome back",
        "密码和 Token，放在一起" => "Passwords and tokens, together",
        "保存在你的电脑上，由主密码解锁" => {
            "Stored on your PC. Unlocked with your master password."
        }
        "主密码" => "Master password",
        "再次输入主密码" => "Confirm master password",
        "至少 12 个字符。忘记后无法找回，请妥善记住。" => {
            "Use at least 12 characters. Keep it safe; it cannot be recovered."
        }
        "显示主密码" => "Show master password",
        "创建密码库" => "Create vault",
        "解锁" => "Unlock",
        "两次输入的主密码不一致。" => "The master passwords do not match.",
        "请输入主密码。" => "Enter your master password.",
        "创建我的密码库" => "Create my vault",
        "打开密码库" => "Open vault",
        "新建密码库" => "New vault",
        "搜索名称、账号或网址" => "Search names, usernames or websites",
        "＋ 添加" => "+ Add",
        "未命名" => "Untitled",
        "添加第一个账号或 Token" => "Add your first account or token",
        "没有找到匹配项" => "No matches found",
        "点击右上角「添加」，下次取用只需一键复制。" => {
            "Click Add above. Next time, copy it with one click."
        }
        "试试名称、用户名或网址。" => "Try a name, username or website.",
        "账号密码" => "Account",
        "查看或编辑" => "View or edit",
        "复制 Token" => "Copy token",
        "复制密码" => "Copy password",
        "此条目尚未设置密码。" => "This entry does not have a password yet.",
        "查看 / 编辑" => "View / Edit",
        "复制用户名" => "Copy username",
        "打开网站" => "Open website",
        "请在编辑中填写以 https:// 或 http:// 开头的网址。" => {
            "Edit this entry and add a website starting with https:// or http://."
        }
        "删除" => "Delete",
        "编辑条目" => "Edit entry",
        "添加条目" => "Add entry",
        "名称" => "Name",
        "用户名" => "Username",
        "密码" => "Password",
        "显示内容" => "Show secret",
        "生成密码" => "Generate password",
        "网址（可选）" => "Website (optional)",
        "备注（可选）" => "Notes (optional)",
        "保存" => "Save",
        "取消" => "Cancel",
        "请填写名称和密码 / Token。" => "Enter a name and a password or token.",
        "正在加密处理，请稍候…" => "Encrypting, please wait…",
        "界面预览 · 示例数据，不保存" => "Preview · Sample data, not saved",
        "● 已保存" => "● Saved",
        "本地加密密码库" => "Local encrypted vault",
        "锁定" => "Lock",
        "设置" => "Settings",
        "删除条目？" => "Delete entry?",
        "此条目将从密码库删除。保存前会备份原文件。" => {
            "This entry will be deleted. The previous file will be backed up."
        }
        "确认删除" => "Delete entry",
        "5 分钟无操作后自动锁定" => "Automatically lock after 5 minutes of inactivity",
        "复制后 30 秒自动清除剪贴板" => "Clear the clipboard 30 seconds after copying",
        "每次保存前保留上一版 .kdbx.bak 备份" => {
            "Keep the previous version as a .kdbx.bak backup"
        }
        "密码库位置" => "Vault location",
        "请另行备份密码库和主密码。首版尚未经过独立安全审计。" => {
            "Keep separate backups of your vault and master password. This first release has not been independently security-audited."
        }
        "锁定并切换密码库" => "Lock and switch vault",
        "主密码至少需要 12 个字符，建议使用容易记住的长短语。" => {
            "Use at least 12 characters for your master password. A memorable long passphrase works well."
        }
        "无法解锁：请检查主密码。首版仅支持使用主密码的 KDBX4 密码库。" => {
            "Cannot unlock. Check your master password. This version supports password-only KDBX4 vaults."
        }
        "首版仅支持 KDBX4；请在 KeePassXC 中将副本另存为 KDBX4 后打开。" => {
            "Only KDBX4 is supported. Use KeePassXC to save a copy in KDBX4 format first."
        }
        "保存验证失败，原文件未修改。" => {
            "Save verification failed. The original file was not changed."
        }
        "密码库已被其他程序修改。请取消编辑并重新打开，以免覆盖更改。" => {
            "Another app changed this vault. Cancel editing and reopen it to avoid overwriting changes."
        }
        "这个文件已存在，请选择新的文件名。" => {
            "This file already exists. Choose a new filename."
        }
        "该条目不存在。" => "This entry no longer exists.",
        "无法读取密码库：" => "Cannot read vault: ",
        "加密失败：" => "Encryption failed: ",
        "无法读取原文件：" => "Cannot read original file: ",
        "无法创建临时文件：" => "Cannot create temporary file: ",
        "写入失败：" => "Write failed: ",
        "无法保存文件：" => "Cannot save file: ",
        _ => source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_ui_language_and_falls_back_to_english() {
        for tag in ["zh", "zh-CN", "zh-TW", "zh-Hans-CN", "ZH_hk"] {
            assert_eq!(Language::from_tag(tag), Language::Chinese);
        }
        for tag in ["en-US", "en-CA", "fr-CA", "ja-JP", "", "zho"] {
            assert_eq!(Language::from_tag(tag), Language::English);
        }
    }

    #[test]
    fn translates_labels_and_preserves_user_text() {
        assert_eq!(translate(Language::English, "保存"), "Save");
        assert_eq!(translate(Language::Chinese, "保存"), "保存");
        assert_eq!(
            translate(Language::English, "My custom account"),
            "My custom account"
        );
    }

    #[test]
    fn all_app_messages_have_english_translations() {
        for source in [include_str!("main.rs"), include_str!("vault.rs")] {
            let production = source.split("#[cfg(test)]").next().unwrap();
            for (_, tail) in production
                .match_indices("t(\"")
                .map(|(i, _)| (i, &production[i + 3..]))
            {
                let message = tail.split('"').next().unwrap();
                if message
                    .chars()
                    .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
                {
                    assert_ne!(english(message), message, "Missing translation: {message}");
                }
            }
        }
    }

    #[test]
    fn reads_windows_display_language() {
        let tag = system_language();
        assert!(!tag.is_empty());
        println!(
            "Windows UI language: {tag}; MyPass language: {:?}",
            Language::from_tag(&tag)
        );
    }
}

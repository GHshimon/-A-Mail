//! 外部 CLI(Codex)を起動するときの PATH。
//!
//! Finder / Dock から起動した .app が受け取る PATH は launchd 既定の
//! `/usr/bin:/bin:/usr/sbin:/sbin` で、Homebrew や npm の bin を含まない。
//! Codex CLI の実体は `#!/usr/bin/env node` で始まる JS なので、この PATH のままだと
//! `env: node: No such file or directory`(exit 127)で必ず失敗する
//! (ターミナルから `npm run tauri dev` した場合だけ動く、という食い違いの正体)。
//!
//! 実行するバイナリ自体は従来どおり絶対パスで解決する(PATH 探索はしない)。
//! ここで足すのは、その子プロセスがインタプリタを見つけるための既知の場所だけ。

use std::ffi::OsString;

/// Codex を起動する子プロセスへ渡す PATH。既知の場所を先頭に足す。
pub fn child_path() -> OsString {
    const EXTRA: [&str; 3] = ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"];
    let inherited = std::env::var("PATH").unwrap_or_default();
    let mut parts: Vec<&str> = Vec::new();
    for dir in EXTRA {
        parts.push(dir);
    }
    for dir in inherited.split(':').filter(|d| !d.is_empty()) {
        if !parts.contains(&dir) {
            parts.push(dir);
        }
    }
    OsString::from(parts.join(":"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn puts_known_interpreter_dirs_first_without_duplicates() {
        let path = child_path();
        let text = path.to_string_lossy().into_owned();
        assert!(text.starts_with("/opt/homebrew/bin:/usr/local/bin:/usr/bin"));
        // 継承分に同じディレクトリがあっても二重に並べない。
        let dirs: Vec<&str> = text.split(':').collect();
        let mut seen = std::collections::HashSet::new();
        for dir in dirs {
            assert!(seen.insert(dir), "PATH に重複: {dir}");
        }
    }
}

//! AI が参照するローカル/NASプロジェクトの登録と、境界付きテキスト検索。
//!
//! 登録ルートへは一切書き込まず、検索時もシンボリックリンクを辿らない。

use std::collections::HashSet;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

use quick_xml::events::Event;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};
use crate::store::models::{AiProject, ProjectFileHit};

const MAX_FILE_BYTES: u64 = 1024 * 1024;
const MAX_DOCX_BYTES: u64 = 16 * 1024 * 1024;
const MAX_DOCX_XML_BYTES: u64 = 4 * 1024 * 1024;
const MAX_DOCX_ENTRIES: usize = 4096;
const MAX_DOCX_EXPANDED_BYTES: u64 = 64 * 1024 * 1024;
const MAX_VISITED_ENTRIES: usize = 10_000;
const MAX_SCANNED_FILES: usize = 2_000;
const MAX_TOTAL_BYTES: u64 = 16 * 1024 * 1024;
const MAX_SCAN_ELAPSED: Duration = Duration::from_secs(2);
const MAX_RESULTS: u32 = 50;
const MAX_SNIPPET_CHARS: usize = 500;

#[derive(Debug)]
pub struct ProjectSearchReport {
    pub hits: Vec<ProjectFileHit>,
    pub review_reasons: Vec<String>,
    pub truncated: bool,
}

#[derive(Debug)]
pub struct ExtractedProjectFile {
    pub text: String,
    /// SHA-256 of the exact source bytes, not just a matching excerpt.
    pub version: String,
    pub is_docx: bool,
}

fn is_docx(path: &Path) -> bool {
    path.extension()
        .and_then(|v| v.to_str())
        .is_some_and(|v| v.eq_ignore_ascii_case("docx"))
}

fn source_error(reason: &str) -> AppError {
    AppError::BadInput(format!("資料を読み取れません: {reason}"))
}

/// Open every directory component relative to a pinned descriptor. A symlink
/// swapped into an ancestor between validation and open cannot redirect reads.
#[cfg(unix)]
fn open_source_file(root: &Path, relative: &Path) -> AppResult<fs::File> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::OpenOptionsExt;

    let path = root.join(relative);
    let parts: Vec<_> = path
        .components()
        .filter_map(|part| match part {
            std::path::Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect();
    let mut current = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open("/")
        .map_err(|_| source_error("参照ルートを開けません"))?;
    for (index, name) in parts.iter().enumerate() {
        let name =
            CString::new(name.as_bytes()).map_err(|_| source_error("ファイル名が不正です"))?;
        let mut flags = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK;
        if index + 1 < parts.len() {
            flags |= libc::O_DIRECTORY;
        }
        // SAFETY: CString is NUL terminated and current owns a valid directory fd.
        let fd = unsafe { libc::openat(current.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(source_error("リンクまたは変更されたパスは参照できません"));
        }
        // SAFETY: openat returned a new owned fd; ownership is transferred once.
        current = unsafe { fs::File::from_raw_fd(fd) };
    }
    Ok(current)
}

#[cfg(not(unix))]
fn open_source_file(root: &Path, relative: &Path) -> AppResult<fs::File> {
    fs::File::open(root.join(relative)).map_err(|_| source_error("ファイルを開けません"))
}

/// Re-read only a registered root's regular file. No archive entry is extracted
/// to disk, and no XML relationships or external entities are followed.
pub fn read_project_source(
    project: &AiProject,
    relative_path: &str,
) -> AppResult<ExtractedProjectFile> {
    let relative = Path::new(relative_path);
    if !project.enabled
        || relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err(source_error("参照範囲が不正です"));
    }
    let root = Path::new(&project.root_path);
    let canonical_root = validate_project_root(&project.root_path)?;
    if canonical_root != root {
        return Err(source_error("登録したルートが変化しています"));
    }
    let mut path = root.to_path_buf();
    for part in relative.components() {
        path.push(part.as_os_str());
        let meta =
            fs::symlink_metadata(&path).map_err(|_| source_error("参照元が見つかりません"))?;
        if meta.file_type().is_symlink() || (meta.is_dir() && skip_directory(&path)) {
            return Err(source_error("リンク・非公開ディレクトリは参照できません"));
        }
    }
    if sensitive_file(&path) || (!allowed_text_file(&path) && !is_docx(&path)) {
        return Err(source_error("未対応形式または秘密情報ファイルです"));
    }
    let canonical = fs::canonicalize(&path).map_err(|_| source_error("パスを確認できません"))?;
    if canonical != path || !canonical.starts_with(&canonical_root) {
        return Err(source_error("登録した参照範囲の外です"));
    }
    let file = open_source_file(&canonical_root, relative)?;
    let meta = file
        .metadata()
        .map_err(|_| source_error("ファイルを確認できません"))?;
    let limit = if is_docx(&path) {
        MAX_DOCX_BYTES
    } else {
        MAX_FILE_BYTES
    };
    if !meta.is_file() || meta.len() > limit {
        return Err(source_error(
            "ファイル形式またはサイズ上限を確認してください",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let current =
            fs::symlink_metadata(&path).map_err(|_| source_error("読取中に資料が変化しました"))?;
        if current.file_type().is_symlink()
            || current.ino() != meta.ino()
            || current.dev() != meta.dev()
        {
            return Err(source_error("読取中に資料が差し替えられました"));
        }
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| source_error("本文を読み取れません"))?;
    if bytes.len() as u64 > limit || bytes.len() as u64 != meta.len() {
        return Err(source_error("読取上限超過または読取中の変更です"));
    }
    let version = format!("{:x}", Sha256::digest(&bytes));
    let docx = is_docx(&path);
    let text = if docx {
        extract_docx(&bytes)?
    } else {
        String::from_utf8(bytes).map_err(|_| source_error("UTF-8テキストではありません"))?
    };
    Ok(ExtractedProjectFile {
        text,
        version,
        is_docx: docx,
    })
}

fn extract_docx(bytes: &[u8]) -> AppResult<String> {
    // Limit the central-directory record count before the library allocates its
    // index. ZIP64/multi-disk are unnecessary for the bounded documents we read.
    if bytes.len() < 22 || bytes.len() as u64 > MAX_DOCX_BYTES {
        return Err(source_error("DOCXのサイズが不正です"));
    }
    let u16_at = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize;
    let start = bytes.len().saturating_sub(65_557);
    let end = (start..=bytes.len() - 22)
        .rev()
        .find(|&at| bytes[at..at + 4] == *b"PK\x05\x06" && at + 22 + u16_at(at + 20) == bytes.len())
        .ok_or_else(|| source_error("DOCXのZIP構造が不正です"))?;
    let count = u16_at(end + 10);
    if count == 0
        || count > MAX_DOCX_ENTRIES
        || u16_at(end + 4) != 0
        || u16_at(end + 6) != 0
        || u16_at(end + 8) != count
    {
        return Err(source_error("DOCXの項目数上限または分割形式です"));
    }
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| source_error("DOCXを開けません"))?;
    if archive.len() != count {
        return Err(source_error("DOCXの項目数が一致しません"));
    }
    let mut total = 0u64;
    let mut documents = 0;
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|_| source_error("DOCXの項目が不正です"))?;
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| source_error("DOCXの展開上限を超えています"))?;
        if total > MAX_DOCX_EXPANDED_BYTES {
            return Err(source_error("DOCXの展開上限を超えています"));
        }
        if entry.name() == "word/document.xml" {
            documents += 1;
        }
    }
    if documents != 1 {
        return Err(source_error("DOCX本文が欠落または重複しています"));
    }
    let entry = archive
        .by_name("word/document.xml")
        .map_err(|_| source_error("DOCX本文がありません"))?;
    if entry.size() > MAX_DOCX_XML_BYTES {
        return Err(source_error("DOCX本文の展開上限を超えています"));
    }
    let mut xml = Vec::new();
    entry
        .take(MAX_DOCX_XML_BYTES + 1)
        .read_to_end(&mut xml)
        .map_err(|_| source_error("DOCX本文を展開できません"))?;
    if xml.len() as u64 > MAX_DOCX_XML_BYTES {
        return Err(source_error("DOCX本文の展開上限を超えています"));
    }
    extract_document_xml(&xml)
}

fn extract_document_xml(xml: &[u8]) -> AppResult<String> {
    if xml.len() as u64 > MAX_DOCX_XML_BYTES {
        return Err(source_error("DOCX本文の読取上限を超えています"));
    }
    let text =
        std::str::from_utf8(xml).map_err(|_| source_error("DOCX本文の文字コードが未対応です"))?;
    let mut reader = quick_xml::Reader::from_str(text);
    let mut output = String::new();
    let mut in_text = false;
    let mut depth = 0usize;
    let started = Instant::now();
    loop {
        if started.elapsed() >= MAX_SCAN_ELAPSED || output.len() > MAX_DOCX_XML_BYTES as usize {
            return Err(source_error("DOCX本文の読取上限を超えています"));
        }
        match reader
            .read_event()
            .map_err(|_| source_error("DOCXのXML構造が不正です"))?
        {
            Event::Start(event) => {
                depth += 1;
                if depth > 128 {
                    return Err(source_error("DOCXのXMLが深すぎます"));
                }
                if event.name().as_ref() == b"w:t" {
                    in_text = true;
                }
            }
            Event::End(event) => {
                depth = depth.saturating_sub(1);
                match event.name().as_ref() {
                    b"w:t" => in_text = false,
                    b"w:p" => {
                        if !output.ends_with('\n') {
                            output.push('\n');
                        }
                    }
                    _ => {}
                }
            }
            Event::Empty(event) if event.name().as_ref() == b"w:tab" => output.push('\t'),
            Event::Empty(event) if matches!(event.name().as_ref(), b"w:br" | b"w:cr") => {
                output.push(' ')
            }
            Event::Text(event) if in_text => output.push_str(
                &event
                    .decode()
                    .map_err(|_| source_error("DOCX文字を読み取れません"))?,
            ),
            Event::CData(event) if in_text => output.push_str(
                &event
                    .decode()
                    .map_err(|_| source_error("DOCX文字を読み取れません"))?,
            ),
            Event::GeneralRef(event) if in_text => {
                let entity = format!(
                    "&{};",
                    event
                        .decode()
                        .map_err(|_| source_error("DOCX文字参照が不正です"))?
                );
                output.push_str(
                    &quick_xml::escape::unescape(&entity)
                        .map_err(|_| source_error("未定義のDOCX文字参照です"))?,
                );
            }
            Event::DocType(_) => return Err(source_error("外部/独自のXML実体は参照しません")),
            Event::Eof => {
                if depth != 0 {
                    return Err(source_error("DOCXのXMLが途中で終了しています"));
                }
                break;
            }
            _ => {}
        }
    }
    if output.trim().is_empty() {
        return Err(source_error("DOCXから本文を取得できません"));
    }
    Ok(output.trim().to_string())
}

/// Keep exact words/identifiers and add bounded CJK chunks. Japanese sentences
/// do not require spaces around words such as 料金 or 納期 to find a document.
pub fn search_terms(query: &str) -> Vec<String> {
    let query: String = query.chars().take(512).collect::<String>().to_lowercase();
    let mut terms: Vec<String> = query
        .split(|c: char| c.is_whitespace() || "。、！？?！,;:()（）「」【】".contains(c))
        .filter(|s| s.chars().count() >= 2)
        .take(16)
        .map(str::to_string)
        .collect();
    let category = |c: char| {
        if ('\u{3400}'..='\u{9fff}').contains(&c) {
            1
        } else if ('\u{30a0}'..='\u{30ff}').contains(&c) {
            2
        } else {
            0
        }
    };
    let chars: Vec<char> = query.chars().collect();
    let mut at = 0;
    while at < chars.len() && terms.len() < 64 {
        let kind = category(chars[at]);
        if kind == 0 {
            at += 1;
            continue;
        }
        let from = at;
        while at < chars.len() && category(chars[at]) == kind {
            at += 1;
        }
        let chunk = &chars[from..at];
        if chunk.len() >= 2 {
            terms.push(chunk.iter().collect());
            if kind == 1 {
                for pair in chunk.windows(2).take(16) {
                    terms.push(pair.iter().collect());
                }
            }
        }
    }
    terms.sort();
    terms.dedup();
    terms.truncate(64);
    terms
}

const TEXT_EXTENSIONS: &[&str] = &[
    "c", "cc", "cpp", "cs", "css", "csv", "gql", "go", "graphql", "h", "hpp", "htm", "html",
    "java", "js", "json", "jsx", "kt", "kts", "md", "markdown", "mjs", "mts", "php", "proto", "py",
    "rb", "rs", "scss", "sh", "sql", "svelte", "swift", "toml", "ts", "tsx", "txt", "vue", "xml",
    "yaml", "yml", "zsh",
];

const SKIPPED_DIRS: &[&str] = &[
    "build",
    "coverage",
    "deriveddata",
    "dist",
    "node_modules",
    "out",
    "pods",
    "target",
    "vendor",
];

/// 絶対パスの既存ディレクトリだけを受け付け、正規化済みパスを返す。
pub fn validate_project_root(root_path: &str) -> AppResult<PathBuf> {
    let raw = Path::new(root_path.trim());
    if root_path.trim().is_empty() || !raw.is_absolute() {
        return Err(AppError::BadInput(
            "プロジェクトには既存の絶対ディレクトリを指定してください".into(),
        ));
    }
    let metadata = fs::symlink_metadata(raw)
        .map_err(|_| AppError::BadInput("プロジェクトのディレクトリが見つかりません".into()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(AppError::BadInput(
            "シンボリックリンクではないディレクトリを指定してください".into(),
        ));
    }
    fs::canonicalize(raw)
        .map_err(|_| AppError::BadInput("プロジェクトのパスを正規化できません".into()))
}

pub fn list(conn: &Connection) -> AppResult<Vec<AiProject>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, root_path, enabled, created_at
         FROM ai_projects ORDER BY created_at DESC, id DESC",
    )?;
    let rows = stmt.query_map([], row_to_project)?;
    let mut projects = Vec::new();
    for row in rows {
        projects.push(row?);
    }
    Ok(projects)
}

pub fn add(conn: &Connection, name: &str, canonical_root: &Path) -> AppResult<AiProject> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(AppError::BadInput(
            "プロジェクト名は1〜100文字で指定してください".into(),
        ));
    }
    let root_path = canonical_root.to_string_lossy().to_string();
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM ai_projects WHERE root_path = ?1",
        [&root_path],
        |row| row.get(0),
    )?;
    if exists > 0 {
        return Err(AppError::BadInput(
            "このプロジェクトは既に登録されています".into(),
        ));
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO ai_projects(name, root_path, enabled, created_at)
         VALUES (?1, ?2, 1, ?3)",
        params![name, root_path, now],
    )?;
    Ok(AiProject {
        id: conn.last_insert_rowid(),
        name: name.to_string(),
        root_path,
        enabled: true,
        created_at: now,
    })
}

pub fn remove(conn: &Connection, project_id: i64) -> AppResult<()> {
    let changed = conn.execute("DELETE FROM ai_projects WHERE id = ?1", [project_id])?;
    if changed == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

/// 選択された有効プロジェクトだけをDBから取得する。
pub fn selected(conn: &Connection, project_ids: &[i64]) -> AppResult<Vec<AiProject>> {
    let wanted: HashSet<i64> = project_ids.iter().copied().collect();
    if wanted.is_empty() {
        return Ok(Vec::new());
    }
    let selected: Vec<AiProject> = list(conn)?
        .into_iter()
        .filter(|project| project.enabled && wanted.contains(&project.id))
        .collect();
    if selected.len() != wanted.len() {
        return Err(AppError::BadInput(
            "選択されたプロジェクトが未登録または無効です".into(),
        ));
    }
    Ok(selected)
}

/// ファイル名と本文を対象にした、ファイル数・サイズ・結果数が有限の検索。
pub fn search_projects(
    projects: &[AiProject],
    query: &str,
    limit: u32,
) -> AppResult<Vec<ProjectFileHit>> {
    Ok(search_projects_report(projects, query, limit)?.hits)
}

/// Returns evidence of incomplete retrieval separately from actual readable hits.
/// Unsupported/corrupt documents must never appear as successfully read sources.
pub fn search_projects_report(
    projects: &[AiProject],
    query: &str,
    limit: u32,
) -> AppResult<ProjectSearchReport> {
    let limit = limit.clamp(1, MAX_RESULTS) as usize;
    let terms = search_terms(query);
    if terms.is_empty() {
        return Ok(finish_report(Vec::new(), limit, Vec::new(), false));
    }

    let mut ranked_hits: Vec<(u32, ProjectFileHit)> = Vec::new();
    let mut review_reasons = Vec::new();
    let mut visited = 0usize;
    let mut scanned = 0usize;
    let mut total_bytes = 0u64;
    let started = Instant::now();
    for project in projects.iter().filter(|project| project.enabled) {
        let root = Path::new(&project.root_path);
        let Ok(root_meta) = fs::symlink_metadata(root) else {
            add_review(&mut review_reasons, "登録資料のルートを読み取れません");
            continue;
        };
        if root_meta.file_type().is_symlink() || !root_meta.is_dir() {
            add_review(&mut review_reasons, "登録資料のルートが変化しています");
            continue;
        }
        // 登録後にルートが差し替えられても、登録時と同じ実体だけを読む。
        let Ok(current_root) = fs::canonicalize(root) else {
            add_review(&mut review_reasons, "登録資料のルートを確認できません");
            continue;
        };
        if current_root != root {
            add_review(&mut review_reasons, "登録資料のルートが変化しています");
            continue;
        }
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            if started.elapsed() >= MAX_SCAN_ELAPSED {
                return Ok(finish_report(ranked_hits, limit, review_reasons, true));
            }
            let Ok(entries) = fs::read_dir(&dir) else {
                add_review(&mut review_reasons, "資料フォルダの一部を読み取れません");
                continue;
            };
            for entry in entries.flatten() {
                if visited >= MAX_VISITED_ENTRIES {
                    return Ok(finish_report(ranked_hits, limit, review_reasons, true));
                }
                visited += 1;
                if started.elapsed() >= MAX_SCAN_ELAPSED {
                    return Ok(finish_report(ranked_hits, limit, review_reasons, true));
                }
                let path = entry.path();
                let Ok(meta) = fs::symlink_metadata(&path) else {
                    continue;
                };
                if meta.file_type().is_symlink() {
                    continue;
                }
                if meta.is_dir() {
                    if !skip_directory(&path) {
                        stack.push(path);
                    }
                    continue;
                }
                if !meta.is_file() || sensitive_file(&path) {
                    continue;
                }
                let docx = is_docx(&path);
                if !allowed_text_file(&path) && !docx {
                    if path.extension().and_then(|v| v.to_str()).is_some_and(|v| {
                        [
                            "pdf", "doc", "xls", "xlsx", "ppt", "pptx", "png", "jpg", "jpeg",
                        ]
                        .contains(&v.to_lowercase().as_str())
                    }) {
                        add_review(
                            &mut review_reasons,
                            "PDF・表計算・スライド・画像の本文/OCRは未対応のため参照していません",
                        );
                    }
                    continue;
                }
                if meta.len() > if docx { MAX_DOCX_BYTES } else { MAX_FILE_BYTES } {
                    add_review(
                        &mut review_reasons,
                        "サイズ上限を超える資料は参照していません",
                    );
                    continue;
                }
                if scanned >= MAX_SCANNED_FILES {
                    return Ok(finish_report(ranked_hits, limit, review_reasons, true));
                }
                if total_bytes.saturating_add(meta.len()) > MAX_TOTAL_BYTES {
                    return Ok(finish_report(ranked_hits, limit, review_reasons, true));
                }
                scanned += 1;
                total_bytes += meta.len();
                // 各ファイルも再正規化し、ルート外や途中で差し込まれたリンクを拒否する。
                let Ok(canonical_file) = fs::canonicalize(&path) else {
                    continue;
                };
                if !canonical_file.starts_with(&current_root) || canonical_file != path {
                    continue;
                }
                let relative = path.strip_prefix(root).unwrap_or(&path);
                let extracted = match read_project_source(project, &relative.to_string_lossy()) {
                    Ok(extracted) => extracted,
                    Err(_) => {
                        add_review(
                            &mut review_reasons,
                            "破損・未対応文字コード・変更された資料の一部は参照していません",
                        );
                        continue;
                    }
                };
                let content = extracted.text;
                let haystack = content.to_lowercase();
                let filename = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let content_matches = terms
                    .iter()
                    .filter(|term| haystack.contains(term.as_str()))
                    .count() as u32;
                let filename_matches = terms
                    .iter()
                    .filter(|term| filename.contains(term.as_str()))
                    .count() as u32;
                if content_matches == 0 && filename_matches == 0 {
                    continue;
                }
                let (snippet, line_number) = matching_snippet_with_label(&content, &terms, docx);
                let identifier_bonus = terms
                    .iter()
                    .filter(|term| {
                        (term.contains('_') || term.contains('-'))
                            && (haystack.contains(term.as_str())
                                || filename.contains(term.as_str()))
                    })
                    .count() as u32
                    * 5;
                let score = filename_matches * 10 + content_matches * 3 + identifier_bonus;
                ranked_hits.push((
                    score,
                    ProjectFileHit {
                        project_id: project.id,
                        project_name: project.name.clone(),
                        relative_path: relative.to_string_lossy().to_string(),
                        snippet,
                        line_number,
                    },
                ));
            }
        }
    }
    Ok(finish_report(ranked_hits, limit, review_reasons, false))
}

fn add_review(reasons: &mut Vec<String>, text: &str) {
    if reasons.len() < 16 && !reasons.iter().any(|reason| reason == text) {
        reasons.push(text.to_string());
    }
}

fn finish_report(
    hits: Vec<(u32, ProjectFileHit)>,
    limit: usize,
    mut review_reasons: Vec<String>,
    incomplete: bool,
) -> ProjectSearchReport {
    let truncated = incomplete || hits.len() > limit;
    if truncated {
        add_review(
            &mut review_reasons,
            "検索時間・件数・容量の上限内の抜粋です。未参照資料が残る可能性があります",
        );
    }
    ProjectSearchReport {
        hits: finish_hits(hits, limit),
        review_reasons,
        truncated,
    }
}

fn finish_hits(mut hits: Vec<(u32, ProjectFileHit)>, limit: usize) -> Vec<ProjectFileHit> {
    hits.sort_by(|(score_a, hit_a), (score_b, hit_b)| {
        score_b
            .cmp(score_a)
            .then_with(|| hit_a.project_id.cmp(&hit_b.project_id))
            .then_with(|| hit_a.relative_path.cmp(&hit_b.relative_path))
    });
    hits.into_iter().take(limit).map(|(_, hit)| hit).collect()
}

fn row_to_project(row: &rusqlite::Row<'_>) -> rusqlite::Result<AiProject> {
    Ok(AiProject {
        id: row.get(0)?,
        name: row.get(1)?,
        root_path: row.get(2)?,
        enabled: row.get::<_, i64>(3)? != 0,
        created_at: row.get(4)?,
    })
}

fn skip_directory(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return true;
    };
    name.starts_with('.') || SKIPPED_DIRS.contains(&name.to_lowercase().as_str())
}

fn allowed_text_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| TEXT_EXTENSIONS.contains(&extension.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// APIキー、秘密鍵、認証情報を含みやすい名前は、拡張子がテキストでも除外する。
fn sensitive_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_lowercase();
    name.starts_with('.')
        || [
            "credential",
            "credentials",
            "secret",
            "private_key",
            "private-key",
            "service_account",
            "service-account",
            "apikey",
            "api_key",
            "api-key",
            "token",
            "access_token",
            "access-token",
            "refresh_token",
            "refresh-token",
            "id_rsa",
            "id_ed25519",
        ]
        .iter()
        .any(|marker| name.contains(marker))
}

fn matching_snippet_with_label(
    content: &str,
    terms: &[String],
    docx: bool,
) -> (String, Option<u32>) {
    let lines: Vec<&str> = content.lines().collect();
    let matched_line = lines.iter().position(|line| {
        let lowered = line.to_lowercase();
        terms.iter().any(|term| lowered.contains(term))
    });
    let anchor = matched_line.unwrap_or(0);
    let start = anchor.saturating_sub(2);
    let mut snippet = String::new();
    for (index, line) in lines.iter().enumerate().skip(start).take(5) {
        // These are extracted paragraphs, not Word page or source-file lines.
        let labelled = if docx {
            format!("段落{}: {}\n", index + 1, line)
        } else {
            format!("L{}: {}\n", index + 1, line)
        };
        if snippet.chars().count() + labelled.chars().count() > MAX_SNIPPET_CHARS {
            let remaining = MAX_SNIPPET_CHARS.saturating_sub(snippet.chars().count());
            snippet.extend(labelled.chars().take(remaining));
            break;
        }
        snippet.push_str(&labelled);
    }
    (
        snippet.trim().to_string(),
        if docx {
            None
        } else {
            matched_line.and_then(|line| u32::try_from(line + 1).ok())
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::migrations;
    use std::io::Write;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("a-mail-{label}-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&path).expect("create temp directory");
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn root_must_be_absolute_existing_directory_and_not_symlink() {
        assert!(validate_project_root("relative/path").is_err());
        let missing = std::env::temp_dir().join(format!("missing-{}", uuid::Uuid::new_v4()));
        assert!(validate_project_root(&missing.to_string_lossy()).is_err());

        let root = TestDir::new("root");
        assert_eq!(
            validate_project_root(&root.0.to_string_lossy()).expect("valid root"),
            fs::canonicalize(&root.0).expect("canonical root")
        );

        #[cfg(unix)]
        {
            let holder = TestDir::new("symlink-holder");
            let link = holder.0.join("linked-root");
            std::os::unix::fs::symlink(&root.0, &link).expect("create symlink");
            assert!(validate_project_root(&link.to_string_lossy()).is_err());
        }
    }

    #[test]
    fn registration_round_trip_uses_canonical_path() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::apply(&conn).expect("migrate");
        let root = TestDir::new("registration");
        let canonical = validate_project_root(&root.0.to_string_lossy()).expect("root");

        let project = add(&conn, " Docs ", &canonical).expect("add");
        assert_eq!(project.name, "Docs");
        assert_eq!(list(&conn).expect("list"), vec![project.clone()]);
        assert!(add(&conn, "Duplicate", &canonical).is_err());
        remove(&conn, project.id).expect("remove");
        assert!(list(&conn).expect("empty list").is_empty());
        assert!(selected(&conn, &[project.id]).is_err());
    }

    #[test]
    fn search_uses_or_matching_and_deterministic_filename_priority() {
        let root = TestDir::new("ranking");
        fs::write(root.0.join("z.md"), "first\nneedle in content").expect("content file");
        fs::write(root.0.join("needle-guide.md"), "unrelated body").expect("filename file");

        let hits = search_projects(&[project(&root.0)], "needle absent", 10).expect("search");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].relative_path, "needle-guide.md");
        assert_eq!(hits[1].line_number, Some(2));
    }

    #[test]
    fn search_skips_hidden_vendor_binary_large_and_symlinked_content() {
        let root = TestDir::new("search-root");
        fs::write(root.0.join("visible.md"), "intro\nneedle safe result\nend").expect("write");
        fs::write(root.0.join("binary.bin"), "needle binary").expect("write");
        fs::write(root.0.join("credentials.json"), "needle credentials")
            .expect("write credentials");
        fs::write(root.0.join("project-secrets.json"), "needle secret").expect("write secrets");
        fs::create_dir(root.0.join(".hidden")).expect("hidden dir");
        fs::write(root.0.join(".hidden/secret.md"), "needle hidden").expect("write");
        fs::create_dir(root.0.join("vendor")).expect("vendor dir");
        fs::write(root.0.join("vendor/dependency.md"), "needle vendor").expect("write");
        fs::write(
            root.0.join("large.txt"),
            vec![b'x'; MAX_FILE_BYTES as usize + 1],
        )
        .expect("large file");

        #[cfg(unix)]
        {
            let outside = TestDir::new("search-outside");
            fs::write(outside.0.join("outside.md"), "needle outside").expect("write outside");
            std::os::unix::fs::symlink(&outside.0, root.0.join("linked")).expect("link outside");
            let hits = search_projects(&[project(&root.0)], "needle", 20).expect("search");
            assert_eq!(hits.len(), 1);
            assert_eq!(hits[0].relative_path, "visible.md");
            return;
        }

        #[cfg(not(unix))]
        {
            let hits = search_projects(&[project(&root.0)], "needle", 20).expect("search");
            assert_eq!(hits.len(), 1);
            assert_eq!(hits[0].relative_path, "visible.md");
        }
    }

    fn docx_bytes(xml: &str) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(
                "word/document.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .expect("zip entry");
        writer.write_all(xml.as_bytes()).expect("XML");
        writer.finish().expect("zip finish").into_inner()
    }

    const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
      <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
        <w:p><w:r><w:t>テストメール用 サンプルサービス</w:t></w:r></w:p>
        <w:tbl><w:tr><w:tc><w:p><w:r><w:t>月額料金は9,800円 &amp; 無料体験あり</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
        <w:p><w:r><w:t>秘密の外部データは取得しない</w:t></w:r></w:p>
      </w:body></w:document>"#;

    #[test]
    fn japanese_query_matches_docx_text_and_has_paragraph_not_page_locator() {
        let root = TestDir::new("docx-ja");
        fs::write(root.0.join("企画書.docx"), docx_bytes(DOCUMENT)).unwrap();
        let p = project(&root.0);
        let report =
            search_projects_report(&[p.clone()], "月額料金と無料体験について教えてください", 8)
                .unwrap();
        assert_eq!(report.hits.len(), 1);
        assert!(report.review_reasons.is_empty());
        assert!(!report.truncated);
        let hit = &report.hits[0];
        assert_eq!(hit.line_number, None);
        assert!(hit
            .snippet
            .contains("段落2: 月額料金は9,800円 & 無料体験あり"));
        let read = read_project_source(&p, "企画書.docx").unwrap();
        assert!(read.is_docx);
        assert_eq!(read.version.len(), 64);
        assert_eq!(
            read.version,
            read_project_source(&p, "企画書.docx").unwrap().version
        );
        fs::write(
            root.0.join("企画書.docx"),
            docx_bytes(&DOCUMENT.replace("9,800", "12,000")),
        )
        .unwrap();
        assert_ne!(
            read.version,
            read_project_source(&p, "企画書.docx").unwrap().version
        );
    }

    #[test]
    fn malformed_archives_entities_and_oversized_xml_are_not_sources() {
        assert!(extract_docx(b"not a zip").is_err());
        assert!(extract_document_xml(b"<w:p><w:t>unfinished").is_err());
        assert!(extract_document_xml(b"<w:p><w:t>&unknown;</w:t></w:p>").is_err());
        assert!(extract_document_xml(
            b"<!DOCTYPE w:p [<!ENTITY x SYSTEM 'file:///invalid'>]><w:p><w:t>&x;</w:t></w:p>"
        )
        .is_err());
        assert!(extract_document_xml(&vec![b'x'; MAX_DOCX_XML_BYTES as usize + 1]).is_err());
        let root = TestDir::new("unsupported");
        fs::write(root.0.join("料金.docx"), b"not a zip").unwrap();
        fs::write(root.0.join("料金.pdf"), b"%PDF-test").unwrap();
        let report = search_projects_report(&[project(&root.0)], "料金", 8).unwrap();
        assert!(report.hits.is_empty());
        assert!(report.review_reasons.iter().any(|s| s.contains("未対応")));
        assert!(report.review_reasons.iter().any(|s| s.contains("破損")));
    }

    #[test]
    fn source_reader_rejects_traversal_symlinks_and_disabled_projects() {
        let root = TestDir::new("source-safety");
        fs::write(root.0.join("safe.md"), "料金: 確認用").unwrap();
        let mut p = project(&root.0);
        assert!(read_project_source(&p, "../safe.md").is_err());
        assert!(read_project_source(&p, "/safe.md").is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.0.join("safe.md"), root.0.join("link.md")).unwrap();
            assert!(read_project_source(&p, "link.md").is_err());
            let canonical = fs::canonicalize(&root.0).unwrap();
            assert!(open_source_file(&canonical, Path::new("link.md")).is_err());
        }
        p.enabled = false;
        assert!(read_project_source(&p, "safe.md").is_err());
        assert!(search_projects_report(&[p], "料金", 8)
            .unwrap()
            .hits
            .is_empty());
    }

    #[test]
    fn japanese_terms_and_top_k_report_are_bounded() {
        let terms = search_terms("無料体験について、月額料金と納期を確認したいです。");
        assert!(terms.contains(&"料金".into()));
        assert!(terms.contains(&"無料体験".into()));
        assert!(search_terms(&"料金確認".repeat(10000)).len() <= 64);
        let root = TestDir::new("top-k");
        fs::write(root.0.join("a.md"), "月額料金").unwrap();
        fs::write(root.0.join("b.md"), "月額料金").unwrap();
        let report = search_projects_report(&[project(&root.0)], "料金", 1).unwrap();
        assert_eq!(report.hits.len(), 1);
        assert!(report.truncated);
        assert!(!report.review_reasons.is_empty());
    }

    /// Local acceptance for the owner's fictional reply source; not a production
    /// mailbox/API test and not required when the optional Sample folder is absent.
    #[test]
    #[ignore = "local Sample DOCX acceptance; run explicitly"]
    fn sample_proposal_can_be_read_as_reply_evidence() {
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("../Sample");
        let p = project(&sample);
        let extracted = read_project_source(&p, "サンプルサービス企画書.docx").unwrap();
        assert!(extracted.is_docx);
        assert!(extracted.text.contains("料金"));
        assert!(extracted.text.contains("テスト") || extracted.text.contains("架空"));
        let report = search_projects_report(&[p], "料金と無料体験を教えてください", 8).unwrap();
        assert!(report
            .hits
            .iter()
            .any(|hit| hit.relative_path == "サンプルサービス企画書.docx"));
    }

    fn project(root: &Path) -> AiProject {
        AiProject {
            id: 7,
            name: "Test".into(),
            root_path: fs::canonicalize(root)
                .expect("canonical test root")
                .to_string_lossy()
                .to_string(),
            enabled: true,
            created_at: 0,
        }
    }
}

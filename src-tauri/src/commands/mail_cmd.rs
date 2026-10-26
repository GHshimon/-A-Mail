use tauri::{Manager, State};

use crate::crypto::keychain;
use crate::error::{AppError, AppResult};
use crate::imap;
use crate::state::AppState;
use crate::store::{
    self,
    models::{Folder, MessageFull, MessageHeader},
};

fn kick_auto_draft(app: tauri::AppHandle, account_id: i64) {
    // Opt-in settings, quota and single-running-job checks are re-evaluated in
    // the worker. Mail sync stays responsive while generation runs.
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let _ = super::assistant_cmd::process_queue_impl(&state, account_id).await;
    });
}

/// フォルダ一覧(DB キャッシュ)。実データは sync_folders で反映。
#[tauri::command]
pub fn list_folders(state: State<'_, AppState>, account_id: i64) -> AppResult<Vec<Folder>> {
    let conn = state.db()?;
    store::list_folders(&conn, account_id)
}

/// IMAP へ接続・ログインできるか検証する(フォルダ操作はしない)。
#[tauri::command]
pub async fn test_connection(state: State<'_, AppState>, account_id: i64) -> AppResult<()> {
    let (email, host, port) = {
        let conn = state.db()?;
        store::account_conn(&conn, account_id)?.ok_or(AppError::NotFound)?
    };
    let pass = keychain::account_password(&email)?
        .ok_or_else(|| AppError::Auth("アプリパスワードが未登録です".into()))?;

    let mut session = imap::client::open_session(&host, port, &email, &pass).await?;
    imap::client::logout(&mut session).await?;
    Ok(())
}

/// IMAP LIST でフォルダ(ラベル)を取得し DB に upsert、最新の一覧を返す。
#[tauri::command]
pub async fn sync_folders(state: State<'_, AppState>, account_id: i64) -> AppResult<Vec<Folder>> {
    let (email, host, port) = {
        let conn = state.db()?;
        store::account_conn(&conn, account_id)?.ok_or(AppError::NotFound)?
    };
    let pass = keychain::account_password(&email)?
        .ok_or_else(|| AppError::Auth("アプリパスワードが未登録です".into()))?;

    // ネットワーク I/O 中は DB ロックを保持しない(MutexGuard は上のブロックで解放済み)。
    let mut session = imap::client::open_session(&host, port, &email, &pass).await?;
    let infos = imap::sync::fetch_folders(&mut session).await?;
    let _ = imap::client::logout(&mut session).await;

    let folders = {
        let conn = state.db()?;
        for f in &infos {
            store::upsert_folder(&conn, account_id, &f.imap_path, &f.name, f.role.as_deref())?;
        }
        store::list_folders(&conn, account_id)?
    };
    Ok(folders)
}

/// フォルダを IMAP から同期してメッセージヘッダ(直近 `limit` 件)を取得・保存し、
/// 最新の一覧を返す。UIDVALIDITY が変化した場合はキャッシュを作り直す。
#[tauri::command]
pub async fn sync_folder(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    folder_id: i64,
    limit: Option<u32>,
) -> AppResult<Vec<MessageHeader>> {
    let info = {
        let conn = state.db()?;
        store::folder_sync_info(&conn, folder_id)?.ok_or(AppError::NotFound)?
    };
    let pass = keychain::account_password(&info.email)?
        .ok_or_else(|| AppError::Auth("アプリパスワードが未登録です".into()))?;
    let limit = limit.unwrap_or(50).clamp(1, 500);

    // ネットワーク I/O 中は DB ロックを保持しない。
    let mut session =
        imap::client::open_session(&info.imap_host, info.imap_port, &info.email, &pass).await?;
    let (uidvalidity, uidnext, headers) =
        imap::fetch::fetch_recent_headers(&mut session, &info.imap_path, limit).await?;
    let _ = imap::client::logout(&mut session).await;

    let out = {
        let conn = state.db()?;
        // UIDVALIDITY 変化 → UID の意味が変わるのでフォルダのキャッシュを破棄。
        if let Some(prev) = info.uidvalidity {
            if prev != uidvalidity {
                store::clear_folder_messages(&conn, folder_id)?;
            }
        }
        for h in &headers {
            let flags = (h.seen as i64) | ((h.flagged as i64) << 1);
            store::upsert_message_header(
                &conn,
                info.account_id,
                folder_id,
                h.uid,
                &h.from,
                &h.subject,
                h.date,
                flags,
            )?;
        }
        store::set_folder_uidstate(&conn, folder_id, uidvalidity, uidnext)?;
        // Opt-in only. Existing rows are excluded by the watermark established
        // when automatic drafts are enabled.
        let _ = crate::assistant_automation::enqueue_new(&conn, info.account_id)?;
        store::list_messages(&conn, folder_id, 0, limit as i64)?
    };
    kick_auto_draft(app, info.account_id);
    Ok(out)
}

/// 一覧の 1 ページ分。`has_more` が false なら、これ以上は過去へ遡れない。
#[derive(serde::Serialize)]
pub struct MessagePage {
    pub messages: Vec<MessageHeader>,
    pub has_more: bool,
}

/// 一覧を過去方向へ 1 ページ読み足す(無限スクロール用)。
///
/// `offset` は「すでに表示している件数」。まず DB キャッシュを見て、足りなければ
/// IMAP から該当ページのヘッダを取得して保存し、改めて DB から返す。
/// DB は date 降順、IMAP はシーケンス番号(到着順)で窓を切るため厳密には一致しない
/// が、取得済みは upsert で重複しないので、読み足しの入口としてはこれで足りる。
#[tauri::command]
pub async fn load_more_messages(
    state: State<'_, AppState>,
    folder_id: i64,
    offset: u32,
    limit: Option<u32>,
) -> AppResult<MessagePage> {
    let limit = limit.unwrap_or(50).clamp(1, 500);

    // 1) DB キャッシュだけで 1 ページ埋まるなら、ネットワークには出ない。
    {
        let conn = state.db()?;
        let cached = store::list_messages(&conn, folder_id, offset as i64, limit as i64)?;
        if cached.len() as u32 == limit {
            return Ok(MessagePage {
                messages: cached,
                has_more: true,
            });
        }
    }

    let info = {
        let conn = state.db()?;
        store::folder_sync_info(&conn, folder_id)?.ok_or(AppError::NotFound)?
    };
    let pass = keychain::account_password(&info.email)?
        .ok_or_else(|| AppError::Auth("アプリパスワードが未登録です".into()))?;

    // ネットワーク I/O 中は DB ロックを保持しない。
    let mut session =
        imap::client::open_session(&info.imap_host, info.imap_port, &info.email, &pass).await?;
    let (uidvalidity, uidnext, headers, exists) =
        imap::fetch::fetch_headers_page(&mut session, &info.imap_path, offset, limit).await?;
    let _ = imap::client::logout(&mut session).await;

    let conn = state.db()?;
    if let Some(prev) = info.uidvalidity {
        if prev != uidvalidity {
            store::clear_folder_messages(&conn, folder_id)?;
        }
    }
    for h in &headers {
        let flags = (h.seen as i64) | ((h.flagged as i64) << 1);
        store::upsert_message_header(
            &conn,
            info.account_id,
            folder_id,
            h.uid,
            &h.from,
            &h.subject,
            h.date,
            flags,
        )?;
    }
    store::set_folder_uidstate(&conn, folder_id, uidvalidity, uidnext)?;

    let messages = store::list_messages(&conn, folder_id, offset as i64, limit as i64)?;
    Ok(MessagePage {
        // サーバ側にまだ古いメッセージが残っているか(EXISTS 基準)。
        has_more: offset.saturating_add(limit) < exists,
        messages,
    })
}

/// メッセージ本文を取得する(未取得なら IMAP から遅延取得して保存)。
#[tauri::command]
pub async fn get_message(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    message_id: i64,
) -> AppResult<MessageFull> {
    let (folder_id, uid, fetched) = {
        let conn = state.db()?;
        store::message_locate(&conn, message_id)?.ok_or(AppError::NotFound)?
    };

    // 既に取得済みなら DB から即返す。
    if fetched {
        let conn = state.db()?;
        return store::get_message_full(&conn, message_id);
    }

    let info = {
        let conn = state.db()?;
        store::folder_sync_info(&conn, folder_id)?.ok_or(AppError::NotFound)?
    };
    let pass = keychain::account_password(&info.email)?
        .ok_or_else(|| AppError::Auth("アプリパスワードが未登録です".into()))?;

    let mut session =
        imap::client::open_session(&info.imap_host, info.imap_port, &info.email, &pass).await?;
    let body = imap::fetch::fetch_body(&mut session, &info.imap_path, uid).await?;
    let _ = imap::client::logout(&mut session).await;

    if let Some(b) = body {
        let atts: Vec<(String, String, i64, Option<String>)> = b
            .attachments
            .iter()
            .map(|a| (a.filename.clone(), a.mime.clone(), a.size, None))
            .collect();
        let conn = state.db()?;
        store::save_message_body(
            &conn,
            message_id,
            &b.to,
            &b.cc,
            &b.message_id,
            b.in_reply_to.as_deref(),
            &b.references,
            b.text.as_deref(),
            b.html.as_deref(),
            &atts,
        )?;
        let _ = crate::assistant_automation::enqueue_new(&conn, info.account_id)?;
    }

    kick_auto_draft(app, info.account_id);
    let conn = state.db()?;
    store::get_message_full(&conn, message_id)
}

/// メッセージヘッダのページング取得(DB キャッシュ)。
#[tauri::command]
pub fn list_messages(
    state: State<'_, AppState>,
    folder_id: i64,
    offset: i64,
    limit: i64,
) -> AppResult<Vec<MessageHeader>> {
    let conn = state.db()?;
    store::list_messages(&conn, folder_id, offset, limit)
}

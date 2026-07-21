//! IMAP 接続 / TLS / 認証(LOGIN, アプリパスワード)。
//!
//! async-imap(runtime-tokio)+ async-native-tls(runtime-tokio)。
//! macOS では native-tls が Secure Transport を使うため openssl 非依存。
//! エラーは種別のみに丸め、資格情報・本文はメッセージに載せない。

use async_imap::Session;
use async_native_tls::{TlsConnector, TlsStream};
use tokio::net::TcpStream;

use crate::error::{AppError, AppResult};

/// TLS 上の認証済み IMAP セッション。
pub type ImapSession = Session<TlsStream<TcpStream>>;

/// TLS(993)で接続し、LOGIN(アプリパスワード)まで済ませたセッションを返す。
pub async fn open_session(
    host: &str,
    port: u16,
    user: &str,
    pass: &str,
) -> AppResult<ImapSession> {
    // ここでは `?`(From<io::Error>=Db)を使わず、種別を明示的に対応づける。
    let tcp = TcpStream::connect((host, port))
        .await
        .map_err(|_| AppError::Network("IMAP サーバへ接続できませんでした".into()))?;

    let tls = TlsConnector::new();
    let tls_stream = tls
        .connect(host, tcp)
        .await
        .map_err(|_| AppError::Imap("TLS ハンドシェイクに失敗しました".into()))?;

    let mut client = async_imap::Client::new(tls_stream);
    // 接続直後のサーバ挨拶(greeting)を読み捨てる。
    let _ = client.read_response().await;

    let session = client.login(user, pass).await.map_err(|(e, _client)| {
        // サーバからの実エラー(例: Gmail の "[AUTHENTICATIONFAILED]"/"Application-specific
        // password required")を診断用にログへ。資格情報は含まれない。フロントには一般化文言のみ。
        tracing::warn!(error = %e, "IMAP LOGIN 失敗");
        AppError::Auth(
            "ログインに失敗しました(メールアドレスとアプリパスワードを確認してください)".into(),
        )
    })?;

    Ok(session)
}

/// セッションを閉じる(失敗は致命ではないので握りつぶす)。
pub async fn logout(session: &mut ImapSession) -> AppResult<()> {
    let _ = session.logout().await;
    Ok(())
}

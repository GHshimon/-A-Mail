//! メッセージ組み立て(MIME/ヘッダ)+ SMTP 送信。M2(SPEC/IMPLEMENTATION §6)。
//!
//! - 本文 text のみ → `text/plain`。HTML 併用 → `multipart/alternative`。
//! - 返信は `In-Reply-To` / `References` を付与(値は呼び出し側が用意)。
//! - TLS は既存の IMAP(async-native-tls)に揃え lettre も native-tls を使用
//!   (macOS は Secure Transport のため openssl 非依存)。
//! - エラーは種別のみに丸め、資格情報・本文をメッセージに載せない。

use lettre::message::{Mailbox, Message, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::store::models::OutgoingMessage;

/// Message-ID 参照を `<...>` 形に正規化する(mail-parser 由来はブラケット無し、
/// フロント由来は有りのことがあるため両対応)。空文字は None 相当で弾く。
fn ensure_brackets(id: &str) -> Option<String> {
    let t = id.trim();
    if t.is_empty() {
        return None;
    }
    if t.starts_with('<') && t.ends_with('>') {
        Some(t.to_string())
    } else {
        Some(format!("<{t}>"))
    }
}

fn parse_mailbox(s: &str) -> AppResult<Mailbox> {
    s.trim()
        .parse::<Mailbox>()
        .map_err(|_| AppError::BadInput(format!("宛先アドレスの形式が不正です: {s}")))
}

/// `OutgoingMessage` から送信用 `Message` を組み立て、生成した Message-ID(`<...>`)を併せて返す。
pub fn build_message(
    from_email: &str,
    from_name: Option<&str>,
    out: &OutgoingMessage,
) -> AppResult<(Message, String)> {
    if out.to.is_empty() {
        return Err(AppError::BadInput("宛先が指定されていません".into()));
    }

    // Message-ID は送信元ドメインで自前生成(送信後の Sent 照合・スレッド化に使う)。
    let domain = from_email.split('@').nth(1).unwrap_or("localhost");
    let message_id = format!("<{}@{}>", Uuid::new_v4(), domain);

    let from_mbox = match from_name {
        Some(n) if !n.trim().is_empty() => format!("{} <{from_email}>", n.trim()),
        _ => from_email.to_string(),
    }
    .parse::<Mailbox>()
    .map_err(|_| AppError::BadInput("送信元アドレスが不正です".into()))?;

    let mut builder = Message::builder()
        .from(from_mbox)
        .subject(out.subject.clone())
        .message_id(Some(message_id.clone()));

    for a in &out.to {
        builder = builder.to(parse_mailbox(a)?);
    }
    for a in &out.cc {
        builder = builder.cc(parse_mailbox(a)?);
    }
    for a in &out.bcc {
        builder = builder.bcc(parse_mailbox(a)?);
    }

    if let Some(irt) = out.in_reply_to.as_deref().and_then(ensure_brackets) {
        builder = builder.in_reply_to(irt);
    }
    let refs: Vec<String> = out
        .references
        .iter()
        .filter_map(|r| ensure_brackets(r))
        .collect();
    if !refs.is_empty() {
        builder = builder.references(refs.join(" "));
    }

    // 本文: HTML があれば multipart/alternative(text + html)、無ければ text/plain 単体。
    let message = match out.body_html.as_deref() {
        Some(html) if !html.is_empty() => builder.multipart(
            MultiPart::alternative()
                .singlepart(SinglePart::plain(out.body_text.clone()))
                .singlepart(SinglePart::html(html.to_string())),
        ),
        _ => builder.singlepart(SinglePart::plain(out.body_text.clone())),
    }
    .map_err(|_| AppError::Smtp("メッセージの組み立てに失敗しました".into()))?;

    Ok((message, message_id))
}

/// SMTP でメッセージを送信する。
///
/// - STARTTLS(587: iCloud)か暗黙 TLS(465: Gmail)かは `starttls` で切替。
/// - 認証はアカウントのアプリパスワード(Keychain 由来。ここには文字列で渡すが
///   ログ・エラーには出さない)。
pub async fn send_message(
    smtp_host: &str,
    smtp_port: u16,
    starttls: bool,
    auth_user: &str,
    auth_pass: &str,
    message: Message,
) -> AppResult<()> {
    let builder = if starttls {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(smtp_host)
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::relay(smtp_host)
    }
    .map_err(|_| AppError::Smtp("SMTP 接続の初期化に失敗しました".into()))?;

    let transport = builder
        .port(smtp_port)
        .credentials(Credentials::new(
            auth_user.to_string(),
            auth_pass.to_string(),
        ))
        .build();

    transport
        .send(message)
        .await
        // 認証失敗・接続失敗・宛先拒否などを一括で丸める(秘密情報は載せない)。
        .map_err(|_| {
            AppError::Smtp("メール送信に失敗しました(接続・認証・宛先をご確認ください)".into())
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> OutgoingMessage {
        OutgoingMessage {
            account_id: 1,
            to: vec!["田中 亮 <tanaka@example.jp>".into()],
            cc: vec![],
            bcc: vec![],
            subject: "Re: テスト".into(),
            body_text: "本文です".into(),
            body_html: None,
            in_reply_to: Some("msg-1@example.jp".into()), // ブラケット無し
            references: vec!["<root@example.jp>".into()], // ブラケット有り
            draft_id: None,
        }
    }

    #[test]
    fn builds_headers_and_normalizes_message_ids() {
        let (msg, id) = build_message("me@example.com", Some("ドウチ"), &sample()).unwrap();
        assert!(id.starts_with('<') && id.ends_with('>'));
        let raw = String::from_utf8(msg.formatted()).unwrap();
        assert!(raw.contains(&format!("Message-ID: {id}")));
        // ブラケット無しの in_reply_to が正規化されている。
        assert!(raw.contains("In-Reply-To: <msg-1@example.jp>"));
        assert!(raw.contains("References: <root@example.jp>"));
        // 日本語件名は RFC2047 エンコードワード(=?utf-8?...?=)になる。
        assert!(raw.contains("Subject:"));
        assert!(raw.contains("=?utf-8?") || raw.contains("テスト"));
    }

    #[test]
    fn rejects_empty_recipients() {
        let mut m = sample();
        m.to.clear();
        assert!(build_message("me@example.com", None, &m).is_err());
    }
}

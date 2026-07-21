//! 機能 A/B/C の system / 入力構築(§7.3)。
//!
//! プロンプトに載せる本文は各所で字数上限を設け、トークンとプライバシー面の
//! 露出を抑える。カーソル以降・無関係な過去メールは送らない。

/// 文字列の末尾 `n` 文字に丸める(マルチバイト安全)。
fn tail(s: &str, n: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= n {
        s.to_string()
    } else {
        chars[chars.len() - n..].iter().collect()
    }
}

/// 文字列の先頭 `n` 文字に丸める。
fn head(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

// ---- A: 予測入力(ゴーストテキスト) ----

pub const COMPLETE_SYSTEM: &str = "あなたはメール作成の続きを予測する補完エンジンです。\
ユーザーが書いた文の自然な続きだけを、短く(最大1文〜十数語)日本語で出力してください。\
挨拶・説明・引用符・箇条書きは付けない。既に書かれている文字は繰り返さない。\
続きが思いつかない場合は空文字を返す。";

/// A の入力: 件名 + 宛先 + カーソル直前の本文(末尾 700 文字)。
pub fn complete_input(subject: &str, to: &[String], body_prefix: &str) -> String {
    format!(
        "件名: {}\n宛先: {}\n---\nここまでの本文(この続きだけを書く):\n{}",
        subject,
        to.join(", "),
        tail(body_prefix, 700),
    )
}

// ---- B: 関連情報サイドバー ----

pub const CONTEXT_SYSTEM: &str = "あなたはメール作成の補助アシスタントです。\
作成中メールと関連する過去メール抜粋を読み、経緯・約束事(コミット)・未対応事項を抽出します。\
出力は JSON 配列のみ。各要素は {\"category\": \"history\"|\"commit\"|\"todo\", \"text\": \"...\"}。\
history=経緯, commit=約束・回答済みの事項, todo=未対応・次アクション。\
推測は避け、抜粋に根拠が無い項目は出さない。該当が無ければ空配列 [] を返す。\
説明文やコードフェンスは付けず、JSON だけを出力する。";

/// B の入力: 作成中メールの要旨 + 過去メール(各 800 字上限)。
/// `related` は (日付文字列, 相手, 件名, 本文抜粋)。
pub fn context_input(
    subject: &str,
    to: &[String],
    body: &str,
    related: &[(String, String, String, String)],
) -> String {
    let mut s = format!(
        "# 作成中メール\n宛先: {}\n件名: {}\n本文: {}\n\n# 関連する過去メール\n",
        to.join(", "),
        subject,
        head(body, 800),
    );
    if related.is_empty() {
        s.push_str("(なし)\n");
    }
    for (i, (date, from, subj, body)) in related.iter().enumerate() {
        s.push_str(&format!(
            "## {}. {} / {} / 件名: {}\n{}\n\n",
            i + 1,
            date,
            from,
            subj,
            head(body, 800),
        ));
    }
    s
}

// ---- C: 返信ドラフト生成 ----

/// トーン・長さから system プロンプトを組む。
pub fn reply_system(tone: &str, length: &str) -> String {
    let tone_ja = match tone {
        "casual" => "カジュアル(砕けすぎない範囲で親しみやすく)",
        "formal" => "丁寧(かしこまったビジネス敬語)",
        _ => "標準的なビジネス敬語",
    };
    let length_ja = match length {
        "short" => "短め(要点のみ、3〜4文)",
        "long" => "長め(丁寧に、必要な背景も添える)",
        _ => "標準的な長さ",
    };
    format!(
        "あなたは日本語ビジネスメールの返信を作成するアシスタントです。\
受信メールの要件に過不足なく対応する返信本文を作成してください。\
件名は書かず本文のみ。宛名と結びの挨拶は含めてよい。\
トーン={tone_ja}。長さ={length_ja}。事実を捏造しない。"
    )
}

/// C の入力: 元メール(差出人/件名/本文 2000 字上限)。
pub fn reply_input(from: &str, subject: &str, body: &str) -> String {
    format!(
        "以下の受信メールへの返信を書いてください。\n\n差出人: {}\n件名: {}\n本文:\n{}",
        from,
        subject,
        head(body, 2000),
    )
}

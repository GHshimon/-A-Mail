//! IMAP modified UTF-7(RFC 3501 §5.1.3)のデコード。
//!
//! メールボックス名は ASCII 印字可能文字はそのまま、それ以外は modified UTF-7 で
//! 表現される。通常の UTF-7 との差分:
//! - シフト文字は `+` ではなく `&`。`&-` は素の `&`。
//! - Base64 のアルファベットは `/` の代わりに `,` を使う。
//! - Base64 が符号化するのは UTF-16BE のバイト列。
//!
//! 例: `&MLQw33ux-` → 「ゴミ箱」。

/// modified UTF-7 のメールボックス名を UTF-8 文字列へデコードする。
/// 不正な入力は可能な範囲でそのまま素通しし、パニックしない。
pub fn decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b != b'&' {
            out.push(b as char);
            i += 1;
            continue;
        }
        // `&` を検出。次が `-` なら素の `&`。
        if i + 1 < bytes.len() && bytes[i + 1] == b'-' {
            out.push('&');
            i += 2;
            continue;
        }
        // `&` から次の `-`(終端)までを Base64(modified)として取り込む。
        let start = i + 1;
        let mut j = start;
        while j < bytes.len() && bytes[j] != b'-' {
            j += 1;
        }
        let chunk = &input[start..j];
        match decode_b64_utf16(chunk) {
            Some(s) => out.push_str(&s),
            // デコード不能なら元表現を温存(化けても情報を失わない)。
            None => {
                out.push('&');
                out.push_str(chunk);
                if j < bytes.len() {
                    out.push('-');
                }
            }
        }
        // 終端の `-` を読み飛ばす。
        i = if j < bytes.len() { j + 1 } else { j };
    }
    out
}

/// modified base64(`,`→`/`)を UTF-16BE とみなしてデコード。
fn decode_b64_utf16(chunk: &str) -> Option<String> {
    if chunk.is_empty() {
        return None;
    }
    let standard: String = chunk.chars().map(|c| if c == ',' { '/' } else { c }).collect();
    let bytes = base64_decode(&standard)?;
    if bytes.len() % 2 != 0 {
        return None;
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16(&units).ok()
}

/// パディング無し標準 Base64 のデコード(依存を増やさない最小実装)。
fn base64_decode(s: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let mut out = Vec::new();
    let mut acc: u32 = 0;
    let mut nbits = 0;
    for &c in s.as_bytes() {
        if c == b'=' {
            break;
        }
        let v = val(c)?;
        acc = (acc << 6) | v;
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push((acc >> nbits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn decodes_gmail_japanese_folders() {
        assert_eq!(decode("&MLQw33ux-"), "ゴミ箱");
        assert_eq!(decode("&Tgtm+DBN-"), "下書き");
        assert_eq!(decode("[Gmail]/&kAFP4W4IMH8w4TD8MOs-"), "[Gmail]/送信済みメール");
    }

    #[test]
    fn passes_ascii_through() {
        assert_eq!(decode("INBOX"), "INBOX");
        assert_eq!(decode("[Gmail]/SONY"), "[Gmail]/SONY");
    }

    #[test]
    fn literal_ampersand() {
        assert_eq!(decode("A&-B"), "A&B");
    }
}

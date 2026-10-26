#!/usr/bin/env python3
"""Build the smartphone-first A-mail actual-UI feature guide PDF."""

from __future__ import annotations

from pathlib import Path
from typing import Iterable

from PIL import Image
from reportlab.lib.colors import Color, HexColor, white
from reportlab.lib.utils import ImageReader
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.pdfgen import canvas


ROOT = Path(__file__).resolve().parent
SCREENS = ROOT / "screens"
OUTPUT = ROOT / "A-mail_実画面_機能説明資料.pdf"

PAGE_W = 540
PAGE_H = 960
MARGIN = 28

NAVY = HexColor("#10233F")
INK = HexColor("#21324A")
MUTED = HexColor("#617087")
BLUE = HexColor("#3C63F3")
BLUE_SOFT = HexColor("#EAF0FF")
TEAL = HexColor("#0E9F91")
TEAL_SOFT = HexColor("#E5F7F3")
AMBER = HexColor("#B66A00")
AMBER_SOFT = HexColor("#FFF3D9")
RED = HexColor("#C2414B")
RED_SOFT = HexColor("#FDECEF")
PAPER = HexColor("#F5F7FB")
LINE = HexColor("#D9E0EA")
WHITE = white


def register_fonts() -> tuple[str, str]:
    # Hiragino on current macOS uses PostScript outlines inside TTC, which
    # ReportLab cannot embed. Arial Unicode is a TrueType CJK-capable fallback.
    fallback = Path("/System/Library/Fonts/Supplemental/Arial Unicode.ttf")
    if fallback.exists():
        pdfmetrics.registerFont(TTFont("GuideJP", str(fallback)))
        pdfmetrics.registerFont(TTFont("GuideJP-Bold", str(fallback)))
    else:
        raise FileNotFoundError("Japanese font not found")
    return "GuideJP", "GuideJP-Bold"


FONT, FONT_BOLD = register_fonts()


def rounded_rect(c: canvas.Canvas, x: float, y: float, w: float, h: float,
                 fill: Color, stroke: Color | None = None, radius: float = 12,
                 line_width: float = 1) -> None:
    c.saveState()
    c.setFillColor(fill)
    if stroke:
        c.setStrokeColor(stroke)
        c.setLineWidth(line_width)
        c.roundRect(x, y, w, h, radius, fill=1, stroke=1)
    else:
        c.roundRect(x, y, w, h, radius, fill=1, stroke=0)
    c.restoreState()


def wrap_text(text: str, font: str, size: float, max_width: float) -> list[str]:
    lines: list[str] = []
    for paragraph in text.split("\n"):
        if not paragraph:
            lines.append("")
            continue
        current = ""
        for ch in paragraph:
            trial = current + ch
            if current and pdfmetrics.stringWidth(trial, font, size) > max_width:
                lines.append(current)
                current = ch
            else:
                current = trial
        if current:
            lines.append(current)
    return lines


def draw_text(c: canvas.Canvas, text: str, x: float, y: float, width: float,
              size: float = 13, color: Color = INK, leading: float | None = None,
              font: str = FONT, max_lines: int | None = None) -> float:
    leading = leading or size * 1.55
    lines = wrap_text(text, font, size, width)
    if max_lines:
        lines = lines[:max_lines]
    c.setFont(font, size)
    c.setFillColor(color)
    for line in lines:
        c.drawString(x, y, line)
        y -= leading
    return y


def draw_chip(c: canvas.Canvas, text: str, x: float, y: float,
              fill: Color = BLUE_SOFT, color: Color = BLUE,
              font_size: float = 10) -> float:
    width = pdfmetrics.stringWidth(text, FONT_BOLD, font_size) + 18
    rounded_rect(c, x, y - 4, width, 24, fill, None, 12)
    c.setFont(FONT_BOLD, font_size)
    c.setFillColor(color)
    c.drawString(x + 9, y + 3, text)
    return width


def draw_chip_right(c: canvas.Canvas, text: str, right: float, y: float,
                    fill: Color = BLUE_SOFT, color: Color = BLUE,
                    font_size: float = 9, max_width: float = 160) -> float:
    size = font_size
    width = pdfmetrics.stringWidth(text, FONT_BOLD, size) + 18
    while width > max_width and size > 7:
        size -= 0.5
        width = pdfmetrics.stringWidth(text, FONT_BOLD, size) + 18
    draw_chip(c, text, right - width, y, fill, color, size)
    return width


def draw_title(c: canvas.Canvas, section: str, title: str, page_no: int,
               status: str = "実画面 / 架空データ") -> float:
    c.setFillColor(PAPER)
    c.rect(0, 0, PAGE_W, PAGE_H, fill=1, stroke=0)
    c.setFillColor(BLUE)
    c.rect(0, PAGE_H - 8, PAGE_W, 8, fill=1, stroke=0)
    c.setFont(FONT_BOLD, 11)
    c.setFillColor(BLUE)
    c.drawString(MARGIN, PAGE_H - 39, section)
    title_size = 28.0
    while pdfmetrics.stringWidth(title, FONT_BOLD, title_size) > PAGE_W - 2 * MARGIN and title_size > 22:
        title_size -= 0.5
    c.setFont(FONT_BOLD, title_size)
    c.setFillColor(NAVY)
    c.drawString(MARGIN, PAGE_H - 82, title)
    draw_chip_right(c, status, PAGE_W - MARGIN, PAGE_H - 48, TEAL_SOFT, TEAL, 9)
    draw_footer(c, page_no)
    return PAGE_H - 103


def draw_footer(c: canvas.Canvas, page_no: int) -> None:
    c.setStrokeColor(LINE)
    c.setLineWidth(0.7)
    c.line(MARGIN, 24, PAGE_W - MARGIN, 24)
    c.setFont(FONT, 8)
    c.setFillColor(MUTED)
    c.drawString(MARGIN, 10, "A-mail 機能説明ガイド｜実画面・架空データ")
    c.drawRightString(PAGE_W - MARGIN, 10, f"{page_no:02d}")


def draw_image(c: canvas.Canvas, filename: str, x: float, y_top: float,
               max_w: float, max_h: float, caption: str | None = None,
               align: str = "center") -> tuple[float, float, float]:
    path = SCREENS / filename
    with Image.open(path) as im:
        iw, ih = im.size
    scale = min(max_w / iw, max_h / ih)
    w, h = iw * scale, ih * scale
    if align == "center":
        image_x = x + (max_w - w) / 2
    elif align == "right":
        image_x = x + max_w - w
    else:
        image_x = x
    image_y = y_top - h
    rounded_rect(c, image_x - 5, image_y - 5, w + 10, h + 10, WHITE, LINE, 10)
    c.drawImage(ImageReader(str(path)), image_x, image_y, width=w, height=h,
                preserveAspectRatio=True, mask="auto")
    if caption:
        c.setFont(FONT, 8)
        c.setFillColor(MUTED)
        c.drawString(image_x, image_y - 16, caption)
    return image_x, image_y, h


def draw_callout(c: canvas.Canvas, title: str, body: str, x: float, y_top: float,
                 w: float, fill: Color = WHITE, accent: Color = BLUE,
                 body_size: float = 11.5) -> float:
    body_lines = wrap_text(body, FONT, body_size, w - 30)
    h = 47 + len(body_lines) * (body_size * 1.5)
    rounded_rect(c, x, y_top - h, w, h, fill, LINE, 12)
    c.setFillColor(accent)
    c.roundRect(x, y_top - h, 5, h, 2.5, fill=1, stroke=0)
    c.setFont(FONT_BOLD, 12)
    c.setFillColor(NAVY)
    c.drawString(x + 16, y_top - 24, title)
    draw_text(c, body, x + 16, y_top - 46, w - 30, body_size, INK, body_size * 1.5)
    return y_top - h


def draw_numbered(c: canvas.Canvas, items: Iterable[tuple[str, str]], x: float,
                  y_top: float, width: float, gap: float = 13) -> float:
    y = y_top
    for idx, (title, body) in enumerate(items, start=1):
        c.setFillColor(BLUE)
        c.circle(x + 12, y - 12, 12, fill=1, stroke=0)
        c.setFont(FONT_BOLD, 10)
        c.setFillColor(WHITE)
        c.drawCentredString(x + 12, y - 16, str(idx))
        c.setFont(FONT_BOLD, 12)
        c.setFillColor(NAVY)
        c.drawString(x + 33, y - 8, title)
        body_y = draw_text(c, body, x + 33, y - 28, width - 33, 10.5, MUTED, 15)
        y = body_y - gap
    return y


def new_page(c: canvas.Canvas) -> None:
    c.showPage()


def page_cover(c: canvas.Canvas) -> None:
    c.setFillColor(NAVY)
    c.rect(0, 0, PAGE_W, PAGE_H, fill=1, stroke=0)
    c.setFillColor(BLUE)
    c.circle(PAGE_W - 15, PAGE_H - 55, 155, fill=1, stroke=0)
    c.setFillColor(HexColor("#27466D"))
    c.circle(45, 70, 130, fill=1, stroke=0)
    draw_chip(c, "実画面", MARGIN, PAGE_H - 98, TEAL_SOFT, TEAL, 11)
    draw_chip(c, "架空データ", MARGIN + 82, PAGE_H - 98, AMBER_SOFT, AMBER, 11)
    c.setFont(FONT_BOLD, 39)
    c.setFillColor(WHITE)
    c.drawString(MARGIN, PAGE_H - 160, "A-mail")
    c.setFont(FONT_BOLD, 29)
    c.drawString(MARGIN, PAGE_H - 201, "実画面 機能説明ガイド")
    c.setFont(FONT, 15)
    c.setFillColor(HexColor("#C7D5E8"))
    c.drawString(MARGIN, PAGE_H - 235, "受信から、確認型AI下書きまで")
    draw_image(c, "01-inbox.png", MARGIN + 12, PAGE_H - 285, PAGE_W - 80, 425,
               "A-mail React UI / browser demo provider")
    rounded_rect(c, MARGIN, 58, PAGE_W - 2 * MARGIN, 82, HexColor("#182F50"), HexColor("#385476"), 14)
    c.setFont(FONT_BOLD, 12)
    c.setFillColor(WHITE)
    c.drawString(MARGIN + 18, 112, "この資料の前提")
    draw_text(c,
              "掲載画面は実際のA-mail React UIです。表示データはすべて架空で、実メール・外部AI・SMTP・認証は使用していません。",
              MARGIN + 18, 91, PAGE_W - 2 * MARGIN - 36, 10.5,
              HexColor("#DCE7F5"), 15)
    c.setFont(FONT, 8)
    c.setFillColor(HexColor("#AABCD3"))
    c.drawRightString(PAGE_W - MARGIN, 22, "2026.09.10")


def page_scope(c: canvas.Canvas, page_no: int) -> None:
    y = draw_title(c, "GUIDE", "この資料の読み方", page_no)
    y = draw_text(c,
                  "画面に存在する機能と、今後の実装・実機受入が必要な項目を混同しないよう、状態を明示しています。",
                  MARGIN, y, PAGE_W - 2 * MARGIN, 13, INK, 20)
    y -= 14
    boxes = [
        ("確認済み", "ブラウザデモ上で操作し、画面遷移または表示を確認。", TEAL_SOFT, TEAL),
        ("未実装", "仕様はあるが、現在の製品コードにはまだ入っていない。", RED_SOFT, RED),
        ("未確認", "実装コードはあるが、実アカウントや配布アプリでの受入が未完了。", AMBER_SOFT, AMBER),
    ]
    for title, body, fill, accent in boxes:
        rounded_rect(c, MARGIN, y - 82, PAGE_W - 2 * MARGIN, 70, fill, None, 12)
        draw_chip(c, title, MARGIN + 14, y - 47, WHITE, accent, 10)
        draw_text(c, body, MARGIN + 105, y - 29, PAGE_W - 2 * MARGIN - 124, 10.5, INK, 15)
        y -= 86
    y -= 4
    c.setFont(FONT_BOLD, 15)
    c.setFillColor(NAVY)
    c.drawString(MARGIN, y, "基本フロー")
    y -= 26
    draw_numbered(c, [
        ("受信・検索", "必要なメールを探し、本文と会話の前提を読む。"),
        ("範囲確認", "顧客・案件・参照資料の境界をID単位で確定する。"),
        ("候補生成", "AIは返信候補と根拠を提示し、本文を勝手に上書きしない。"),
        ("本人確認", "候補を本文へ反映し、必要なら修正して、人が送信する。"),
    ], MARGIN, y, PAGE_W - 2 * MARGIN)
    rounded_rect(c, MARGIN, 54, PAGE_W - 2 * MARGIN, 75, WHITE, LINE, 12)
    draw_text(c,
              "安全原則：AIによる自動送信は行わず、送信責任は常に利用者が持ちます。アプリ終了後は処理しません。",
              MARGIN + 16, 105, PAGE_W - 2 * MARGIN - 32, 11.5, NAVY, 17, FONT_BOLD)


def page_inbox(c: canvas.Canvas, page_no: int) -> None:
    y = draw_title(c, "01 / MAIL", "受信トレイと検索", page_no)
    draw_image(c, "01-inbox.png", MARGIN, y, 300, 282, "受信トレイ")
    draw_image(c, "03-mail-search.png", 340, y, 172, 282, "検索結果")
    y -= 320
    draw_callout(c, "一覧で状況を把握", "差出人、件名、受信時刻、スター、既読状態を一画面で確認できます。デモでは問い合わせ・請求書・会議資料など複数種のメールを用意しています。",
                 MARGIN, y, PAGE_W - 2 * MARGIN, WHITE, BLUE)
    y -= 130
    draw_callout(c, "メール検索", "検索欄から対象メールを絞り込みます。返信生成の前に、人が対象メールを選べる入口です。",
                 MARGIN, y, PAGE_W - 2 * MARGIN, WHITE, TEAL)
    rounded_rect(c, MARGIN, 53, PAGE_W - 2 * MARGIN, 62, BLUE_SOFT, None, 12)
    draw_text(c, "確認済み：一覧表示、メール選択、検索表示をブラウザデモで確認。",
              MARGIN + 16, 91, PAGE_W - 2 * MARGIN - 32, 10.5, BLUE, 16, FONT_BOLD)


def page_reader(c: canvas.Canvas, page_no: int) -> None:
    y = draw_title(c, "02 / READ", "メールを読み、返信へ", page_no)
    draw_image(c, "02-message-reader.png", MARGIN, y, PAGE_W - 2 * MARGIN, 470,
               "選択したメールの本文画面")
    y -= 510
    draw_numbered(c, [
        ("本文を確認", "受信内容を読み、返信が必要かを人が判断します。"),
        ("返信を開始", "返信ボタンから作成画面へ移り、元メールを参照した状態で作業します。"),
        ("送信はまだ行わない", "この段階では候補生成も送信も発生しません。"),
    ], MARGIN, y, PAGE_W - 2 * MARGIN)
    rounded_rect(c, MARGIN, 55, PAGE_W - 2 * MARGIN, 70, TEAL_SOFT, None, 12)
    draw_text(c, "設計意図：『受信したら即AI』ではなく、ユーザーが対象を選ぶ導線を残します。",
              MARGIN + 16, 99, PAGE_W - 2 * MARGIN - 32, 11, TEAL, 17, FONT_BOLD)


def page_compose(c: canvas.Canvas, page_no: int) -> None:
    y = draw_title(c, "03 / COMPOSE", "返信作成とAI設定", page_no)
    draw_image(c, "04-context-confirmed.png", MARGIN, y, PAGE_W - 2 * MARGIN, 470,
               "返信作成画面：本文とAI補助を分離")
    y -= 510
    draw_callout(c, "人が入力する領域", "宛先・件名・本文は通常のメール作成欄として扱います。生成後も直接編集できます。",
                 MARGIN, y, PAGE_W - 2 * MARGIN, WHITE, BLUE)
    y -= 116
    draw_callout(c, "AIへ渡す条件", "返信トーン、文章量、From/To、回答方針、元メールを個別に確認してから候補生成へ進みます。",
                 MARGIN, y, PAGE_W - 2 * MARGIN, WHITE, TEAL)
    rounded_rect(c, MARGIN, 50, PAGE_W - 2 * MARGIN, 70, AMBER_SOFT, None, 12)
    draw_text(c, "重要：返信候補とメール本文は別状態。候補生成だけでは本文を変更しません。",
              MARGIN + 16, 94, PAGE_W - 2 * MARGIN - 32, 11, AMBER, 17, FONT_BOLD)


def page_context(c: canvas.Canvas, page_no: int) -> None:
    y = draw_title(c, "04 / CONTEXT", "顧客・案件の境界を確認", page_no)
    draw_image(c, "15-context-confirmed-panel.png", MARGIN, y, 250, 670,
               "AI補助パネル：参照範囲の確認")
    x = 304
    yy = y - 4
    yy = draw_text(c,
                   "メールアドレスだけに依存せず、内部IDで対象を束ねます。返信先ごとの情報混在を防ぐため、生成前に参照範囲を可視化します。",
                   x, yy, PAGE_W - MARGIN - x, 12, INK, 18)
    yy -= 15
    draw_callout(c, "Customer ID", "誰とのやり取りかを識別。別顧客の履歴は原則参照しません。",
                 x, yy, PAGE_W - MARGIN - x, WHITE, BLUE, 10.5)
    yy -= 128
    draw_callout(c, "Workspace / Project ID", "同じ顧客でも、案件・製品・契約単位で文脈を分離します。",
                 x, yy, PAGE_W - MARGIN - x, WHITE, TEAL, 10.5)
    yy -= 128
    draw_callout(c, "Source scope", "参照メール・資料を列挙し、根拠のない横断参照を避けます。",
                 x, yy, PAGE_W - MARGIN - x, WHITE, AMBER, 10.5)
    rounded_rect(c, x, 86, PAGE_W - MARGIN - x, 110, RED_SOFT, None, 12)
    draw_text(c, "安全側の動作", x + 14, 172, PAGE_W - MARGIN - x - 28, 11, RED, 16, FONT_BOLD)
    draw_text(c, "不足や不一致があれば生成前に止め、対象を選び直します。",
              x + 14, 145, PAGE_W - MARGIN - x - 28, 10.5, INK, 16)


def page_candidate(c: canvas.Canvas, page_no: int) -> None:
    y = draw_title(c, "05 / DRAFT", "返信候補と根拠を提示", page_no)
    draw_image(c, "05-reply-candidate-panel.png", MARGIN, y, 250, 690,
               "生成された返信候補")
    x = 304
    yy = y - 3
    draw_chip(c, "候補状態", x, yy - 15, BLUE_SOFT, BLUE, 10)
    yy -= 50
    yy = draw_text(c,
                   "AIは返信文を『候補』として表示します。この時点では本文へ反映されず、送信もされません。",
                   x, yy, PAGE_W - MARGIN - x, 12, INK, 18)
    yy -= 14
    yy = draw_callout(c, "回答本文", "指定したトーン・文章量・スタイルプロファイルを使った案。",
                      x, yy, PAGE_W - MARGIN - x, WHITE, BLUE, 10.5) - 14
    yy = draw_callout(c, "根拠・参照元", "どのメールや資料を使ったかを併記し、確認可能にします。",
                      x, yy, PAGE_W - MARGIN - x, WHITE, TEAL, 10.5) - 14
    yy = draw_callout(c, "不足情報", "断定できない点は質問・注意として分離し、作り話を防ぎます。",
                      x, yy, PAGE_W - MARGIN - x, WHITE, AMBER, 10.5)
    rounded_rect(c, x, 74, PAGE_W - MARGIN - x, 96, TEAL_SOFT, None, 12)
    draw_text(c, "確認済み", x + 14, 145, PAGE_W - MARGIN - x - 28, 10.5, TEAL, 16, FONT_BOLD)
    draw_text(c, "候補生成、参照元表示、本文と候補の分離をデモで確認。",
              x + 14, 120, PAGE_W - MARGIN - x - 28, 10, INK, 15)


def page_apply(c: canvas.Canvas, page_no: int) -> None:
    y = draw_title(c, "06 / REVIEW", "確認して本文へ反映", page_no)
    draw_image(c, "06-applied-draft.png", MARGIN, y, PAGE_W - 2 * MARGIN, 465,
               "候補を本文へ反映した状態")
    y -= 505
    draw_numbered(c, [
        ("候補を読む", "内容、言い回し、根拠、不足情報を人が確認します。"),
        ("本文へ反映", "『確認して本文に反映』を押して初めてメール本文が更新されます。"),
        ("必要なら修正", "宛先・件名・本文を本人が編集し、本人の責任で送信します。"),
    ], MARGIN, y, PAGE_W - 2 * MARGIN)
    rounded_rect(c, MARGIN, 49, PAGE_W - 2 * MARGIN, 78, RED_SOFT, None, 12)
    draw_text(c, "送信ガード：AIが勝手に送る機能は採用しません。最終送信操作は必ず人が行います。",
              MARGIN + 16, 97, PAGE_W - 2 * MARGIN - 32, 11.2, RED, 17, FONT_BOLD)


def page_chatgpt(c: canvas.Canvas, page_no: int) -> None:
    y = draw_title(c, "07 / AI CONNECTION", "ChatGPT利用枠と自動下書き", page_no)
    draw_image(c, "07-chatgpt-connected-panel.png", MARGIN, y, 250, 685,
               "ChatGPT接続・自動下書き設定")
    x = 304
    yy = y
    yy = draw_text(c,
                   "APIキー管理を前提にせず、利用者自身のChatGPT契約枠を使う方向のUIです。利用前に明示同意を求めます。",
                   x, yy, PAGE_W - MARGIN - x, 12, INK, 18)
    yy -= 12
    yy = draw_callout(c, "利用者負担", "サービス側のAI費用ではなく、本人の利用枠を消費する設計。",
                      x, yy, PAGE_W - MARGIN - x, WHITE, BLUE, 10.5) - 12
    yy = draw_callout(c, "自動下書き", "受信を契機に候補だけ作成。1日の上限を設定し、送信はしません。",
                      x, yy, PAGE_W - MARGIN - x, WHITE, TEAL, 10.5) - 12
    yy = draw_callout(c, "起動中のみ", "アプリ終了後に監視・生成を継続しない前提です。",
                      x, yy, PAGE_W - MARGIN - x, WHITE, AMBER, 10.5)
    rounded_rect(c, x, 75, PAGE_W - MARGIN - x, 120, AMBER_SOFT, None, 12)
    draw_text(c, "未確認", x + 14, 169, PAGE_W - MARGIN - x - 28, 10.5, AMBER, 16, FONT_BOLD)
    draw_text(c, "実ChatGPTログイン、実利用枠、配布アプリでの生成・終了時停止は実機受入が必要。",
              x + 14, 144, PAGE_W - MARGIN - x - 28, 10, INK, 15)


def page_style_learn(c: canvas.Canvas, page_no: int) -> None:
    y = draw_title(c, "08 / STYLE", "任意タイミングで文体を学習", page_no)
    draw_image(c, "08-style-learning-preview-panel.png", MARGIN, y, 252, 690,
               "学習前の対象・除外・送信範囲確認")
    x = 306
    yy = y
    yy = draw_text(c,
                   "生メールを毎回すべて渡すのではなく、直近100件または200件から文章の特徴を抽象化します。実行は常時ではなく、本人が押した時だけです。",
                   x, yy, PAGE_W - MARGIN - x, 12, INK, 18)
    yy -= 10
    draw_numbered(c, [
        ("対象を選択", "直近100件 / 200件を選ぶ。"),
        ("除外を確認", "定型署名、引用、個人情報などを除外。"),
        ("ローカル集計", "言葉遣い、長さ、改行、構造を抽象化。"),
        ("同意して保存", "確認後にプロファイルとして保持。"),
    ], x, yy, PAGE_W - MARGIN - x, 11)
    rounded_rect(c, x, 73, PAGE_W - MARGIN - x, 116, TEAL_SOFT, None, 12)
    draw_text(c, "画面上の安全表示", x + 14, 163, PAGE_W - MARGIN - x - 28, 10.5, TEAL, 16, FONT_BOLD)
    draw_text(c, "デモでは外部AI投入0件と表示。本文そのものではなく抽象化した特徴を保持します。",
              x + 14, 138, PAGE_W - MARGIN - x - 28, 10, INK, 15)


def page_style_edit(c: canvas.Canvas, page_no: int) -> None:
    y = draw_title(c, "09 / STYLE PROFILE", "本人が直せる文体プロファイル", page_no)
    draw_image(c, "09-style-profile-editor-panel.png", MARGIN, y, 231, 665,
               "抽象化した文体設定")
    draw_image(c, "10-style-profile-preview-panel.png", 281, y, 231, 665,
               "架空メールでのプレビュー")
    y -= 706
    rounded_rect(c, MARGIN, y - 112, PAGE_W - 2 * MARGIN, 112, WHITE, LINE, 12)
    draw_text(c, "学習結果はブラックボックスにしない", MARGIN + 16, y - 26,
              PAGE_W - 2 * MARGIN - 32, 12.5, NAVY, 18, FONT_BOLD)
    draw_text(c,
              "敬語の強さ、頻出表現、文章量、段落・改行、避ける言い回しを本人が編集できます。保存前に架空の問い合わせで返信例を確認できます。",
              MARGIN + 16, y - 54, PAGE_W - 2 * MARGIN - 32, 10.8, INK, 16)
    rounded_rect(c, MARGIN, 46, PAGE_W - 2 * MARGIN, 63, BLUE_SOFT, None, 12)
    draw_text(c, "付加価値：本人らしさを継続利用でき、毎回100〜200件のメールを読み直すトークン消費を抑えます。",
              MARGIN + 15, 85, PAGE_W - 2 * MARGIN - 30, 10.5, BLUE, 15, FONT_BOLD)


def page_project_context(c: canvas.Canvas, page_no: int) -> None:
    y = draw_title(c, "10 / AGENT CONTEXT", "案件知識とAIエージェント", page_no)
    draw_image(c, "12-new-compose-context-panel.png", MARGIN, y, 231, 645,
               "新規メールの一時プロジェクト")
    draw_image(c, "11-codex-cli-panel.png", 281, y, 231, 645,
               "Codex CLIツール設定")
    y = 250
    rounded_rect(c, MARGIN, y - 130, PAGE_W - 2 * MARGIN, 130, WHITE, LINE, 12)
    draw_text(c, "段階的に必要な文脈だけを渡す", MARGIN + 16, y - 26,
              PAGE_W - 2 * MARGIN - 32, 12.5, NAVY, 18, FONT_BOLD)
    draw_text(c,
              "新規メールでは一時プロジェクトを選び、返信では顧客・案件IDを引き継ぎます。専門エージェントは常時すべてを読むのではなく、選択したプロジェクトの許可済み情報だけを参照します。",
              MARGIN + 16, y - 54, PAGE_W - 2 * MARGIN - 32, 10.6, INK, 16)
    rounded_rect(c, MARGIN, 43, PAGE_W - 2 * MARGIN, 62, AMBER_SOFT, None, 12)
    draw_text(c, "現状：Codex CLIは手動実行・読み取り専用のUI。実タスク実行はこの資料では行っていません。",
              MARGIN + 15, 81, PAGE_W - 2 * MARGIN - 30, 10.5, AMBER, 15, FONT_BOLD)


def page_accounts(c: canvas.Canvas, page_no: int) -> None:
    y = draw_title(c, "11 / MAIL CONNECTION", "現在のメールアカウント設定", page_no)
    draw_image(c, "13-account-settings.png", MARGIN, y, PAGE_W - 2 * MARGIN, 438,
               "現在のアカウント追加画面")
    y -= 480
    draw_callout(c, "現在の実装", "Gmail / iCloudを選び、メールアドレスとアプリパスワードを入力する画面です。保存前に接続テストを行う導線があります。",
                 MARGIN, y, PAGE_W - 2 * MARGIN, WHITE, BLUE)
    y -= 130
    draw_callout(c, "目標とする導線", "初心者でも扱いやすい『Googleで接続』のOAuth認証を採用し、アプリ側でパスワードを保管しない構成へ移行します。",
                 MARGIN, y, PAGE_W - 2 * MARGIN, WHITE, TEAL)
    rounded_rect(c, MARGIN, 49, PAGE_W - 2 * MARGIN, 82, RED_SOFT, None, 12)
    draw_text(c, "未実装：Gmail OAuth / XOAUTH2。現画面をOAuth接続済みと誤認しないでください。",
              MARGIN + 16, 100, PAGE_W - 2 * MARGIN - 32, 11.2, RED, 17, FONT_BOLD)


def page_legacy_ai(c: canvas.Canvas, page_no: int) -> None:
    y = draw_title(c, "12 / LEGACY SETTINGS", "既存のAI・プロジェクト設定", page_no)
    draw_image(c, "14-legacy-ai-provider-panel.png", MARGIN, y, 265, 665,
               "従来のGemini / OpenAI APIキー設定")
    x = 320
    yy = y
    yy = draw_text(c,
                   "従来UIにはGeminiまたはOpenAIのAPIキーを登録し、プロジェクト情報を設定する画面があります。これは現在存在する経路です。",
                   x, yy, PAGE_W - MARGIN - x, 12, INK, 18)
    yy -= 12
    yy = draw_callout(c, "位置づけ", "APIキーに慣れた利用者向けの任意・互換経路。",
                      x, yy, PAGE_W - MARGIN - x, WHITE, BLUE, 10.5) - 13
    yy = draw_callout(c, "推奨方向", "一般利用者はChatGPT接続を主導線にし、料金は本人の契約枠へ。",
                      x, yy, PAGE_W - MARGIN - x, WHITE, TEAL, 10.5) - 13
    yy = draw_callout(c, "統合課題", "二つの設定経路を整理し、どちらが利用中かを一画面で明示する。",
                      x, yy, PAGE_W - MARGIN - x, WHITE, AMBER, 10.5)
    rounded_rect(c, x, 74, PAGE_W - MARGIN - x, 108, BLUE_SOFT, None, 12)
    draw_text(c, "注意", x + 14, 156, PAGE_W - MARGIN - x - 28, 10.5, BLUE, 16, FONT_BOLD)
    draw_text(c, "このデモではAPIキー入力・外部AI呼び出しは行っていません。",
              x + 14, 131, PAGE_W - MARGIN - x - 28, 10, INK, 15)


def page_status(c: canvas.Canvas, page_no: int) -> None:
    y = draw_title(c, "13 / RELEASE VIEW", "現状と次の受入項目", page_no,
                   "実装・確認範囲を明示")
    y = draw_text(c,
                  "デモで見える完成度と、本番運用までに必要な作業を分けて整理します。",
                  MARGIN, y, PAGE_W - 2 * MARGIN, 13, INK, 20)
    y -= 12
    columns = [
        ("確認済み", TEAL, TEAL_SOFT, [
            "受信一覧・検索・本文表示",
            "返信作成と参照範囲確認",
            "候補・根拠・不足情報の表示",
            "候補を確認後に本文へ反映",
            "文体学習・編集・プレビューUI",
            "ChatGPT接続と上限設定UI",
        ]),
        ("未実装 / 未確認", RED, RED_SOFT, [
            "Gmail OAuth / XOAUTH2は未実装",
            "実ChatGPTログイン・利用枠連携",
            "配布アプリでの実返信生成",
            "アプリ終了時の確実な停止",
            "実Gmail受信・下書き・手動送信",
            "顧客混在を防ぐ実データ負荷試験",
        ]),
    ]
    for title, accent, fill, items in columns:
        h = 252
        rounded_rect(c, MARGIN, y - h, PAGE_W - 2 * MARGIN, h, fill, None, 14)
        draw_chip(c, title, MARGIN + 16, y - 36, WHITE, accent, 11)
        yy = y - 70
        for item in items:
            c.setFillColor(accent)
            c.circle(MARGIN + 24, yy + 4, 3.5, fill=1, stroke=0)
            yy = draw_text(c, item, MARGIN + 38, yy + 9, PAGE_W - 2 * MARGIN - 54,
                           10.8, INK, 17) - 3
        y -= h + 14
    rounded_rect(c, MARGIN, 96, PAGE_W - 2 * MARGIN, 90, AMBER_SOFT, None, 12)
    draw_text(c, "今回観測した注意点", MARGIN + 16, 160,
              PAGE_W - 2 * MARGIN - 32, 11, AMBER, 16, FONT_BOLD)
    draw_text(c,
              "参照範囲を連続変更して候補生成した際、一度だけ『下書きを保存できません』の警告を確認。候補は保持され、本文反映後は保存状態へ戻りました。再現試験が必要です。",
              MARGIN + 16, 136, PAGE_W - 2 * MARGIN - 32, 10.2, INK, 15)
    c.setFont(FONT_BOLD, 12)
    c.setFillColor(NAVY)
    c.drawCentredString(PAGE_W / 2, 58, "本番公開判断は、上記の実機受入後に行います。")


def build() -> None:
    c = canvas.Canvas(str(OUTPUT), pagesize=(PAGE_W, PAGE_H), pageCompression=1)
    c.setTitle("A-mail 実画面 機能説明資料")
    c.setAuthor("A-mail project")
    c.setSubject("Actual React UI feature guide using fictional demo data")

    page_cover(c)
    pages = [
        page_scope,
        page_inbox,
        page_reader,
        page_compose,
        page_context,
        page_candidate,
        page_apply,
        page_chatgpt,
        page_style_learn,
        page_style_edit,
        page_project_context,
        page_accounts,
        page_legacy_ai,
        page_status,
    ]
    for page_no, fn in enumerate(pages, start=2):
        new_page(c)
        fn(c, page_no)
    c.save()
    print(OUTPUT)


if __name__ == "__main__":
    build()

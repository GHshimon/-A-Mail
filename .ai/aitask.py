#!/usr/bin/env python3
"""作業リスト: 誰が何に着手しているかを共有し、編集範囲の衝突を防ぐ。

オーナー / Claude / Codex が同じリポジトリを並行して触るため、
「いま誰がどのファイルを触っているか」が分からないと同じ場所を壊し合う。
判断ログ(.ai/decisions.jsonl)が「なぜそうしたか」を残すのに対して、
こちらは「いま誰が何をしているか」を残す。

保存先は .ai/tasks.jsonl(追記専用)。worktree はすべて本体チェックアウトの
配下にあるので、上へ辿れば全員が同じファイルに行き着く。追記専用なので
同時に書いても行が壊れない(flock で ID の重複も防ぐ)。

    # 着手前に、触る範囲が他の人と重なっていないか確認する
    aitask.py check --areas src-tauri/src/imap,src/store

    # 登録して着手する
    aitask.py add --title "IMAP の差分同期" --areas src-tauri/src/imap
    aitask.py claim t0007 --actor claude --branch claude/xxx

    # 進捗・完了
    aitask.py note t0007 --note "接続プールまで実装"
    aitask.py done t0007 --refs 662ce44

    # 一覧と、人が読む用の書き出し
    aitask.py list
    aitask.py render
"""

from __future__ import annotations

import argparse
import errno
import fcntl
import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

LOG_DIR = ".ai"
LOG_NAME = "tasks.jsonl"
RENDER_NAME = "TASKS.md"

# 状態を変える種別と、その結果の状態。
KIND_STATUS = {
    "add": "open",
    "claim": "active",
    "note": None,  # 状態は変えない
    "block": "blocked",
    "done": "done",
    "drop": "open",  # 手放す。誰でも取れる状態に戻す
    "cancel": "cancelled",
}
OPEN_STATUSES = {"open", "active", "blocked"}


def now_iso() -> str:
    return datetime.now(timezone.utc).astimezone().isoformat(timespec="seconds")


def find_root(start: Path | None = None) -> Path:
    """.ai/ を持つ一番近い親を探す。

    worktree の中から実行しても本体の .ai/ に行き着くので、全員が同じ
    リストを見る。見つからなければ git の作業ツリー最上位に作る。
    """
    cur = (start or Path.cwd()).resolve()
    for d in [cur, *cur.parents]:
        if (d / LOG_DIR).is_dir():
            return d
    try:
        out = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            capture_output=True,
            text=True,
            check=True,
        )
        return Path(out.stdout.strip())
    except Exception:
        return cur


def log_path(root: Path) -> Path:
    return root / LOG_DIR / LOG_NAME


def read_events(path: Path) -> list[dict]:
    if not path.exists():
        return []
    events = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            events.append(json.loads(line))
        except json.JSONDecodeError:
            # 壊れた行があっても全体を止めない(追記専用なので後続は読める)。
            print(f"warning: 読めない行を飛ばしました: {line[:60]}", file=sys.stderr)
    return events


def append_event(path: Path, event: dict) -> None:
    """flock を取って追記する(ID 採番と書き込みを一体にする)。"""
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "a+", encoding="utf-8") as f:
        fcntl.flock(f.fileno(), fcntl.LOCK_EX)
        try:
            f.write(json.dumps(event, ensure_ascii=False) + "\n")
            f.flush()
            os.fsync(f.fileno())
        finally:
            fcntl.flock(f.fileno(), fcntl.LOCK_UN)


def next_task_id(events: list[dict]) -> str:
    n = 0
    for e in events:
        tid = e.get("task", "")
        if tid.startswith("t") and tid[1:].isdigit():
            n = max(n, int(tid[1:]))
    return f"t{n + 1:04d}"


def build_state(events: list[dict]) -> dict[str, dict]:
    """イベントを再生して、タスクごとの現在の状態を作る。"""
    tasks: dict[str, dict] = {}
    for e in events:
        tid = e.get("task")
        if not tid:
            continue
        t = tasks.setdefault(
            tid,
            {
                "id": tid,
                "title": "",
                "areas": [],
                "status": "open",
                "owner": None,
                "branch": None,
                "created": e.get("ts"),
                "updated": e.get("ts"),
                "refs": [],
                "history": [],
            },
        )
        if e.get("title"):
            t["title"] = e["title"]
        if e.get("areas"):
            t["areas"] = e["areas"]
        kind = e.get("kind", "")
        status = KIND_STATUS.get(kind)
        if status:
            t["status"] = status
        if kind == "claim":
            t["owner"] = e.get("actor")
            t["branch"] = e.get("branch")
        elif kind in ("drop", "cancel"):
            t["owner"] = None
            t["branch"] = None
        elif kind == "done":
            # 完了時は誰がやったかを残す(所有権は外す)。
            t["owner"] = t["owner"] or e.get("actor")
        for r in e.get("refs") or []:
            if r not in t["refs"]:
                t["refs"].append(r)
        t["updated"] = e.get("ts", t["updated"])
        t["history"].append(e)
    return tasks


def areas_overlap(a: str, b: str) -> bool:
    """パスとして重なるか(どちらかがもう一方の配下でも重なりとみなす)。"""
    a = a.strip().strip("/")
    b = b.strip().strip("/")
    if not a or not b:
        return False
    if a == b:
        return True
    return a.startswith(b + "/") or b.startswith(a + "/")


def split_areas(value: str | None) -> list[str]:
    if not value:
        return []
    return [p.strip().strip("/") for p in value.split(",") if p.strip()]


def default_actor() -> str:
    return os.environ.get("AI_ACTOR", "unknown")


def current_branch() -> str | None:
    try:
        out = subprocess.run(
            ["git", "rev-parse", "--abbrev-ref", "HEAD"],
            capture_output=True,
            text=True,
            check=True,
        )
        return out.stdout.strip() or None
    except Exception:
        return None


# ------------------------------------------------------------------ コマンド


def cmd_add(args, root: Path) -> int:
    path = log_path(root)
    events = read_events(path)
    tid = next_task_id(events)
    append_event(
        path,
        {
            "ts": now_iso(),
            "task": tid,
            "kind": "add",
            "actor": args.actor,
            "title": args.title,
            "areas": split_areas(args.areas),
            "note": args.note,
        },
    )
    print(tid)
    return 0


def _touch(args, root: Path, kind: str) -> int:
    path = log_path(root)
    tasks = build_state(read_events(path))
    if args.task not in tasks:
        print(f"error: {args.task} は登録されていません", file=sys.stderr)
        return 2
    event = {
        "ts": now_iso(),
        "task": args.task,
        "kind": kind,
        "actor": args.actor,
        "note": args.note,
    }
    if kind == "claim":
        event["branch"] = args.branch or current_branch()
    if getattr(args, "refs", None):
        event["refs"] = [r.strip() for r in args.refs.split(",") if r.strip()]
    append_event(path, event)
    print(f"{args.task}: {kind}")
    return 0


def cmd_check(args, root: Path) -> int:
    """着手予定の範囲が、他の人の作業中タスクと重なっていないか調べる。"""
    wanted = split_areas(args.areas)
    if not wanted:
        print("error: --areas を指定してください", file=sys.stderr)
        return 2

    tasks = build_state(read_events(log_path(root)))
    hits = []
    for t in tasks.values():
        if t["status"] not in ("active", "blocked"):
            continue
        if args.actor and t["owner"] == args.actor:
            continue  # 自分の作業は衝突ではない
        overlap = [
            (w, a) for w in wanted for a in t["areas"] if areas_overlap(w, a)
        ]
        if overlap:
            hits.append((t, overlap))

    if not hits:
        print("重なりはありません")
        return 0

    print("重なっている作業があります:")
    for t, overlap in hits:
        who = t["owner"] or "(担当未設定)"
        branch = f" / {t['branch']}" if t.get("branch") else ""
        print(f"  {t['id']} [{t['status']}] {who}{branch}: {t['title']}")
        for w, a in overlap:
            print(f"      {w}  <->  {a}")
    return 1


STATUS_MARK = {
    "open": "・",
    "active": "▶",
    "blocked": "■",
    "done": "✓",
    "cancelled": "×",
}


def cmd_list(args, root: Path) -> int:
    tasks = build_state(read_events(log_path(root)))
    rows = sorted(tasks.values(), key=lambda t: t["id"])
    if args.status == "open":
        rows = [t for t in rows if t["status"] in OPEN_STATUSES]
    elif args.status != "all":
        rows = [t for t in rows if t["status"] == args.status]
    if args.actor:
        rows = [t for t in rows if t["owner"] == args.actor]

    if not rows:
        print("該当する作業はありません")
        return 0
    for t in rows:
        who = t["owner"] or "-"
        mark = STATUS_MARK.get(t["status"], "?")
        print(f"{mark} {t['id']} [{t['status']:<9}] {who:<8} {t['title']}")
        if t["areas"]:
            print(f"        範囲: {', '.join(t['areas'])}")
    return 0


def cmd_show(args, root: Path) -> int:
    tasks = build_state(read_events(log_path(root)))
    t = tasks.get(args.task)
    if not t:
        print(f"error: {args.task} は登録されていません", file=sys.stderr)
        return 2
    print(f"{t['id']} [{t['status']}] {t['title']}")
    print(f"  担当  : {t['owner'] or '-'}{'  / ' + t['branch'] if t.get('branch') else ''}")
    print(f"  範囲  : {', '.join(t['areas']) or '-'}")
    print(f"  参照  : {', '.join(t['refs']) or '-'}")
    print("  履歴  :")
    for e in t["history"]:
        note = f" — {e['note']}" if e.get("note") else ""
        print(f"    {e.get('ts','')[:16]} {e.get('kind',''):<6} {e.get('actor') or '-'}{note}")
    return 0


def cmd_render(args, root: Path) -> int:
    """人が読む用に Markdown を書き出す(常に jsonl から作り直す)。"""
    tasks = build_state(read_events(log_path(root)))
    rows = sorted(tasks.values(), key=lambda t: t["id"])
    out_path = Path(args.out) if args.out else root / LOG_DIR / RENDER_NAME

    lines = [
        "# 作業リスト",
        "",
        "`.ai/tasks.jsonl` から生成。直接編集しても次の render で消える。",
        "更新は `scripts/aitask.py`(共有コピー: `.ai/aitask.py`)から行う。",
        "",
        f"最終更新: {now_iso()[:16]}",
        "",
    ]
    for status, heading in [
        ("active", "作業中"),
        ("blocked", "止まっている"),
        ("open", "未着手"),
        ("done", "完了"),
        ("cancelled", "取りやめ"),
    ]:
        group = [t for t in rows if t["status"] == status]
        if not group:
            continue
        lines += [f"## {heading}", "", "| ID | 担当 | 作業 | 範囲 |", "|---|---|---|---|"]
        for t in group:
            areas = "<br>".join(t["areas"]) or "-"
            who = t["owner"] or "-"
            lines.append(f"| {t['id']} | {who} | {t['title']} | {areas} |")
        lines.append("")

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(out_path)
    return 0


def main(argv=None) -> int:
    p = argparse.ArgumentParser(
        prog="aitask.py",
        description="作業リスト(誰が何に着手しているかの共有)",
    )
    sub = p.add_subparsers(dest="cmd", required=True)

    def add_common(sp, *, note=True):
        sp.add_argument("--actor", default=default_actor(), help="claude / codex / owner")
        if note:
            sp.add_argument("--note", default=None, help="ひとこと")

    sp = sub.add_parser("add", help="作業を登録する")
    sp.add_argument("--title", required=True)
    sp.add_argument("--areas", default=None, help="触るパスをカンマ区切りで")
    add_common(sp)
    sp.set_defaults(func=cmd_add)

    for kind, help_text in [
        ("claim", "着手する(担当を自分にする)"),
        ("note", "進捗を書く"),
        ("block", "止まっていることを記録する"),
        ("done", "完了にする"),
        ("drop", "手放す(未着手へ戻す)"),
        ("cancel", "取りやめる"),
    ]:
        sp = sub.add_parser(kind, help=help_text)
        sp.add_argument("task")
        add_common(sp)
        if kind == "claim":
            sp.add_argument("--branch", default=None)
        if kind == "done":
            sp.add_argument("--refs", default=None, help="コミット等をカンマ区切りで")
        sp.set_defaults(func=lambda a, r, k=kind: _touch(a, r, k))

    sp = sub.add_parser("check", help="触る範囲が他の人と重なっていないか調べる")
    sp.add_argument("--areas", required=True)
    sp.add_argument("--actor", default=default_actor())
    sp.set_defaults(func=cmd_check)

    sp = sub.add_parser("list", help="一覧")
    sp.add_argument("--status", default="open",
                    choices=["all", "open", "active", "blocked", "done", "cancelled"])
    sp.add_argument("--actor", default=None)
    sp.set_defaults(func=cmd_list)

    sp = sub.add_parser("show", help="1 件の詳細と履歴")
    sp.add_argument("task")
    sp.set_defaults(func=cmd_show)

    sp = sub.add_parser("render", help="Markdown を書き出す")
    sp.add_argument("--out", default=None)
    sp.set_defaults(func=cmd_render)

    args = p.parse_args(argv)
    root = find_root()
    try:
        return args.func(args, root)
    except OSError as e:
        if e.errno == errno.EACCES:
            print("error: .ai/ へ書き込めません", file=sys.stderr)
            return 2
        raise


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""
scripts/update_docs_index.py

docs/plans/README.md と docs/proposals/README.md のインデックス表を自動更新する。

動作:
- ディレクトリを走査して実在するファイルのみ一覧表示する。
- 既存エントリの Notes は保持する（手動補足を上書きしない）。
- 新規ファイルはファイル内容から説明を自動抽出する。
- 削除済みファイルのエントリは除去する。

使い方:
  python scripts/update_docs_index.py
"""

import re
import sys
from datetime import date
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
PLANS_DIR = REPO_ROOT / "docs" / "plans"
PROPOSALS_DIR = REPO_ROOT / "docs" / "proposals"

SKIP_FILES = {"README.md", "plan-template.md", "proposal-template.md"}


# ---------------------------------------------------------------------------
# ファイル内容からの説明抽出
# ---------------------------------------------------------------------------

def extract_first_heading(path: Path) -> str:
    """ファイルの最初の # 見出しテキストを返す。"""
    try:
        for line in path.read_text(encoding="utf-8").splitlines():
            if line.startswith("# "):
                return line[2:].strip()
    except Exception:
        pass
    return path.stem


def extract_description(path: Path) -> str:
    """計画書・提案書から短い説明文を抽出する。"""
    try:
        text = path.read_text(encoding="utf-8")

        # 計画書: "解決したい課題" フィールド
        m = re.search(r"解決したい課題[：:]\s*[`「『]*([^\n`「」『』]+)[`」』]*", text)
        if m:
            desc = m.group(1).strip().rstrip("。").rstrip(".")
            return desc + "の計画。"

        # 提案書: "## 背景" セクション直後の最初の非空行
        m = re.search(
            r"^##\s*(?:\d+\.\s*)?背景[^\n]*\n(?:[^\n]*\n)*?[-\s]*([^\n#]{10,})",
            text,
            re.MULTILINE,
        )
        if m:
            desc = m.group(1).strip("- 　").rstrip("。").strip()
            if desc:
                return desc + "の提案。"

    except Exception:
        pass

    return extract_first_heading(path)


# ---------------------------------------------------------------------------
# ファイル内容からのステータス抽出（plans 用）
# ---------------------------------------------------------------------------

def extract_plan_status(path: Path) -> str | None:
    """計画書のメタ情報テーブルから `ステータス` を抽出する。"""
    try:
        text = path.read_text(encoding="utf-8")
        m = re.search(r"^\|\s*ステータス\s*\|\s*`?([^`|]+)`?\s*\|", text, re.MULTILINE)
        if m:
            return m.group(1).strip()
    except Exception:
        pass
    return None


# ---------------------------------------------------------------------------
# 既存テーブルの Notes 解析
# ---------------------------------------------------------------------------

def parse_existing_notes(content: str, section_prefix: str) -> dict[str, str]:
    """README 内の指定セクションから {ファイル名: Notes} を返す。"""
    result: dict[str, str] = {}

    sec_m = re.search(
        r"^" + re.escape(section_prefix) + r"[^\n]*$", content, re.MULTILINE
    )
    if not sec_m:
        return result

    after_header = content[sec_m.end():]
    next_m = re.search(r"^## ", after_header, re.MULTILINE)
    block = after_header[: next_m.start()] if next_m else after_header

    for line in block.splitlines():
        line = line.strip()
        if not line.startswith("|"):
            continue
        cells = [c.strip() for c in line.split("|")]
        cells = [c for c in cells if c]
        # ヘッダ行・区切り行をスキップ
        if not cells or cells[0].lower() == "document" or re.fullmatch(r"[-\s]+", cells[0]):
            continue

        doc_cell = cells[0]
        notes = cells[-1] if len(cells) >= 3 else (cells[1] if len(cells) >= 2 else "")

        # リンク [text](path) またはバッククォート `filename` からファイル名を取得
        lm = re.search(r"\[([^\]]+)\]\(([^)]+)\)", doc_cell)
        if lm:
            fname = Path(lm.group(2)).name
        else:
            bm = re.search(r"`([^`]+)`", doc_cell)
            fname = bm.group(1) if bm else doc_cell.strip()

        if fname and fname != "Document":
            result[fname] = notes

    return result


def parse_existing_plan_meta(
    content: str, section_prefix: str
) -> dict[str, tuple[str, str]]:
    """README 内の plans テーブルから {ファイル名: (status, notes)} を返す。"""
    result: dict[str, tuple[str, str]] = {}

    sec_m = re.search(
        r"^" + re.escape(section_prefix) + r"[^\n]*$", content, re.MULTILINE
    )
    if not sec_m:
        return result

    after_header = content[sec_m.end():]
    next_m = re.search(r"^## ", after_header, re.MULTILINE)
    block = after_header[: next_m.start()] if next_m else after_header

    for line in block.splitlines():
        line = line.strip()
        if not line.startswith("|"):
            continue
        cells = [c.strip() for c in line.split("|")]
        cells = [c for c in cells if c]
        if not cells or cells[0].lower() == "document" or re.fullmatch(r"[-\s]+", cells[0]):
            continue
        if len(cells) < 3:
            continue

        doc_cell, status, notes = cells[0], cells[1], cells[2]
        lm = re.search(r"\[([^\]]+)\]\(([^)]+)\)", doc_cell)
        if lm:
            fname = Path(lm.group(2)).name
        else:
            bm = re.search(r"`([^`]+)`", doc_cell)
            fname = bm.group(1) if bm else doc_cell.strip()

        if fname and fname != "Document":
            result[fname] = (status, notes)

    return result


# ---------------------------------------------------------------------------
# テーブル行の生成
# ---------------------------------------------------------------------------

def build_rows_3col(
    files: list[Path], rel_prefix: str, status: str, existing: dict[str, str]
) -> list[str]:
    """3列テーブル（Document | Status | Notes）の行リストを生成する。"""
    rows = []
    for f in sorted(files, key=lambda x: x.name.lower()):
        rel = f"{rel_prefix}/{f.name}" if rel_prefix else f.name
        notes = existing.get(f.name) or extract_description(f)
        rows.append(f"| [{rel}]({rel}) | {status} | {notes} |")
    return rows


def build_rows_2col(
    files: list[Path], rel_prefix: str, existing: dict[str, str]
) -> list[str]:
    """2列テーブル（Document | Notes）の行リストを生成する。"""
    rows = []
    for f in sorted(files, key=lambda x: x.name.lower()):
        rel = f"{rel_prefix}/{f.name}" if rel_prefix else f.name
        notes = existing.get(f.name) or extract_description(f)
        rows.append(f"| [{rel}]({rel}) | {notes} |")
    return rows


def build_plan_rows(
    files: list[Path], rel_prefix: str, existing: dict[str, tuple[str, str]]
) -> list[str]:
    """plans 用3列テーブル（Document | Status | Notes）の行リストを生成する。"""
    rows = []
    for f in sorted(files, key=lambda x: x.name.lower()):
        rel = f"{rel_prefix}/{f.name}" if rel_prefix else f.name
        existing_status, existing_notes = existing.get(f.name, ("", ""))
        status = extract_plan_status(f) or existing_status or "Draft"
        notes = existing_notes or extract_description(f)
        rows.append(f"| [{rel}]({rel}) | {status} | {notes} |")
    return rows


# ---------------------------------------------------------------------------
# セクション置換
# ---------------------------------------------------------------------------

def replace_section(
    content: str,
    section_prefix: str,
    header_row: str,
    sep_row: str,
    data_rows: list[str],
) -> str:
    """README 内の指定セクションのテーブルを data_rows で置き換える。"""
    sec_m = re.search(
        r"^" + re.escape(section_prefix) + r"[^\n]*$", content, re.MULTILINE
    )
    if not sec_m:
        return content

    full_header = sec_m.group(0)  # 完全なヘッダ行（バッククォート込み）を保持
    sec_start = sec_m.start()
    after_header = content[sec_m.end():]
    next_m = re.search(r"^## ", after_header, re.MULTILINE)
    remainder = after_header[next_m.start():] if next_m else ""

    rows_str = "\n".join(data_rows)
    new_section = f"{full_header}\n\n{header_row}\n{sep_row}\n{rows_str}\n\n"
    return content[:sec_start] + new_section + remainder


# ---------------------------------------------------------------------------
# docs/plans/README.md の更新
# ---------------------------------------------------------------------------

def update_plans_readme() -> None:
    readme = PLANS_DIR / "README.md"
    content = readme.read_text(encoding="utf-8")

    # 現行計画書（docs/plans/*.md、テンプレート・README を除く）
    current_files = [
        f for f in PLANS_DIR.glob("*.md") if f.name not in SKIP_FILES
    ]
    existing_current = parse_existing_plan_meta(content, "## 現行計画書")
    current_rows = build_plan_rows(current_files, "", existing_current)

    # アーカイブ計画書（docs/plans/archive/*.md）
    archive_dir = PLANS_DIR / "archive"
    archive_files = list(archive_dir.glob("*.md")) if archive_dir.exists() else []
    existing_archive = parse_existing_notes(content, "## アーカイブ計画書一覧")
    archive_rows = build_rows_3col(archive_files, "archive", "アーカイブ", existing_archive)

    # テーブル置換
    content = replace_section(
        content,
        "## 現行計画書",
        "| Document | Status | Notes |",
        "|---|---|---|",
        current_rows,
    )
    content = replace_section(
        content,
        "## アーカイブ計画書一覧",
        "| Document | Status | Notes |",
        "|---|---|---|",
        archive_rows,
    )

    # 更新日を今日の日付に更新
    today = date.today().strftime("%Y-%m-%d")
    content = re.sub(r"（更新日: \d{4}-\d{2}-\d{2}）", f"（更新日: {today}）", content)

    readme.write_text(content, encoding="utf-8")
    print(f"Updated {readme.relative_to(REPO_ROOT)}")
    print(f"  現行計画書: {len(current_files)} 件")
    print(f"  アーカイブ: {len(archive_files)} 件")


# ---------------------------------------------------------------------------
# docs/proposals/README.md の更新
# ---------------------------------------------------------------------------

def update_proposals_readme() -> None:
    readme = PROPOSALS_DIR / "README.md"
    content = readme.read_text(encoding="utf-8")

    # 現在の提案書（docs/proposals/*.md、テンプレート・README を除く）
    current_files = [
        f for f in PROPOSALS_DIR.glob("*.md") if f.name not in SKIP_FILES
    ]
    existing_current = parse_existing_notes(content, "## 現在の提案書")
    current_rows = build_rows_2col(current_files, "", existing_current)

    # アーカイブ提案書（docs/proposals/archive/*.md）
    archive_dir = PROPOSALS_DIR / "archive"
    archive_files = list(archive_dir.glob("*.md")) if archive_dir.exists() else []
    existing_archive = parse_existing_notes(content, "## アーカイブ提案書一覧")
    archive_rows = build_rows_2col(archive_files, "archive", existing_archive)

    # 現在の提案書テーブルを置換
    content = replace_section(
        content,
        "## 現在の提案書",
        "| Document | Notes |",
        "| --- | --- |",
        current_rows,
    )

    # アーカイブセクションの処理
    if "## アーカイブ提案書一覧" in content:
        content = replace_section(
            content,
            "## アーカイブ提案書一覧",
            "| Document | Notes |",
            "| --- | --- |",
            archive_rows,
        )
    else:
        # 旧「補足」テキストを削除してアーカイブテーブルセクションを追加
        old_note = "\n補足: 過去提案は `docs/proposals/archive/` を参照。"
        content = content.replace(old_note, "")
        rows_str = "\n".join(archive_rows)
        archive_section = (
            "\n## アーカイブ提案書一覧 (`docs/proposals/archive`)\n\n"
            "| Document | Notes |\n"
            "| --- | --- |\n"
            f"{rows_str}\n"
        )
        content = content.rstrip() + "\n" + archive_section

    readme.write_text(content, encoding="utf-8")
    print(f"Updated {readme.relative_to(REPO_ROOT)}")
    print(f"  現在の提案書: {len(current_files)} 件")
    print(f"  アーカイブ:   {len(archive_files)} 件")


# ---------------------------------------------------------------------------
# エントリポイント
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    update_plans_readme()
    print()
    update_proposals_readme()
    print()
    print("Done. 差分を確認: git diff docs/plans/README.md docs/proposals/README.md")

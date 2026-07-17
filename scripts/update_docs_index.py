#!/usr/bin/env python3
"""
scripts/update_docs_index.py

docs/plans/README.md と docs/proposals/README.md のインデックス表を自動更新する。

動作:
- ディレクトリを走査して実在するファイルのみ一覧表示する。
- 現行文書はサブディレクトリも走査し、`archive/` / `archived/` / `rejected/` を除外する。
- root の gitignored `archive/` は追跡済み文書だけ、nested `archived/` は実在文書を一覧化する。
- 既存エントリの Notes は保持する（手動補足を上書きしない）。
- 新規ファイルはファイル内容から説明を自動抽出する。
- 削除済みファイルのエントリは除去する。

使い方:
  python3 scripts/update_docs_index.py --check
  python3 scripts/update_docs_index.py --write
"""

import argparse
import re
import subprocess
import sys
from datetime import date
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
PLANS_DIR = REPO_ROOT / "docs" / "plans"
PROPOSALS_DIR = REPO_ROOT / "docs" / "proposals"

SKIP_FILES = {"README.md", "plan-template.md", "proposal-template.md"}
ARCHIVE_DIR_NAMES = {"archive", "archived"}


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

def extract_document_status(path: Path) -> str | None:
    """文書のメタ情報テーブルまたは先頭付近のステータス行を抽出する。"""
    try:
        text = path.read_text(encoding="utf-8")
        m = re.search(r"^\|\s*ステータス\s*\|\s*`?([^`|]+)`?\s*\|", text, re.MULTILINE)
        if m:
            return m.group(1).strip()
        m = re.search(r"^(?:ステータス|Status):\s*`?([^`\n]+)`?\s*$", text, re.MULTILINE)
        if m:
            return m.group(1).strip()
    except Exception:
        pass
    return None


# ---------------------------------------------------------------------------
# 既存テーブルの Notes 解析
# ---------------------------------------------------------------------------

def link_key(doc_cell: str) -> str:
    """README の文書セルから、基準ディレクトリ相対のリンク文字列を返す。"""
    lm = re.search(r"\[([^\]]+)\]\(([^)]+)\)", doc_cell)
    if lm:
        return lm.group(2).replace("\\", "/")
    bm = re.search(r"`([^`]+)`", doc_cell)
    return bm.group(1) if bm else doc_cell.strip()

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

        fname = link_key(doc_cell)

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
        fname = link_key(doc_cell)

        if fname and fname != "Document":
            result[fname] = (status, notes)

    return result


# ---------------------------------------------------------------------------
# テーブル行の生成
# ---------------------------------------------------------------------------

def relative_doc_path(path: Path, base_dir: Path) -> str:
    return path.relative_to(base_dir).as_posix()


def existing_value(existing: dict[str, str], rel: str, name: str) -> str:
    """移動前の root 行も維持できるよう相対パス、basename の順に値を探す。"""
    return existing.get(rel) or existing.get(name) or ""


def tracked_files() -> set[Path]:
    """index と HEAD が追跡するファイルを絶対パスで返す。"""
    result = subprocess.run(
        ["git", "ls-files", "--cached"],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return {
        (REPO_ROOT / line).resolve()
        for line in result.stdout.splitlines()
        if line.strip()
    }


def is_archived(path: Path, base_dir: Path) -> bool:
    parts = relative_doc_path(path, base_dir).split("/")[:-1]
    return any(part in ARCHIVE_DIR_NAMES for part in parts)


def collect_current_files(base_dir: Path) -> list[Path]:
    """サブディレクトリを含む現行文書を収集する。"""
    return [
        path
        for path in base_dir.rglob("*.md")
        if path.name not in SKIP_FILES
        and not is_archived(path, base_dir)
        and "rejected" not in relative_doc_path(path, base_dir).split("/")
    ]


def collect_archive_files(base_dir: Path) -> list[Path]:
    """追跡済みroot archiveと、通常追跡できるnested archivedを収集する。"""
    tracked = tracked_files()
    files = []
    for path in base_dir.rglob("*.md"):
        if not is_archived(path, base_dir):
            continue
        rel_parts = relative_doc_path(path, base_dir).split("/")
        is_ignored_root_archive = rel_parts[0] == "archive"
        if is_ignored_root_archive and path.resolve() not in tracked:
            continue
        files.append(path)
    return files

def build_rows_3col(
    files: list[Path], base_dir: Path, status: str, existing: dict[str, str]
) -> list[str]:
    """3列テーブル（Document | Status | Notes）の行リストを生成する。"""
    rows = []
    for f in sorted(files, key=lambda x: relative_doc_path(x, base_dir).lower()):
        rel = relative_doc_path(f, base_dir)
        notes = existing_value(existing, rel, f.name) or extract_description(f)
        rows.append(f"| [{rel}]({rel}) | {status} | {notes} |")
    return rows


def build_plan_rows(
    files: list[Path], base_dir: Path, existing: dict[str, tuple[str, str]]
) -> list[str]:
    """plans 用3列テーブル（Document | Status | Notes）の行リストを生成する。"""
    rows = []
    for f in sorted(files, key=lambda x: relative_doc_path(x, base_dir).lower()):
        rel = relative_doc_path(f, base_dir)
        existing_status, existing_notes = existing.get(
            rel, existing.get(f.name, ("", ""))
        )
        status = extract_document_status(f) or existing_status or "Draft"
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
# docs/plans/README.md の生成
# ---------------------------------------------------------------------------

def render_plans_readme(content: str) -> tuple[str, int, int]:
    """Return the expected plans README and entry counts without writing."""
    # 現行計画書（サブディレクトリを含み、archive/archivedを除く）
    current_files = collect_current_files(PLANS_DIR)
    existing_current = parse_existing_plan_meta(content, "## 現行計画書")
    current_rows = build_plan_rows(current_files, PLANS_DIR, existing_current)

    # アーカイブ計画書（追跡済みroot archive + nested archived）
    archive_files = collect_archive_files(PLANS_DIR)
    existing_archive = parse_existing_notes(content, "## アーカイブ計画書一覧")
    archive_rows = build_rows_3col(archive_files, PLANS_DIR, "Archived", existing_archive)

    # テーブル置換
    rendered = replace_section(
        content,
        "## 現行計画書",
        "| Document | Status | Notes |",
        "|---|---|---|",
        current_rows,
    )
    rendered = replace_section(
        rendered,
        "## アーカイブ計画書一覧",
        "| Document | Status | Notes |",
        "|---|---|---|",
        archive_rows,
    )

    # テーブルが変わった時だけ日付を更新し、日付だけのchurnを防ぐ。
    if rendered != content:
        today = date.today().strftime("%Y-%m-%d")
        rendered = re.sub(
            r"（更新日: \d{4}-\d{2}-\d{2}）",
            f"（更新日: {today}）",
            rendered,
        )

    return rendered, len(current_files), len(archive_files)


# ---------------------------------------------------------------------------
# docs/proposals/README.md の生成
# ---------------------------------------------------------------------------

def render_proposals_readme(content: str) -> tuple[str, int, int]:
    """Return the expected proposals README and entry counts without writing."""
    # 現在の提案書（サブディレクトリを含み、archive/archivedを除く）
    current_files = collect_current_files(PROPOSALS_DIR)
    existing_current = parse_existing_plan_meta(content, "## 現在の提案書")
    current_rows = build_plan_rows(current_files, PROPOSALS_DIR, existing_current)

    # アーカイブ提案書（追跡済みroot archive + nested archived）
    archive_files = collect_archive_files(PROPOSALS_DIR)
    existing_archive = parse_existing_notes(content, "## アーカイブ提案書一覧")
    archive_rows = build_rows_3col(
        archive_files, PROPOSALS_DIR, "Archived", existing_archive
    )

    # 現在の提案書テーブルを置換
    content = replace_section(
        content,
        "## 現在の提案書",
        "| Document | Status | Notes |",
        "| --- | --- | --- |",
        current_rows,
    )

    # アーカイブセクションの処理
    if "## アーカイブ提案書一覧" in content:
        content = replace_section(
            content,
            "## アーカイブ提案書一覧",
            "| Document | Status | Notes |",
            "| --- | --- | --- |",
            archive_rows,
        )
    else:
        # 旧「補足」テキストを削除してアーカイブテーブルセクションを追加
        old_note = "\n補足: 過去提案は `docs/proposals/archive/` を参照。"
        content = content.replace(old_note, "")
        rows_str = "\n".join(archive_rows)
        archive_section = (
            "\n## アーカイブ提案書一覧 (`archive/` / `**/archived/`)\n\n"
            "| Document | Status | Notes |\n"
            "| --- | --- | --- |\n"
            f"{rows_str}\n"
        )
        content = content.rstrip() + "\n" + archive_section

    return content, len(current_files), len(archive_files)


# ---------------------------------------------------------------------------
# check/write 適用とエントリポイント
# ---------------------------------------------------------------------------

def apply_index(
    path: Path,
    rendered: str,
    *,
    write: bool,
    current_count: int,
    archive_count: int,
) -> bool:
    """Apply or check one generated index. Return True when it was stale."""
    current = path.read_text(encoding="utf-8")
    try:
        relative = path.relative_to(REPO_ROOT)
    except ValueError:
        relative = path
    if current == rendered:
        print(f"OK {relative} ({current_count} current, {archive_count} archived)")
        return False
    if write:
        path.write_text(rendered, encoding="utf-8")
        print(f"Updated {relative} ({current_count} current, {archive_count} archived)")
    else:
        print(f"Stale {relative}; run with --write", file=sys.stderr)
    return True


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="validate without writing")
    mode.add_argument("--write", action="store_true", help="update generated indexes")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    plans_path = PLANS_DIR / "README.md"
    proposals_path = PROPOSALS_DIR / "README.md"

    plans = render_plans_readme(plans_path.read_text(encoding="utf-8"))
    proposals = render_proposals_readme(proposals_path.read_text(encoding="utf-8"))

    stale = apply_index(
        plans_path,
        plans[0],
        write=args.write,
        current_count=plans[1],
        archive_count=plans[2],
    )
    stale |= apply_index(
        proposals_path,
        proposals[0],
        write=args.write,
        current_count=proposals[1],
        archive_count=proposals[2],
    )
    return 1 if stale and args.check else 0


if __name__ == "__main__":
    raise SystemExit(main())

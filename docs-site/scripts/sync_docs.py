#!/usr/bin/env python3
"""Copy canonical Nero Markdown docs into Hugo content without duplicating the source.

The files under ../docs remain canonical. This script only creates the generated Hugo
content tree used by the documentation build.
"""
from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT.parent / "docs"
DEST = ROOT / "content" / "docs"

DOCS = [
    ("README.md", "Documentation", ""),
    ("USAGE.md", "Usage", "usage"),
    ("WORKSPACES.md", "Workspaces", "workspaces"),
    ("APP_HOME.md", "Application home", "app-home"),
    ("MARKDOWN.md", "Markdown", "markdown"),
    ("DUE_DATES.md", "Due dates", "due-dates"),
    ("GUI.md", "Desktop GUI", "gui"),
    ("BACKUP.md", "Backups", "backup"),
    ("RECOVERY.md", "Recovery", "recovery"),
    ("SECURITY.md", "Security", "security"),
    ("STORAGE.md", "Remote Storage", "storage"),
    ("GIT.md", "Git versioning", "git"),
    ("DESIGN.md", "Architecture", "architecture"),
    ("DEVELOPMENT.md", "Development", "development"),
    ("CI.md", "CI & Releases", "ci"),
    ("CHANGELOG.md", "Changelog", "changelog"),
    ("ROADMAP.md", "Roadmap", "roadmap"),
]

SLUGS = {filename.lower(): slug for filename, _, slug in DOCS}


def rewrite_links(text: str, source_slug: str) -> str:
    """Rewrite links among canonical Markdown docs without assuming a domain root.

    Relative URLs preserve the `/nero/` project-Pages base path; absolute `/docs/...`
    links would incorrectly jump to the root of `saeidaliei.github.io`.
    """
    def replace(match: re.Match[str]) -> str:
        label, target = match.group(1), match.group(2)
        if target.startswith(("http://", "https://", "mailto:", "#", "/")):
            return match.group(0)
        path, sep, fragment = target.partition("#")
        slug = SLUGS.get(path.lower())
        if slug is None:
            return match.group(0)
        if source_slug:
            href = "../" if not slug else f"../{slug}/"
        else:
            href = "./" if not slug else f"{slug}/"
        if sep:
            href += f"#{fragment}"
        return f"[{label}]({href})"

    return re.sub(r"\[([^\]]+)\]\(([^)]+)\)", replace, text)


def title_from(markdown: str, fallback: str) -> str:
    for line in markdown.splitlines():
        m = re.match(r"^#\s+(.+?)\s*$", line)
        if m:
            return m.group(1)
    return fallback


def strip_first_h1(markdown: str) -> str:
    lines = markdown.splitlines()
    if lines and re.match(r"^#\s+", lines[0]):
        return "\n".join(lines[1:]).lstrip("\n") + "\n"
    return markdown


def main() -> None:
    if not SOURCE.is_dir():
        raise SystemExit(f"missing canonical docs directory: {SOURCE}")
    if DEST.exists():
        for path in sorted(DEST.rglob("*"), reverse=True):
            if path.is_file() or path.is_symlink():
                path.unlink()
            elif path.is_dir():
                path.rmdir()
    DEST.mkdir(parents=True, exist_ok=True)

    for filename, label, slug in DOCS:
        source = SOURCE / filename
        raw = source.read_text(encoding="utf-8")
        title = title_from(raw, label)
        body = rewrite_links(strip_first_h1(raw), slug)
        out_dir = DEST if not slug else DEST / slug
        out_dir.mkdir(parents=True, exist_ok=True)
        out = out_dir / "_index.md" if not slug else out_dir / "index.md"
        frontmatter = f"---\ntitle: {title!r}\n---\n\n"
        out.write_text(frontmatter + body, encoding="utf-8")


if __name__ == "__main__":
    main()

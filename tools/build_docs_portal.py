"""Compile the project's Markdown pages into one offline guide. Requires Pandoc."""
from html import escape
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
PAGES = [
    ("docs/README.md", "overview", "Оглавление"),
    ("docs/ideas/base-assault.md", "base-assault", "Штурм базы"),
    ("TODO.md", "backlog", "Бэклог"),
    ("docs/history.md", "history", "История версий"),
    ("docs/architecture.md", "architecture", "Архитектура"),
    ("docs/decisions/playtest-05.md", "decisions", "Решения плейтеста"),
    ("docs/decisions/graphics-settings.md", "graphics", "Настройки графики"),
    ("docs/main-protection.md", "main-protection", "Защита main"),
    ("AGENTS.md", "agents", "Работа с проектом"),
    ("BALANCE.md", "balance", "Баланс"),
    ("README.md", "play", "Запуск и управление"),
]
STYLE = """
:root{color-scheme:light;--ink:#172b28;--muted:#546760;--bg:#f7f6f0;--line:#ccd4c7;--accent:#256d5b}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--ink);font:16px/1.58 system-ui,sans-serif}
nav{position:fixed;inset:0 auto 0 0;width:224px;background:#152d29;color:#f0eee4;padding:32px 22px;overflow:auto}
nav strong{display:block;font-size:24px;line-height:1.12;letter-spacing:-.04em}nav small{display:block;color:#c4cfbd;margin:7px 0 24px}
nav a{display:block;color:#e9ece4;text-decoration:none;padding:6px 0}nav a:hover{text-decoration:underline;color:white}
main{margin-left:224px;padding:20px clamp(24px,5vw,76px) 64px;max-width:1300px}article{padding:22px 0 40px;border-bottom:1px solid var(--line)}
h1,h2,h3{line-height:1.16;letter-spacing:-.025em}h1{font-size:clamp(30px,4.7vw,53px);margin:14px 0 24px}h2{margin-top:1.6em;font-size:26px}
p,li{max-width:85ch}p{margin:0 0 16px}a{color:var(--accent);text-underline-offset:3px}code{background:#e6eae0;border-radius:3px;padding:.08em .22em}
pre{background:#e6eae0;border-left:3px solid #649380;padding:14px;overflow:auto}pre code{padding:0}
.tbl{overflow-x:auto;margin:12px 0 20px}table{border-collapse:collapse;min-width:100%;font-size:14px}th,td{padding:9px 11px;border:1px solid var(--line);text-align:left;vertical-align:top}
th{background:#e2e9dc;white-space:normal}tbody tr:nth-child(even){background:#f0f2e9}
@media(max-width:760px){nav{position:static;width:100%;padding:18px}nav small{margin:4px 0 8px}nav a{display:inline-block;margin-right:14px;font-size:14px}main{margin:0;padding:10px 16px 30px}}
@media print{nav{display:none}main{margin:0;padding:0}article{page-break-before:always}article:first-child{page-break-before:auto}}
"""
def main():
    anchors = {(ROOT / path).resolve(): slug for path, slug, _ in PAGES}
    navigation = "".join(f'<a href="#{slug}">{escape(label)}</a>' for _, slug, label in PAGES)
    articles = []
    for path, slug, _ in PAGES:
        file = ROOT / path
        html = subprocess.run(["pandoc", "--from=gfm", "--to=html5", "--wrap=none"],
                              input=file.read_text(encoding="utf8"), text=True,
                              stdout=subprocess.PIPE, check=True).stdout
        def rewrite(link):
            target = link.group(1)
            if target.startswith(("https://", "http://", "#")):
                return link.group(0)
            absolute = (file.parent / target.split("#", 1)[0]).resolve()
            if absolute not in anchors:
                raise ValueError(f"Unlisted doc destination in {path}: {target}")
            return f'href="#{anchors[absolute]}"'
        html = re.sub(r'href="([^"]+)"', rewrite, html)
        html = html.replace("<table>", '<div class="tbl"><table>').replace("</table>", "</table></div>")
        articles.append(f'<article id="{slug}">{html}</article>')
    document = ('<!doctype html><html lang="ru"><meta charset="utf-8">'
                '<meta name="viewport" content="width=device-width,initial-scale=1">'
                '<title>Линия фронта · портал разработки 0.5.2</title>'
                f'<style>{STYLE}</style><nav aria-label="Страницы проекта"><strong>Линия фронта</strong>'
                f'<small>Документация · v0.5.2</small>{navigation}</nav><main>{"".join(articles)}</main></html>')
    (ROOT / "docs/index.html").write_text(document, encoding="utf8")

if __name__ == "__main__":
    main()

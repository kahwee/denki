#!/usr/bin/env python3
"""Build static documentation. Run from any directory; output defaults to _site."""
import argparse
from html import escape
from pathlib import Path
import re
import shutil
import markdown

ROOT = Path(__file__).resolve().parent.parent
PAGES = [
    ('getting-started', 'Getting started'), ('energy', 'Energy monitoring'),
    ('commands', 'Command reference'), ('troubleshooting', 'Troubleshooting'),
    ('library', 'Rust library'), ('architecture', 'Architecture'),
]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / '_site')
    out = parser.parse_args().output
    out.mkdir(parents=True, exist_ok=True)
    template = (ROOT / 'docs/site/template.html').read_text()
    for file in ['style.css', 'app.js']:
        shutil.copyfile(ROOT / 'docs/site' / file, out / file)
    for slug, title in [('index', 'Local energy. Clear answers.'), *PAGES]:
        if slug == 'index':
            content = (ROOT / 'docs/site/home.html').read_text()
        else:
            source = (ROOT / f'docs/{slug}.md').read_text()
            # Rustdoc's no_run attribute has no meaning in the web renderer.
            source = source.replace('```rust,no_run', '```rust')
            content = markdown.markdown(source, extensions=['fenced_code', 'tables', 'toc'])
            def rewrite(match):
                href = match.group(1)
                if href.startswith('../'):
                    href = 'https://github.com/kahwee/denki/blob/main/' + href[3:]
                else:
                    href = re.sub(r'\.md(?=#|$)', '.html', href)
                return 'href="' + href + '"'
            content = re.sub(r'href="([^"]+)"', rewrite, content)
            content = '<article class="prose">' + content + '</article>'
        nav = ''.join(f'<a href="{key}.html"' + (' aria-current="page"' if key == slug else '') + f'>{label}</a>' for key, label in PAGES)
        page = template.replace('{{title}}', escape(title)).replace('{{nav}}', nav).replace('{{content}}', content)
        (out / f'{slug}.html').write_text(page)
    (out / '.nojekyll').touch()
    print(f'Built {len(PAGES) + 1} pages in {out}')

if __name__ == '__main__':
    main()

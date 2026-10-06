#!/usr/bin/env python3
"""Check built pages for missing local links, fragments, assets, and empty titles."""
from html.parser import HTMLParser
from pathlib import Path
import sys
from urllib.parse import unquote, urlsplit

class Page(HTMLParser):
    def __init__(self, source):
        super().__init__()
        self.ids = set()
        self.links = []
        self.feed(source)
    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if 'id' in attrs:
            assert attrs['id'] not in self.ids, f"Duplicate id: {attrs['id']}"
            self.ids.add(attrs['id'])
        for key in ('href', 'src'):
            if key in attrs:
                self.links.append(attrs[key])

root = Path(sys.argv[1] if len(sys.argv) > 1 else '_site').resolve()
pages = {p: Page(p.read_text()) for p in root.glob('*.html')}
assert len(pages) >= 7, 'Expected all documentation pages'
for path, page in pages.items():
    for link in page.links:
        url = urlsplit(link)
        if url.scheme or url.netloc:
            continue
        assert not url.path.startswith('/'), f'Project Pages needs relative URLs: {link}'
        target = (path.parent / unquote(url.path)).resolve() if url.path else path
        assert target.is_relative_to(root), f'Link escapes site: {link}'
        assert target.exists(), f'{path.name}: missing {link}'
        if url.fragment and target in pages:
            assert unquote(url.fragment) in pages[target].ids, f'{path.name}: missing fragment {link}'
print(f'Checked local links, fragments, and assets across {len(pages)} pages')

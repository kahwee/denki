#!/usr/bin/env python3
"""Exercise the built documentation in Chromium without contacting devices."""
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import csv
import io
import json
import os
import shlex
import threading
from playwright.sync_api import sync_playwright

class QuietHandler(SimpleHTTPRequestHandler):
    def log_message(self, *args):
        pass

server = ThreadingHTTPServer(('127.0.0.1', 0), partial(QuietHandler, directory=str(Path('_site').resolve())))
threading.Thread(target=server.serve_forever, daemon=True).start()
origin = f'http://127.0.0.1:{server.server_port}'
try:
    with sync_playwright() as p:
        browser = p.chromium.launch()
        context = browser.new_context(permissions=['clipboard-read', 'clipboard-write'], viewport={'width': 1440, 'height': 1100})
        page = context.new_page()
        errors = []
        page.on('pageerror', lambda error: errors.append(str(error)))
        page.goto(origin)
        command = page.locator('#demo-command')
        page.locator('#copy-command').click()
        assert page.evaluate('navigator.clipboard.readText()') == command.inner_text()
        assert json.loads(page.locator('#demo-output').inner_text())['data']['measurement']['power_w'] == 42.6
        # Quotes, shell metacharacters and markup remain literal data.
        alias = "desk's $(touch nope); <b>plug</b>, west"
        page.locator('#device').fill(alias)
        assert shlex.split(command.inner_text())[2] == alias
        assert page.locator('#demo-output b').count() == 0
        page.locator('#format').select_option('csv')
        page.locator('#interval').fill('10')
        page.locator('#count').fill('3')
        assert '--interval 10 --count 3 --format csv > energy.csv' in command.inner_text()
        rows = list(csv.reader(io.StringIO(page.locator('#demo-output').inner_text())))
        assert len(rows[0]) == len(rows[1]) == 12 and rows[1][1] == alias
        page.locator('#format').select_option('jsonl')
        samples = [json.loads(line) for line in page.locator('#demo-output').inner_text().splitlines()]
        assert len(samples) == 2 and samples[1]['timestamp_unix_ms'] - samples[0]['timestamp_unix_ms'] == 10000
        page.locator('#count').fill('1')
        assert len(page.locator('#demo-output').inner_text().splitlines()) == 1
        page.locator('#interval').fill('0')
        assert page.locator('#copy-command').is_disabled()
        page.locator('#interval').fill('5')
        page.locator('#device').fill('   ')
        assert page.locator('#copy-command').is_disabled()
        page.locator('#device').fill('desk plug')
        assert page.locator('#copy-command').is_enabled()
        page.locator('#format').select_option('json')
        if directory := os.environ.get('DOCS_SCREENSHOTS'):
            Path(directory).mkdir(parents=True, exist_ok=True)
            page.screenshot(path=str(Path(directory) / 'desktop.png'), full_page=True)
        page.goto(origin + '/getting-started.html')
        first = page.locator('pre').first
        first.locator('button').click()
        assert page.evaluate('navigator.clipboard.readText()') == first.locator('code').inner_text()
        # A denied clipboard still selects the text for manual copying.
        page.evaluate("Object.defineProperty(navigator, 'clipboard', {value: {writeText: () => Promise.reject(new Error('denied'))}})")
        first.locator('button').click()
        page.wait_for_function("document.querySelector('pre button').textContent === 'Select text'")
        assert page.evaluate('window.getSelection().toString()').rstrip() == first.locator('code').inner_text().rstrip()
        for width in [390, 768, 1440]:
            page.set_viewport_size({'width': width, 'height': 900})
            for name in ['index', 'getting-started', 'energy', 'commands', 'troubleshooting', 'library', 'architecture']:
                page.goto(origin + '/' + name + '.html')
                assert page.locator('h1').count() == 1
                assert page.evaluate('document.documentElement.scrollWidth <= window.innerWidth'), (width, name)
        page.set_viewport_size({'width': 390, 'height': 844})
        page.goto(origin)
        if directory := os.environ.get('DOCS_SCREENSHOTS'):
            page.screenshot(path=str(Path(directory) / 'mobile.png'), full_page=True)
        assert not errors, errors
        browser.close()
        print('Browser checks passed: copy, clipboard fallback, formats, escaping, validation, and all pages at 3 widths')
finally:
    server.shutdown()

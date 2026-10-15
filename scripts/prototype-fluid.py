#!/usr/bin/env python3
"""Throwaway visual study. Run: python3 scripts/prototype-fluid.py"""
import http.server
import os
from pathlib import Path
import subprocess
import time
import urllib.request
from urllib.parse import parse_qs, urlsplit

ROOT = Path(__file__).resolve().parents[1]
PORT = int(os.environ.get('PROTOTYPE_PORT', '4317'))
BACKEND = PORT + 1
os.chdir(ROOT)
subprocess.run(['cargo', 'build', '-p', 'app-server'], check=True)
env = {**os.environ, 'NBA_PORT': str(BACKEND), 'NBA_DATA_MODE': 'real'}
backend = subprocess.Popen([str(ROOT / 'target/debug/app-server')], env=env)
try:
    for attempt in range(100):
        try:
            with urllib.request.urlopen(f'http://127.0.0.1:{BACKEND}/api/network') as response:
                network = response.read()
            break
        except OSError:
            if backend.poll() is not None:
                raise RuntimeError('App server exited')
            time.sleep(.1)
    else:
        raise RuntimeError('App server did not start')

    class Handler(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            parsed = urlsplit(self.path)
            path = parsed.path
            variant = parse_qs(parsed.query).get('variant', ['A'])[0]
            if path == '/api/network':
                body, mime = network, 'application/json'
            elif path in ('/', '/graph'):
                filename = ('destination-compass.html' if variant in ('G', 'H', 'I')
                            else 'orbit-walk.html' if variant in ('D', 'E', 'F') else 'index.html')
                body = (ROOT / 'crates/app-server/src/prototype-fluid' / filename).read_bytes()
                mime = 'text/html; charset=utf-8'
            else:
                self.send_error(404)
                return
            self.send_response(200)
            self.send_header('Content-Type', mime)
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    print(f'Prototype: http://127.0.0.1:{PORT}/graph?variant=A', flush=True)
    http.server.ThreadingHTTPServer(('127.0.0.1', PORT), Handler).serve_forever()
finally:
    backend.terminate()
    backend.wait()

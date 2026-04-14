from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import os

ROOT = Path(__file__).resolve().parents[1]
INDEX = ROOT / 'index.html'

class SPAHandler(SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(ROOT), **kwargs)

    def do_GET(self):
        requested = self.path.split('?', 1)[0].split('#', 1)[0]
        full_path = (ROOT / requested.lstrip('/')).resolve()
        try:
            full_path.relative_to(ROOT)
            exists = full_path.exists()
        except Exception:
            exists = False

        if requested.startswith('/pkg/') or requested.startswith('/.run/') or exists:
            return super().do_GET()

        self.path = '/index.html'
        return super().do_GET()

if __name__ == '__main__':
    host = '0.0.0.0'
    port = 4173
    httpd = ThreadingHTTPServer((host, port), SPAHandler)
    print(f'Serving SPA preview on http://{host}:{port}')
    httpd.serve_forever()

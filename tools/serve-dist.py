#!/usr/bin/env python3
"""Serve webui/dist with SPA history fallback. Usage: python serve-dist.py [port]"""
import sys
import os
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "webui", "dist")
ROOT = os.path.normpath(ROOT)
DATA = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "data"))


class SPA(SimpleHTTPRequestHandler):
    def __init__(self, *a, **kw):
        super().__init__(*a, directory=ROOT, **kw)

    def send_head(self):
        # /data/* is served from web/data (normally the Rust server's job).
        if self.path.startswith("/data/"):
            name = self.path[len("/data/"):]
            if "/" in name or ".." in name:
                self.send_error(404)
                return None
            f = os.path.join(DATA, name)
            if not os.path.isfile(f):
                self.send_error(404)
                return None
            ctype = self.guess_type(f)
            try:
                fh = open(f, "rb")
            except OSError:
                self.send_error(404)
                return None
            fs = os.fstat(fh.fileno())
            self.send_response(200)
            self.send_header("Content-Type", ctype)
            self.send_header("Content-Length", str(fs.st_size))
            self.end_headers()
            return fh
        path = self.translate_path(self.path)
        if not os.path.exists(path) or (os.path.isdir(path) and not os.path.exists(os.path.join(path, "index.html"))):
            self.path = "/index.html"
        return super().send_head()

    def log_message(self, fmt, *args):
        sys.stderr.write("%s - %s\n" % (self.address_string(), fmt % args))


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 18080
    srv = ThreadingHTTPServer(("127.0.0.1", port), SPA)
    print("serving %s on http://127.0.0.1:%d" % (ROOT, port), flush=True)
    srv.serve_forever()


if __name__ == "__main__":
    main()
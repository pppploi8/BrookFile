import threading
import time
import xml.etree.ElementTree as ET
from http.server import HTTPServer, BaseHTTPRequestHandler
from urllib.parse import unquote, urlparse


class MockWebDAVState:
    def __init__(self):
        self.username = 'admin'
        self.password = 'admin123'
        self.auth_fail = False
        self.upload_fail = False
        self.upload_fail_filter = None
        self.upload_delay = 0
        self.download_fail_paths = set()
        self.list_fail = False
        self.files = {}

    def reset(self):
        self.auth_fail = False
        self.upload_fail = False
        self.upload_fail_filter = None
        self.upload_delay = 0
        self.download_fail_paths = set()
        self.list_fail = False
        self.files = {}

    def _check_auth(self, handler):
        if self.auth_fail:
            return False
        import base64
        auth = handler.headers.get('Authorization', '')
        if auth.startswith('Basic '):
            decoded = base64.b64decode(auth[6:]).decode()
            parts = decoded.split(':', 1)
            if len(parts) == 2 and parts[0] == self.username and parts[1] == self.password:
                return True
        return False


class MockWebDAVHandler(BaseHTTPRequestHandler):
    mock_state: MockWebDAVState = None

    def _resolve_path(self):
        path = unquote(urlparse(self.path).path)
        return path.lstrip('/')

    def _send_xml_propfind(self, path):
        state = self.mock_state
        path_stripped = path.rstrip('/')
        if path_stripped == '':
            path_stripped = ''

        content_items = []
        seen_dirs = set()

        for fp, data in state.files.items():
            fp_stripped = fp.lstrip('/')
            if path_stripped and not fp_stripped.startswith(path_stripped + '/'):
                continue
            rel = fp_stripped
            if path_stripped:
                rel = fp_stripped[len(path_stripped):].lstrip('/')

            if '/' in rel:
                dirname = rel.split('/')[0]
                if dirname not in seen_dirs:
                    seen_dirs.add(dirname)
                    dir_href = '/' + (path_stripped + '/' + dirname if path_stripped else dirname)
                    content_items.append((dir_href, True))
            else:
                file_href = '/' + (path_stripped + '/' + rel if path_stripped else rel)
                content_items.append((file_href, False))

        xml_parts = ['<?xml version="1.0" encoding="utf-8"?>']
        xml_parts.append('<d:multistatus xmlns:d="DAV:">')
        for href, is_dir in content_items:
            xml_parts.append('<d:response>')
            xml_parts.append(f'<d:href>{href}</d:href>')
            xml_parts.append('<d:propstat>')
            xml_parts.append('<d:prop>')
            if is_dir:
                xml_parts.append('<d:resourcetype><d:collection/></d:resourcetype>')
            else:
                xml_parts.append('<d:resourcetype/>')
            xml_parts.append('</d:prop>')
            xml_parts.append('<d:status>HTTP/1.1 200 OK</d:status>')
            xml_parts.append('</d:propstat>')
            xml_parts.append('</d:response>')
        xml_parts.append('</d:multistatus>')

        body = '\n'.join(xml_parts).encode()
        self.send_response(207)
        self.send_header('Content-Type', 'application/xml; charset=utf-8')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path == '/ping':
            self.send_response(200)
            self.send_header('Content-Length', '4')
            self.end_headers()
            self.wfile.write(b'pong')
            return

        if not self.mock_state._check_auth(self):
            self.send_response(401)
            self.end_headers()
            return

        path = self._resolve_path()
        if path in self.mock_state.download_fail_paths:
            self.send_response(500)
            self.end_headers()
            return

        if path in self.mock_state.files:
            data = self.mock_state.files[path]
            if isinstance(data, str):
                data = data.encode()
            self.send_response(200)
            self.send_header('Content-Length', str(len(data)))
            self.send_header('Content-Type', 'application/octet-stream')
            self.end_headers()
            self.wfile.write(data)
        else:
            self.send_response(404)
            self.end_headers()

    def do_PUT(self):
        if not self.mock_state._check_auth(self):
            self.send_response(401)
            self.end_headers()
            return

        state = self.mock_state
        if state.upload_delay > 0:
            time.sleep(state.upload_delay)

        path = self._resolve_path()

        if state.upload_fail:
            self._read_body()
            self.send_response(500)
            self.end_headers()
            return

        if state.upload_fail_filter and state.upload_fail_filter(path):
            self._read_body()
            self.send_response(500)
            self.end_headers()
            return

        body = self._read_body()
        state.files[path] = body
        self.send_response(201)
        self.end_headers()

    def do_PROPFIND(self):
        if not self.mock_state._check_auth(self):
            self.send_response(401)
            self.end_headers()
            return

        if self.mock_state.list_fail:
            self.send_response(500)
            self.end_headers()
            return

        path = self._resolve_path()
        self._read_body()
        self._send_xml_propfind(path)

    def do_MKCOL(self):
        if not self.mock_state._check_auth(self):
            self.send_response(401)
            self.end_headers()
            return
        self.send_response(201)
        self.end_headers()

    def do_DELETE(self):
        if not self.mock_state._check_auth(self):
            self.send_response(401)
            self.end_headers()
            return

        path = self._resolve_path()
        deleted = False
        keys_to_delete = []
        for fp in self.mock_state.files:
            if fp == path or fp.startswith(path + '/'):
                keys_to_delete.append(fp)
                deleted = True
        for k in keys_to_delete:
            del self.mock_state.files[k]

        if deleted:
            self.send_response(204)
        else:
            self.send_response(404)
        self.end_headers()

    def do_MOVE(self):
        if not self.mock_state._check_auth(self):
            self.send_response(401)
            self.end_headers()
            return

        src_path = self._resolve_path()
        dest = unquote(self.headers.get('Destination', ''))
        dest_path = dest.lstrip('/')

        if src_path in self.mock_state.files:
            self.mock_state.files[dest_path] = self.mock_state.files.pop(src_path)
            self.send_response(201)
        else:
            keys_to_move = []
            for fp in self.mock_state.files:
                if fp.startswith(src_path + '/'):
                    keys_to_move.append(fp)
            for old_key in keys_to_move:
                new_key = dest_path + old_key[len(src_path):]
                self.mock_state.files[new_key] = self.mock_state.files.pop(old_key)
            if keys_to_move:
                self.send_response(201)
            else:
                self.send_response(404)
        self.end_headers()

    def _read_body(self):
        length = int(self.headers.get('Content-Length', 0))
        return self.rfile.read(length) if length > 0 else b''

    def log_message(self, *args):
        pass


class MockWebDAVServer:
    def __init__(self, port=15244):
        self.port = port
        self.state = MockWebDAVState()
        self._server = None
        self._thread = None

    def start(self):
        self._server = HTTPServer(('127.0.0.1', self.port), MockWebDAVHandler)
        MockWebDAVHandler.mock_state = self.state
        self._server.mock_state = self.state
        self._thread = threading.Thread(target=self._server.serve_forever, daemon=True)
        self._thread.start()

        for _ in range(50):
            try:
                import requests
                r = requests.get(f'http://127.0.0.1:{self.port}/ping', timeout=0.5)
                if r.status_code == 200:
                    return
            except Exception:
                pass
            time.sleep(0.1)

    def stop(self):
        if self._server:
            self._server.shutdown()

    @property
    def url(self):
        return f'http://127.0.0.1:{self.port}'

    def seed_file(self, path, content):
        if isinstance(content, str):
            content = content.encode()
        self.state.files[path.lstrip('/')] = content


if __name__ == '__main__':
    srv = MockWebDAVServer()
    srv.start()
    print(f'Mock WebDAV running on {srv.url}')
    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        srv.stop()

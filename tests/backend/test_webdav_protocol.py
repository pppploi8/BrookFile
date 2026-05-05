import os
import base64
import requests
from requests.auth import HTTPDigestAuth
from test_utils import run_tests, BASE_URL


def basic_auth(username, password):
    token = base64.b64encode(f'{username}:{password}'.encode()).decode()
    return f'Basic {token}'


def test_webdav_protocol(session, root_path):
    test_dir = os.path.join(root_path, 'webdav_test')
    os.makedirs(os.path.join(test_dir, 'subdir'), exist_ok=True)
    with open(os.path.join(test_dir, 'hello.txt'), 'w') as f:
        f.write('hello world')
    with open(os.path.join(test_dir, 'subdir', 'nested.txt'), 'w') as f:
        f.write('nested content')

    resp = session.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': 'test',
        'access_path': 'webdav_test',
        'password': 'davpass123',
        'permission': 'full_control'
    })
    assert resp.json()['success'] is True

    session.post(f'{BASE_URL}/api/auth/logout')

    dav_digest = HTTPDigestAuth('admin', 'davpass123')
    wrong_digest = HTTPDigestAuth('admin', 'wrongpass')
    nonexistent_digest = HTTPDigestAuth('noone', 'davpass123')
    dav_basic = basic_auth('admin', 'davpass123')

    # Digest auth - PROPFIND Depth 1
    resp = requests.request('PROPFIND', f'{BASE_URL}/dav/test/', auth=dav_digest, headers={
        'Depth': '1'
    })
    assert resp.status_code == 207, f'PROPFIND Depth 1 failed: {resp.status_code}'
    body = resp.text
    assert 'hello.txt' in body
    assert 'subdir' in body

    # No auth - should get 401
    resp = requests.request('PROPFIND', f'{BASE_URL}/dav/test/', headers={
        'Depth': '1'
    })
    assert resp.status_code == 401

    # Wrong password digest - should get 401
    resp = requests.request('PROPFIND', f'{BASE_URL}/dav/test/', auth=wrong_digest, headers={
        'Depth': '1'
    })
    assert resp.status_code == 401

    # Nonexistent user digest - should get 401
    resp = requests.request('PROPFIND', f'{BASE_URL}/dav/test/', auth=nonexistent_digest, headers={
        'Depth': '1'
    })
    assert resp.status_code == 401

    # Basic auth over HTTP - should get 401 (HTTP only allows Digest)
    resp = requests.request('PROPFIND', f'{BASE_URL}/dav/test/', headers={
        'Authorization': dav_basic,
        'Depth': '1'
    })
    assert resp.status_code == 401

    # Digest auth - PROPFIND Depth 0
    resp = requests.request('PROPFIND', f'{BASE_URL}/dav/test/', auth=dav_digest, headers={
        'Depth': '0'
    })
    assert resp.status_code == 207
    body = resp.text
    assert '/dav/test/' in body
    assert 'hello.txt' not in body

    # PROPFIND Depth infinity - should get 403
    resp = requests.request('PROPFIND', f'{BASE_URL}/dav/test/', auth=dav_digest, headers={
        'Depth': 'infinity'
    })
    assert resp.status_code == 403

    # Digest auth - PROPFIND subdir
    resp = requests.request('PROPFIND', f'{BASE_URL}/dav/test/subdir/', auth=dav_digest, headers={
        'Depth': '1'
    })
    assert resp.status_code == 207
    assert 'nested.txt' in resp.text

    # GET file
    resp = requests.get(f'{BASE_URL}/dav/test/hello.txt', auth=dav_digest)
    assert resp.status_code == 200
    assert resp.text == 'hello world'

    # GET nonexistent
    resp = requests.get(f'{BASE_URL}/dav/test/noexist.txt', auth=dav_digest)
    assert resp.status_code == 404

    # PUT new file
    resp = requests.put(f'{BASE_URL}/dav/test/uploaded.txt', auth=dav_digest, data='uploaded content')
    assert resp.status_code in (200, 201, 204)
    uploaded_path = os.path.join(test_dir, 'uploaded.txt')
    assert os.path.exists(uploaded_path)
    with open(uploaded_path) as f:
        assert f.read() == 'uploaded content'

    # PUT overwrite
    resp = requests.put(f'{BASE_URL}/dav/test/uploaded.txt', auth=dav_digest, data='new content')
    assert resp.status_code in (200, 204)
    with open(uploaded_path) as f:
        assert f.read() == 'new content'

    # MKCOL
    resp = requests.request('MKCOL', f'{BASE_URL}/dav/test/newfolder/', auth=dav_digest)
    assert resp.status_code == 201
    assert os.path.isdir(os.path.join(test_dir, 'newfolder'))

    # MKCOL duplicate
    resp = requests.request('MKCOL', f'{BASE_URL}/dav/test/newfolder/', auth=dav_digest)
    assert resp.status_code == 405

    # DELETE file
    resp = requests.delete(f'{BASE_URL}/dav/test/uploaded.txt', auth=dav_digest)
    assert resp.status_code == 204
    assert not os.path.exists(uploaded_path)

    # DELETE nonexistent
    resp = requests.delete(f'{BASE_URL}/dav/test/noexist.txt', auth=dav_digest)
    assert resp.status_code == 404

    # DELETE folder
    resp = requests.delete(f'{BASE_URL}/dav/test/newfolder/', auth=dav_digest)
    assert resp.status_code == 204
    assert not os.path.exists(os.path.join(test_dir, 'newfolder'))

    # MOVE
    resp = requests.request('MOVE', f'{BASE_URL}/dav/test/hello.txt', auth=dav_digest, headers={
        'Destination': '/dav/test/hello_renamed.txt',
    })
    assert resp.status_code == 201
    assert not os.path.exists(os.path.join(test_dir, 'hello.txt'))
    assert os.path.exists(os.path.join(test_dir, 'hello_renamed.txt'))
    with open(os.path.join(test_dir, 'hello_renamed.txt')) as f:
        assert f.read() == 'hello world'

    # OPTIONS
    resp = requests.options(f'{BASE_URL}/dav/test/', auth=dav_digest)
    assert resp.status_code == 200
    assert 'PROPFIND' in resp.headers.get('Allow', '')

    # Test global access with different password
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'admin',
        'password': 'password123'
    })
    resp = session.post(f'{BASE_URL}/api/webdav/list')
    test_cfg = next(c for c in resp.json()['configs'] if c['dav_path'] == 'test')
    session.post(f'{BASE_URL}/api/webdav/delete', json={'id': test_cfg['id']})
    resp = session.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': '',
        'access_path': 'webdav_test',
        'password': 'rootpass',
        'permission': 'read_only',
        'global_access': True
    })
    assert resp.json()['success'] is True
    session.post(f'{BASE_URL}/api/auth/logout')

    root_digest = HTTPDigestAuth('admin', 'rootpass')

    # Global PROPFIND
    resp = requests.request('PROPFIND', f'{BASE_URL}/dav/', auth=root_digest, headers={
        'Depth': '1'
    })
    assert resp.status_code == 207
    assert 'hello_renamed.txt' in resp.text

    # Global GET
    resp = requests.get(f'{BASE_URL}/dav/hello_renamed.txt', auth=root_digest)
    assert resp.status_code == 200
    assert resp.text == 'hello world'

    # Read-only: PUT forbidden
    resp = requests.put(f'{BASE_URL}/dav/forbidden.txt', auth=root_digest, data='should fail')
    assert resp.status_code == 403

    # Read-only: DELETE forbidden
    resp = requests.delete(f'{BASE_URL}/dav/hello_renamed.txt', auth=root_digest)
    assert resp.status_code == 403

    # Read-only: MKCOL forbidden
    resp = requests.request('MKCOL', f'{BASE_URL}/dav/newdir/', auth=root_digest)
    assert resp.status_code == 403

    # Test multiple configs
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'admin',
        'password': 'password123'
    })
    resp = session.post(f'{BASE_URL}/api/webdav/list')
    global_cfg = resp.json()['configs'][0]
    session.post(f'{BASE_URL}/api/webdav/delete', json={'id': global_cfg['id']})
    session.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': 'test',
        'access_path': 'webdav_test',
        'password': 'davpass123',
        'permission': 'full_control'
    })
    session.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': 'editonly',
        'access_path': 'webdav_test',
        'password': 'editpass',
        'permission': 'edit'
    })
    session.post(f'{BASE_URL}/api/auth/logout')

    # Nonexistent dav_path
    resp = requests.request('PROPFIND', f'{BASE_URL}/dav/nonexistent/', auth=dav_digest, headers={
        'Depth': '1'
    })
    assert resp.status_code == 404

    # Edit permission
    edit_digest = HTTPDigestAuth('admin', 'editpass')
    resp = requests.get(f'{BASE_URL}/dav/editonly/hello_renamed.txt', auth=edit_digest)
    assert resp.status_code == 200

    resp = requests.put(f'{BASE_URL}/dav/editonly/edit_test.txt', auth=edit_digest, data='edit content')
    assert resp.status_code in (200, 201, 204)

    # Edit: DELETE forbidden
    resp = requests.delete(f'{BASE_URL}/dav/editonly/edit_test.txt', auth=edit_digest)
    assert resp.status_code == 403

    # Path traversal attempts
    resp = requests.get(f'{BASE_URL}/dav/test/%2e%2e/%2e%2e/secret.txt', auth=dav_digest)
    assert resp.status_code == 404

    resp = requests.put(f'{BASE_URL}/dav/test/%2e%2e/%2e%2e/escaped.txt', auth=dav_digest, data='escaped')
    assert resp.status_code in (400, 404)
    assert not os.path.exists(os.path.join(root_path, 'escaped.txt'))

    # Cross-dav-path MOVE forbidden
    resp = requests.request('MOVE', f'{BASE_URL}/dav/editonly/edit_test.txt', auth=edit_digest, headers={
        'Destination': '/dav/test/stolen.txt',
    })
    assert resp.status_code in (400, 403)

    # MOVE within edit-permission dav_path
    resp = requests.put(f'{BASE_URL}/dav/editonly/move_source.txt', auth=edit_digest, data='move me')
    assert resp.status_code in (200, 201, 204)

    resp = requests.request('MOVE', f'{BASE_URL}/dav/editonly/move_source.txt', auth=edit_digest, headers={
        'Destination': '/dav/editonly/move_dest.txt',
    })
    assert resp.status_code in (201, 204), \
        f'MOVE within edit-permission dav_path should succeed, got {resp.status_code}'
    assert not os.path.exists(os.path.join(test_dir, 'move_source.txt'))
    assert os.path.exists(os.path.join(test_dir, 'move_dest.txt'))
    with open(os.path.join(test_dir, 'move_dest.txt')) as f:
        assert f.read() == 'move me'


if __name__ == '__main__':
    run_tests(test_webdav_protocol)

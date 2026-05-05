import os
import base64
import json
import requests
from test_utils import run_tests, BASE_URL


def test_vault_api(session, root_path):
    # --- list: empty ---
    resp = session.post(f'{BASE_URL}/api/vault/list')
    data = resp.json()
    assert data['vaults'] == []

    # --- create: not logged in ---
    session.post(f'{BASE_URL}/api/auth/logout')
    resp = session.post(f'{BASE_URL}/api/vault/create', json={
        'name': 'test', 'path': 'v1', 'filename': 'test.dat',
        'file_data': base64.b64encode(b'data').decode()
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_LOGGED_IN'

    # --- list: not logged in ---
    resp = session.post(f'{BASE_URL}/api/vault/list')
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_LOGGED_IN'

    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'admin', 'password': 'password123'
    })

    # --- create: invalid path ---
    resp = session.post(f'{BASE_URL}/api/vault/create', json={
        'name': 'test',
        'path': '../outside',
        'filename': 'test.dat',
        'file_data': base64.b64encode(b'data').decode()
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'INVALID_FILE_PATH'

    # --- create: invalid filename ---
    resp = session.post(f'{BASE_URL}/api/vault/create', json={
        'name': 'test',
        'path': 'v1',
        'filename': '../hack.dat',
        'file_data': base64.b64encode(b'data').decode()
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'INVALID_FILE_PATH'

    # --- create: invalid base64 ---
    resp = session.post(f'{BASE_URL}/api/vault/create', json={
        'name': 'test',
        'path': 'vaults',
        'filename': 'test.dat',
        'file_data': 'not-valid-base64!!!'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'INVALID_FILE_DATA'

    # --- create: success ---
    vault_content = {"version": 1, "rounds": 100000, "data": "test"}
    file_data = base64.b64encode(json.dumps(vault_content).encode()).decode()
    resp = session.post(f'{BASE_URL}/api/vault/create', json={
        'name': 'MyVault',
        'description': 'test vault',
        'path': 'vaults',
        'filename': 'personal.dat',
        'file_data': file_data
    })
    data = resp.json()
    assert data['success'] is True
    vault_id = data['id']

    assert os.path.isfile(os.path.join(root_path, 'vaults', 'personal.dat'))
    with open(os.path.join(root_path, 'vaults', 'personal.dat'), 'rb') as f:
        stored = json.loads(f.read())
    assert stored['version'] == 1
    assert stored['rounds'] == 100000

    # --- list: has one ---
    resp = session.post(f'{BASE_URL}/api/vault/list')
    data = resp.json()
    assert len(data['vaults']) == 1
    assert data['vaults'][0]['name'] == 'MyVault'
    assert data['vaults'][0]['description'] == 'test vault'
    assert data['vaults'][0]['path'] == 'vaults'
    assert data['vaults'][0]['filename'] == 'personal.dat'

    # --- update: name only ---
    resp = session.post(f'{BASE_URL}/api/vault/update_meta', json={
        'id': vault_id,
        'name': 'MyVault2'
    })
    assert resp.json()['success'] is True
    resp = session.post(f'{BASE_URL}/api/vault/list')
    v = resp.json()['vaults'][0]
    assert v['name'] == 'MyVault2'
    assert v['description'] == 'test vault'

    # --- update: new file_data ---
    new_content = {"version": 1, "rounds": 200000, "data": "updated"}
    new_data = base64.b64encode(json.dumps(new_content).encode()).decode()
    resp = session.post(f'{BASE_URL}/api/vault/update', json={
        'id': vault_id,
        'file_data': new_data
    })
    assert resp.json()['success'] is True
    with open(os.path.join(root_path, 'vaults', 'personal.dat'), 'rb') as f:
        stored = json.loads(f.read())
    assert stored == new_content

    # --- update: not found ---
    resp = session.post(f'{BASE_URL}/api/vault/update_meta', json={
        'id': 'nonexistent',
        'name': 'x'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'VAULT_NOT_FOUND'

    # --- update: not logged in ---
    session.post(f'{BASE_URL}/api/auth/logout')
    resp = session.post(f'{BASE_URL}/api/vault/update_meta', json={
        'id': vault_id, 'name': 'x'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_LOGGED_IN'
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'admin', 'password': 'password123'
    })

    # --- import: file not found ---
    resp = session.post(f'{BASE_URL}/api/vault/import', json={
        'name': 'Imported',
        'file_path': 'nonexist/file.dat'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'FILE_NOT_FOUND'

    # --- import: path traversal ---
    resp = session.post(f'{BASE_URL}/api/vault/import', json={
        'name': 'Traversal',
        'file_path': '../../etc/passwd'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] in ('FILE_NOT_FOUND', 'INVALID_FILE_PATH')

    # --- import: not logged in ---
    session.post(f'{BASE_URL}/api/auth/logout')
    resp = session.post(f'{BASE_URL}/api/vault/import', json={
        'name': 'NoAuth',
        'file_path': 'backup/import.dat'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_LOGGED_IN'
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'admin', 'password': 'password123'
    })

    # --- import: from existing file ---
    os.makedirs(os.path.join(root_path, 'backup'), exist_ok=True)
    with open(os.path.join(root_path, 'backup', 'import.dat'), 'wb') as f:
        f.write(b'imported-data')
    resp = session.post(f'{BASE_URL}/api/vault/import', json={
        'name': 'ImportedVault',
        'description': 'from backup',
        'file_path': 'backup/import.dat'
    })
    data = resp.json()
    assert data['success'] is True
    import_id = data['id']

    # --- import: duplicate ---
    resp = session.post(f'{BASE_URL}/api/vault/import', json={
        'name': 'Another',
        'file_path': 'backup/import.dat'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'VAULT_ALREADY_EXISTS'

    # --- upload_single ---
    resp = session.post(f'{BASE_URL}/api/vault/upload_single', files={
        'path': (None, 'vaults/single.dat'),
        'file': ('file', b'single-upload-content', 'application/octet-stream')
    })
    assert resp.json()['success'] is True
    assert os.path.isfile(os.path.join(root_path, 'vaults', 'single.dat'))
    with open(os.path.join(root_path, 'vaults', 'single.dat'), 'rb') as f:
        assert f.read() == b'single-upload-content'

    # --- upload_single: overwrite ---
    resp = session.post(f'{BASE_URL}/api/vault/upload_single', files={
        'path': (None, 'vaults/single.dat'),
        'file': ('file', b'overwritten', 'application/octet-stream')
    })
    assert resp.json()['success'] is True
    with open(os.path.join(root_path, 'vaults', 'single.dat'), 'rb') as f:
        assert f.read() == b'overwritten'

    # --- delete imported vault: file still exists ---
    resp = session.post(f'{BASE_URL}/api/vault/delete', json={'id': import_id})
    assert resp.json()['success'] is True
    assert os.path.exists(os.path.join(root_path, 'backup', 'import.dat'))

    # --- delete: not found ---
    resp = session.post(f'{BASE_URL}/api/vault/delete', json={'id': 'nonexistent'})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'VAULT_NOT_FOUND'

    # --- delete: not logged in ---
    session.post(f'{BASE_URL}/api/auth/logout')
    resp = session.post(f'{BASE_URL}/api/vault/delete', json={'id': vault_id})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_LOGGED_IN'
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'admin', 'password': 'password123'
    })

    # --- delete created vault: file still exists ---
    resp = session.post(f'{BASE_URL}/api/vault/delete', json={'id': vault_id})
    assert resp.json()['success'] is True
    assert os.path.exists(os.path.join(root_path, 'vaults', 'personal.dat'))

    # --- list: empty after all deletions ---
    resp = session.post(f'{BASE_URL}/api/vault/list')
    data = resp.json()
    assert data['vaults'] == []

    # --- delete user cleans up vaults ---
    workdir = os.path.dirname(root_path)
    vault_user_root = os.path.join(workdir, 'vault_user_data')
    os.makedirs(vault_user_root)
    resp = session.post(f'{BASE_URL}/api/user/create', json={
        'username': 'vault_user',
        'password': 'test12345',
        'root_path': vault_user_root
    })
    assert resp.json()['success'] is True

    session.post(f'{BASE_URL}/api/auth/logout')
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'vault_user', 'password': 'test12345'
    })
    resp = session.post(f'{BASE_URL}/api/vault/create', json={
        'name': 'UserVault',
        'path': 'user_vaults',
        'filename': 'user.dat',
        'file_data': base64.b64encode(b'user-vault').decode()
    })
    assert resp.json()['success'] is True

    session.post(f'{BASE_URL}/api/auth/logout')
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'admin', 'password': 'password123'
    })
    resp = session.post(f'{BASE_URL}/api/user/list')
    vu = next(u for u in resp.json() if u['username'] == 'vault_user')
    session.post(f'{BASE_URL}/api/user/delete', json={'id': vu['id']})

    resp = session.post(f'{BASE_URL}/api/user/create', json={
        'username': 'vault_verify',
        'password': 'test12345'
    })
    session.post(f'{BASE_URL}/api/auth/logout')
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'vault_verify', 'password': 'test12345'
    })
    resp = session.post(f'{BASE_URL}/api/vault/list')
    assert resp.json()['vaults'] == []


if __name__ == '__main__':
    run_tests(test_vault_api)

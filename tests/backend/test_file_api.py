import os
import time
import requests
from test_utils import run_tests, BASE_URL


def test_file_api(session, root_path):
    # --- browse: not logged in ---
    anon = requests.Session()
    resp = anon.post(f'{BASE_URL}/api/file/browse', json={})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_LOGGED_IN'

    # --- download: not logged in ---
    resp = anon.post(f'{BASE_URL}/api/file/download', json={'path': 'test.txt'})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_LOGGED_IN'

    # --- upload_start: not logged in ---
    resp = anon.post(f'{BASE_URL}/api/file/upload_start', json={'files': ['test.txt']})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_LOGGED_IN'

    # --- move: not logged in ---
    resp = anon.post(f'{BASE_URL}/api/file/move', json={'files': ['test'], 'target_path': ''})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_LOGGED_IN'

    # --- batch_delete: not logged in ---
    resp = anon.post(f'{BASE_URL}/api/file/batch_delete', json={'files': ['test']})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_LOGGED_IN'

    # --- browse: empty root ---
    resp = session.post(f'{BASE_URL}/api/file/browse', json={})
    data = resp.json()
    assert 'files' in data
    assert len(data['files']) == 0

    # --- create folder ---
    resp = session.post(f'{BASE_URL}/api/file/create_folder', json={'name': 'docs'})
    assert resp.json()['success']
    assert os.path.isdir(os.path.join(root_path, 'docs'))

    # --- create folder: duplicate ---
    resp = session.post(f'{BASE_URL}/api/file/create_folder', json={'name': 'docs'})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'FOLDER_ALREADY_EXISTS'

    # --- create folder: invalid name ---
    resp = session.post(f'{BASE_URL}/api/file/create_folder', json={'name': 'a/b'})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'INVALID_FOLDER_NAME'

    # --- create sub folder ---
    resp = session.post(f'{BASE_URL}/api/file/create_folder', json={
        'parent_path': 'docs', 'name': 'sub'
    })
    assert resp.json()['success']

    # --- create folder: nonexistent parent ---
    resp = session.post(f'{BASE_URL}/api/file/create_folder', json={
        'parent_path': 'nonexistent', 'name': 'folder'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PATH_NOT_FOUND'

    # --- browse: root has docs ---
    resp = session.post(f'{BASE_URL}/api/file/browse', json={})
    data = resp.json()
    names = [f['name'] for f in data['files']]
    assert 'docs' in names

    # --- browse: docs has sub ---
    resp = session.post(f'{BASE_URL}/api/file/browse', json={'path': 'docs'})
    data = resp.json()
    names = [f['name'] for f in data['files']]
    assert 'sub' in names

    # --- browse: not found ---
    resp = session.post(f'{BASE_URL}/api/file/browse', json={'path': 'nope'})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PATH_NOT_FOUND'

    # --- browse: path traversal ---
    resp = session.post(f'{BASE_URL}/api/file/browse', json={'path': '../..'})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'INVALID_FILE_PATH'

    # --- upload: start + chunk + complete ---
    resp = session.post(f'{BASE_URL}/api/file/upload_start', json={
        'files': ['hello.txt']
    })
    data = resp.json()
    assert data['success']
    upload_id = data['uploads'][0]['id']

    content = b'Hello, BrookFile!'
    for i in range(0, len(content), 5):
        chunk = content[i:i + 5]
        resp = session.post(f'{BASE_URL}/api/file/upload_chunk', files={
            'upload_id': (None, upload_id),
            'offset': (None, str(i)),
            'chunk': ('chunk', chunk, 'application/octet-stream')
        })
        assert resp.json()['success']

    resp = session.post(f'{BASE_URL}/api/file/upload_complete', json={
        'upload_id': upload_id
    })
    assert resp.json()['success']
    with open(os.path.join(root_path, 'hello.txt'), 'rb') as f:
        assert f.read() == content

    # --- upload: already exists ---
    resp = session.post(f'{BASE_URL}/api/file/upload_start', json={
        'files': ['hello.txt']
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'FILES_ALREADY_EXIST'
    assert 'hello.txt' in data['existing_files']

    # --- upload: batch start ---
    resp = session.post(f'{BASE_URL}/api/file/upload_start', json={
        'files': ['batch1.txt', 'batch2.txt']
    })
    data = resp.json()
    assert data['success'] is True
    assert len(data['uploads']) == 2
    for uid in [u['id'] for u in data['uploads']]:
        session.post(f'{BASE_URL}/api/file/upload_chunk', files={
            'upload_id': (None, uid),
            'offset': (None, '0'),
            'chunk': ('chunk', b'batch', 'application/octet-stream')
        })
        session.post(f'{BASE_URL}/api/file/upload_complete', json={'upload_id': uid})

    # --- upload: cancel ---
    resp = session.post(f'{BASE_URL}/api/file/upload_start', json={
        'files': ['cancel.txt']
    })
    data = resp.json()
    assert data['success']
    cancel_id = data['uploads'][0]['id']

    resp = session.post(f'{BASE_URL}/api/file/upload_cancel', json={
        'upload_id': cancel_id
    })
    assert resp.json()['success']

    resp = session.post(f'{BASE_URL}/api/file/upload_complete', json={
        'upload_id': cancel_id
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'UPLOAD_NOT_FOUND'

    # --- upload: nonexistent upload_id ---
    resp = session.post(f'{BASE_URL}/api/file/upload_complete', json={
        'upload_id': 'nonexistent-uuid'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'UPLOAD_NOT_FOUND'

    # --- upload: path traversal ---
    resp = session.post(f'{BASE_URL}/api/file/upload_start', json={
        'files': ['../outside.txt']
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'INVALID_FILE_PATH'

    # --- download ---
    resp = session.post(f'{BASE_URL}/api/file/download', json={'path': 'hello.txt'})
    assert resp.status_code == 200
    assert resp.content == content

    # --- download: path traversal ---
    resp = session.post(f'{BASE_URL}/api/file/download', json={'path': '../../Windows/system.ini'})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'INVALID_FILE_PATH'

    # --- download: not found ---
    resp = session.post(f'{BASE_URL}/api/file/download', json={'path': 'nope.txt'})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PATH_NOT_FOUND'

    # --- download: folder ---
    resp = session.post(f'{BASE_URL}/api/file/download', json={'path': 'docs'})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_A_FILE'

    # --- special filename with .. ---
    with open(os.path.join(root_path, '..abc..def.txt'), 'w') as f:
        f.write('Special filename test')
    resp = session.post(f'{BASE_URL}/api/file/browse', json={})
    names = [f['name'] for f in resp.json()['files']]
    assert '..abc..def.txt' in names
    resp = session.post(f'{BASE_URL}/api/file/download', json={'path': '..abc..def.txt'})
    assert resp.status_code == 200
    assert resp.content == b'Special filename test'

    # --- upload to sub dir (auto create parent) ---
    resp = session.post(f'{BASE_URL}/api/file/upload_start', json={
        'files': ['docs/a/b.txt']
    })
    data = resp.json()
    assert data['success']
    sub_id = data['uploads'][0]['id']

    resp = session.post(f'{BASE_URL}/api/file/upload_chunk', files={
        'upload_id': (None, sub_id),
        'offset': (None, '0'),
        'chunk': ('chunk', b'sub', 'application/octet-stream')
    })
    assert resp.json()['success']

    resp = session.post(f'{BASE_URL}/api/file/upload_complete', json={
        'upload_id': sub_id
    })
    assert resp.json()['success']
    assert os.path.isfile(os.path.join(root_path, 'docs', 'a', 'b.txt'))

    # --- move ---
    resp = session.post(f'{BASE_URL}/api/file/move', json={
        'files': ['hello.txt'],
        'target_path': 'docs'
    })
    assert resp.json()['success']
    assert not os.path.exists(os.path.join(root_path, 'hello.txt'))
    assert os.path.isfile(os.path.join(root_path, 'docs', 'hello.txt'))

    # --- move: conflict (create same-name file in target) ---
    resp = session.post(f'{BASE_URL}/api/file/upload_start', json={
        'files': ['hello2.txt']
    })
    up2 = resp.json()['uploads'][0]['id']
    resp = session.post(f'{BASE_URL}/api/file/upload_chunk', files={
        'upload_id': (None, up2),
        'offset': (None, '0'),
        'chunk': ('chunk', b'x', 'application/octet-stream')
    })
    resp = session.post(f'{BASE_URL}/api/file/upload_complete', json={'upload_id': up2})

    with open(os.path.join(root_path, 'docs', 'hello2.txt'), 'w') as f:
        f.write('conflict')

    resp = session.post(f'{BASE_URL}/api/file/move', json={
        'files': ['hello2.txt'],
        'target_path': 'docs'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'FILES_ALREADY_EXIST'
    assert 'hello2.txt' in data.get('conflict_files', [])

    # --- move: empty files list ---
    resp = session.post(f'{BASE_URL}/api/file/move', json={
        'files': [],
        'target_path': 'docs'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NO_FILES_SPECIFIED'

    # --- move: nonexistent current_path ---
    resp = session.post(f'{BASE_URL}/api/file/move', json={
        'files': ['test.txt'],
        'current_path': 'nonexistent',
        'target_path': 'docs'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PATH_NOT_FOUND'

    # --- move: nonexistent target_path ---
    resp = session.post(f'{BASE_URL}/api/file/move', json={
        'files': ['test.txt'],
        'target_path': 'nonexistent'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'TARGET_PATH_NOT_FOUND'

    # --- move: using current_path to move files from subfolder ---
    with open(os.path.join(root_path, 'batch1.txt'), 'w') as f:
        f.write('move me')

    resp = session.post(f'{BASE_URL}/api/file/move', json={
        'files': ['batch1.txt'],
        'current_path': '',
        'target_path': 'docs'
    })
    assert resp.json()['success']
    assert not os.path.exists(os.path.join(root_path, 'batch1.txt'))
    assert os.path.isfile(os.path.join(root_path, 'docs', 'batch1.txt'))

    # --- move folder ---
    os.makedirs(os.path.join(root_path, 'move_target'))
    resp = session.post(f'{BASE_URL}/api/file/move', json={
        'files': ['a'],
        'current_path': 'docs',
        'target_path': 'move_target'
    })
    assert resp.json()['success']
    assert os.path.isdir(os.path.join(root_path, 'move_target', 'a'))
    assert os.path.isfile(os.path.join(root_path, 'move_target', 'a', 'b.txt'))

    # --- delete ---
    resp = session.post(f'{BASE_URL}/api/file/delete', json={'path': 'hello2.txt'})
    assert resp.json()['success']
    assert not os.path.exists(os.path.join(root_path, 'hello2.txt'))

    # --- delete: already gone ---
    resp = session.post(f'{BASE_URL}/api/file/delete', json={'path': 'hello2.txt'})
    assert resp.json()['success']

    # --- delete folder ---
    resp = session.post(f'{BASE_URL}/api/file/delete', json={'path': 'docs'})
    assert resp.json()['success']
    assert not os.path.exists(os.path.join(root_path, 'docs'))

    # --- batch delete ---
    with open(os.path.join(root_path, 'b1.txt'), 'w') as f:
        f.write('1')
    with open(os.path.join(root_path, 'b2.txt'), 'w') as f:
        f.write('2')

    resp = session.post(f'{BASE_URL}/api/file/batch_delete', json={
        'files': ['b1.txt', 'b2.txt']
    })
    assert resp.json()['success']
    assert not os.path.exists(os.path.join(root_path, 'b1.txt'))
    assert not os.path.exists(os.path.join(root_path, 'b2.txt'))

    # --- batch delete: empty files list ---
    resp = session.post(f'{BASE_URL}/api/file/batch_delete', json={'files': []})
    assert resp.json()['success']

    # --- batch delete: nonexistent files ---
    resp = session.post(f'{BASE_URL}/api/file/batch_delete', json={
        'files': ['nonexistent1.txt', 'nonexistent2.txt']
    })
    assert resp.json()['success']

    # --- batch delete: using current_path ---
    os.makedirs(os.path.join(root_path, 'target'))
    with open(os.path.join(root_path, 'target', 'td1.txt'), 'w') as f:
        f.write('t')
    with open(os.path.join(root_path, 'target', 'td2.txt'), 'w') as f:
        f.write('t')

    resp = session.post(f'{BASE_URL}/api/file/batch_delete', json={
        'files': ['td1.txt', 'td2.txt'],
        'current_path': 'target'
    })
    assert resp.json()['success']
    assert not os.path.exists(os.path.join(root_path, 'target', 'td1.txt'))
    assert not os.path.exists(os.path.join(root_path, 'target', 'td2.txt'))

    # --- batch delete folder ---
    os.makedirs(os.path.join(root_path, 'folder_to_del'))
    with open(os.path.join(root_path, 'folder_to_del', 'f.txt'), 'w') as f:
        f.write('f')
    resp = session.post(f'{BASE_URL}/api/file/batch_delete', json={
        'files': ['folder_to_del']
    })
    assert resp.json()['success']
    assert not os.path.exists(os.path.join(root_path, 'folder_to_del'))

    # --- rename: not logged in ---
    resp = anon.post(f'{BASE_URL}/api/file/rename', json={'path': 'test.txt', 'new_name': 'renamed.txt'})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_LOGGED_IN'

    # --- rename file ---
    with open(os.path.join(root_path, 'rename_me.txt'), 'w') as f:
        f.write('rename test')
    resp = session.post(f'{BASE_URL}/api/file/rename', json={
        'path': 'rename_me.txt', 'new_name': 'renamed.txt'
    })
    assert resp.json()['success']
    assert not os.path.exists(os.path.join(root_path, 'rename_me.txt'))
    assert os.path.isfile(os.path.join(root_path, 'renamed.txt'))

    # --- rename folder ---
    os.makedirs(os.path.join(root_path, 'rename_folder'))
    resp = session.post(f'{BASE_URL}/api/file/rename', json={
        'path': 'rename_folder', 'new_name': 'folder_renamed'
    })
    assert resp.json()['success']
    assert not os.path.exists(os.path.join(root_path, 'rename_folder'))
    assert os.path.isdir(os.path.join(root_path, 'folder_renamed'))

    # --- rename: target already exists ---
    with open(os.path.join(root_path, 'conflict_a.txt'), 'w') as f:
        f.write('a')
    with open(os.path.join(root_path, 'conflict_b.txt'), 'w') as f:
        f.write('b')
    resp = session.post(f'{BASE_URL}/api/file/rename', json={
        'path': 'conflict_a.txt', 'new_name': 'conflict_b.txt'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'TARGET_ALREADY_EXISTS'

    # --- rename: path not found ---
    resp = session.post(f'{BASE_URL}/api/file/rename', json={
        'path': 'nonexistent.txt', 'new_name': 'nope.txt'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PATH_NOT_FOUND'

    # --- rename: invalid new name ---
    resp = session.post(f'{BASE_URL}/api/file/rename', json={
        'path': 'renamed.txt', 'new_name': 'a/b'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'INVALID_FILE_NAME'

    # --- rename: invalid path ---
    resp = session.post(f'{BASE_URL}/api/file/rename', json={
        'path': '../etc/passwd', 'new_name': 'hacked'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'INVALID_FILE_PATH'

    # --- rename: empty path ---
    resp = session.post(f'{BASE_URL}/api/file/rename', json={
        'path': '', 'new_name': 'nope'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'INVALID_FILE_PATH'

    # --- rename in subfolder ---
    os.makedirs(os.path.join(root_path, 'sub_rename'))
    with open(os.path.join(root_path, 'sub_rename', 'inner.txt'), 'w') as f:
        f.write('inner')
    resp = session.post(f'{BASE_URL}/api/file/rename', json={
        'path': 'sub_rename/inner.txt', 'new_name': 'inner_renamed.txt'
    })
    assert resp.json()['success']
    assert os.path.isfile(os.path.join(root_path, 'sub_rename', 'inner_renamed.txt'))


    # --- H2: upload_chunk should reject requests from other users ---
    workdir = os.path.dirname(root_path)
    h2_user_root = os.path.join(workdir, 'h2_other_data')
    h2_user_recycle = os.path.join(workdir, 'h2_other_recycle')
    os.makedirs(h2_user_root, exist_ok=True)
    os.makedirs(h2_user_recycle, exist_ok=True)

    resp = session.post(f'{BASE_URL}/api/user/create', json={
        'username': 'h2_other', 'password': 'pass12345',
        'root_path': h2_user_root, 'recycle_bin_path': h2_user_recycle, 'is_admin': False,
    })
    assert resp.json()['success']

    other_session = requests.Session()
    resp = other_session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'h2_other', 'password': 'pass12345'
    })
    assert resp.json()['success']

    resp = session.post(f'{BASE_URL}/api/file/upload_start', json={'files': ['h2_test.txt']})
    data = resp.json()
    assert data['success']
    upload_id = data['uploads'][0]['id']

    resp = other_session.post(f'{BASE_URL}/api/file/upload_chunk', files={
        'upload_id': (None, upload_id),
        'offset': (None, '0'),
        'chunk': ('chunk', b'evil data from other user', 'application/octet-stream'),
    })
    data = resp.json()
    assert data['success'] is False, \
        'H2 BUG: Other user can upload chunks to current user\'s upload session!'

    session.post(f'{BASE_URL}/api/file/upload_cancel', json={'upload_id': upload_id})

    # --- H3: upload_complete should reject requests from other users ---
    resp = session.post(f'{BASE_URL}/api/file/upload_start', json={'files': ['h3_test.txt']})
    data = resp.json()
    assert data['success']
    upload_id2 = data['uploads'][0]['id']

    resp = session.post(f'{BASE_URL}/api/file/upload_chunk', files={
        'upload_id': (None, upload_id2),
        'offset': (None, '0'),
        'chunk': ('chunk', b'legit data', 'application/octet-stream'),
    })
    assert resp.json()['success']

    resp = other_session.post(f'{BASE_URL}/api/file/upload_complete', json={'upload_id': upload_id2})
    data = resp.json()
    assert data['success'] is False, \
        'H3 BUG: Other user can complete current user\'s upload session!'

    if data['success'] is False:
        session.post(f'{BASE_URL}/api/file/upload_cancel', json={'upload_id': upload_id2})

    # --- delete: path traversal → INVALID_FILE_PATH ---
    resp = session.post(f'{BASE_URL}/api/file/delete', json={'path': '../outside.txt'})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'INVALID_FILE_PATH'

    # --- batch_delete: path traversal in filename → INVALID_FILE_PATH ---
    resp = session.post(f'{BASE_URL}/api/file/batch_delete', json={
        'files': ['../outside.txt']
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'INVALID_FILE_PATH'


if __name__ == '__main__':
    run_tests(test_file_api)

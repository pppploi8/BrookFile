import os
import io
import requests
from datetime import datetime, timedelta, timezone
from test_utils import run_tests, BASE_URL


def test_user_api(session, root_path):
    # --- list users ---
    resp = session.post(f'{BASE_URL}/api/user/list')
    users = resp.json()
    assert isinstance(users, list)
    assert len(users) == 1
    assert users[0]['username'] == 'admin'
    assert users[0]['is_admin'] is True
    admin_id = users[0]['id']

    # --- get user ---
    resp = session.post(f'{BASE_URL}/api/user/get', json={'id': admin_id})
    user = resp.json()
    assert user['username'] == 'admin'
    assert user['is_admin'] is True
    assert user['id'] == admin_id

    # --- get user: not found ---
    resp = session.post(f'{BASE_URL}/api/user/get', json={'id': 'nonexistent'})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'USER_NOT_FOUND'

    # --- not logged in ---
    anon = requests.Session()
    resp = anon.post(f'{BASE_URL}/api/user/list')
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_LOGGED_IN'
    resp = anon.post(f'{BASE_URL}/api/user/get', json={'id': admin_id})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_LOGGED_IN'

    # --- create user: empty username ---
    resp = session.post(f'{BASE_URL}/api/user/create', json={
        'username': '',
        'password': 'pass12345'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'USERNAME_EMPTY'

    # --- create user: empty password ---
    resp = session.post(f'{BASE_URL}/api/user/create', json={
        'username': 'user1',
        'password': ''
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PASSWORD_EMPTY'

    # --- create user: success ---
    workdir = os.path.dirname(root_path)
    user_root = os.path.join(workdir, 'user1_data')
    user_recycle = os.path.join(workdir, 'user1_recycle')
    os.makedirs(user_root)
    os.makedirs(user_recycle)
    resp = session.post(f'{BASE_URL}/api/user/create', json={
        'username': 'user1',
        'password': 'pass12345',
        'root_path': user_root,
        'recycle_bin_path': user_recycle,
        'is_admin': False,
        'remark': 'test user'
    })
    assert resp.json()['success'] is True

    # --- create user: duplicate ---
    resp = session.post(f'{BASE_URL}/api/user/create', json={
        'username': 'user1',
        'password': 'pass45678'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'USERNAME_ALREADY_EXISTS'

    # --- create user: recycle_bin_path invalid ---
    user2_root = os.path.join(workdir, 'user2_data')
    os.makedirs(user2_root)
    resp = session.post(f'{BASE_URL}/api/user/create', json={
        'username': 'user2',
        'password': 'pass12345',
        'root_path': user2_root,
        'recycle_bin_path': os.path.join(user2_root, 'sub')
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'RECYCLE_BIN_PATH_INVALID'

    # --- list users: now 2 ---
    resp = session.post(f'{BASE_URL}/api/user/list')
    users = resp.json()
    assert len(users) == 2
    user1 = next(u for u in users if u['username'] == 'user1')
    user1_id = user1['id']
    assert user1['is_admin'] is False
    assert user1['remark'] == 'test user'

    # --- update user password via user/update ---
    resp = session.post(f'{BASE_URL}/api/user/update', json={
        'id': user1_id,
        'password': 'newpassword123'
    })
    assert resp.json()['success'] is True

    # verify old password fails
    resp = session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'user1',
        'password': 'pass12345'
    })
    assert resp.json()['success'] is False

    # verify new password works
    resp = session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'user1',
        'password': 'newpassword123'
    })
    assert resp.json()['success'] is True

    # re-login as admin
    session.post(f'{BASE_URL}/api/auth/logout')
    resp = session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'admin',
        'password': 'password123'
    })
    assert resp.json()['success'] is True

    # --- update self admin: CANNOT_MODIFY_SELF_ADMIN ---
    resp = session.post(f'{BASE_URL}/api/user/update', json={
        'id': admin_id,
        'is_admin': False
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'CANNOT_MODIFY_SELF_ADMIN'

    # --- update self expire: CANNOT_MODIFY_SELF_EXPIRE ---
    resp = session.post(f'{BASE_URL}/api/user/update', json={
        'id': admin_id,
        'expire_at': '2030-01-01T00:00:00Z'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'CANNOT_MODIFY_SELF_EXPIRE'

    # --- update self: save without changing expire_at ---
    resp = session.post(f'{BASE_URL}/api/user/get', json={'id': admin_id})
    admin_info = resp.json()
    current_expire_at = admin_info.get('expire_at')
    resp = session.post(f'{BASE_URL}/api/user/update', json={
        'id': admin_id,
        'remark': 'test remark',
        'expire_at': current_expire_at
    })
    assert resp.json()['success'] is True

    # re-login after session may have changed
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'admin',
        'password': 'password123'
    })

    # --- update remark ---
    resp = session.post(f'{BASE_URL}/api/user/update', json={
        'id': user1_id,
        'remark': 'updated remark'
    })
    assert resp.json()['success'] is True
    resp = session.post(f'{BASE_URL}/api/user/get', json={'id': user1_id})
    assert resp.json()['remark'] == 'updated remark'

    # --- clear remark to empty string ---
    resp = session.post(f'{BASE_URL}/api/user/update', json={
        'id': user1_id,
        'remark': ''
    })
    assert resp.json()['success'] is True
    resp = session.post(f'{BASE_URL}/api/user/get', json={'id': user1_id})
    assert resp.json()['remark'] is None

    # --- update: not found ---
    resp = session.post(f'{BASE_URL}/api/user/update', json={
        'id': 'nonexistent',
        'remark': 'x'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'USER_NOT_FOUND'

    # --- delete self: CANNOT_DELETE_SELF ---
    resp = session.post(f'{BASE_URL}/api/user/delete', json={'id': admin_id})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'CANNOT_DELETE_SELF'

    # --- delete: not found ---
    resp = session.post(f'{BASE_URL}/api/user/delete', json={'id': 'nonexistent'})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'USER_NOT_FOUND'

    # --- create user with expire_at (past) ---
    expired_time = (datetime.now(timezone.utc) - timedelta(minutes=1)).strftime('%Y-%m-%dT%H:%M:%SZ')
    resp = session.post(f'{BASE_URL}/api/user/create', json={
        'username': 'expiring_user',
        'password': 'test12345',
        'expire_at': expired_time
    })
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/user/list')
    expiring = next(u for u in resp.json() if u['username'] == 'expiring_user')
    assert expiring['expire_at'] == expired_time
    expiring_id = expiring['id']

    # --- expired user cannot login ---
    resp = session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'expiring_user',
        'password': 'test12345'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'ACCOUNT_EXPIRED'

    # --- update expire_at to future ---
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'admin',
        'password': 'password123'
    })
    resp = session.post(f'{BASE_URL}/api/user/update', json={
        'id': expiring_id,
        'expire_at': '2099-12-31T23:59:59Z'
    })
    assert resp.json()['success'] is True

    session.post(f'{BASE_URL}/api/auth/logout')
    resp = session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'expiring_user',
        'password': 'test12345'
    })
    assert resp.json()['success'] is True

    # --- clear expire_at ---
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'admin',
        'password': 'password123'
    })
    resp = session.post(f'{BASE_URL}/api/user/update', json={
        'id': expiring_id,
        'expire_at': ''
    })
    assert resp.json()['success'] is True
    resp = session.post(f'{BASE_URL}/api/user/get', json={'id': expiring_id})
    assert resp.json()['expire_at'] is None

    # --- delete expiring user ---
    resp = session.post(f'{BASE_URL}/api/user/delete', json={'id': expiring_id})
    assert resp.json()['success'] is True
    resp = session.post(f'{BASE_URL}/api/user/get', json={'id': expiring_id})
    assert resp.json()['success'] is False
    assert resp.json()['fail_code'] == 'USER_NOT_FOUND'

    # --- delete user1 ---
    resp = session.post(f'{BASE_URL}/api/user/delete', json={'id': user1_id})
    assert resp.json()['success'] is True

    # --- delete again: not found ---
    resp = session.post(f'{BASE_URL}/api/user/delete', json={'id': user1_id})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'USER_NOT_FOUND'

    # --- update feature order: invalid ---
    resp = session.post(f'{BASE_URL}/api/user/update_feature_order', json={
        'feature_order': 'file,photo'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'INVALID_FEATURE_ORDER'

    # --- update feature order: success ---
    resp = session.post(f'{BASE_URL}/api/user/update_feature_order', json={
        'feature_order': 'file,note,password'
    })
    assert resp.json()['success'] is True

    # --- upload avatar ---
    png = b'\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde\x00\x00\x00\x0cIDATx\x9cc\xf8\x0f\x00\x00\x01\x01\x00\x05\x18\xd8N\x00\x00\x00\x00IEND\xaeB`\x82'
    resp = session.post(f'{BASE_URL}/api/user/upload_avatar', files={
        'avatar': ('avatar.png', io.BytesIO(png), 'image/png')
    })
    assert resp.json()['success'] is True

    # --- get avatar: verify content and content-type ---
    resp = session.post(f'{BASE_URL}/api/user/get_avatar', json={'id': admin_id})
    assert resp.status_code == 200
    assert resp.headers.get('Content-Type') == 'image/png'
    assert resp.content == png

    # --- get avatar: nonexistent user ---
    resp = session.post(f'{BASE_URL}/api/user/get_avatar', json={'id': 'nonexistent'})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'AVATAR_NOT_FOUND'

    # --- upload avatar: invalid file type ---
    resp = session.post(f'{BASE_URL}/api/user/upload_avatar', files={
        'avatar': ('test.txt', io.BytesIO(b'not an image'), 'text/plain')
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'INVALID_FILE_TYPE'

    # --- delete avatar ---
    resp = session.post(f'{BASE_URL}/api/user/delete_avatar')
    assert resp.json()['success'] is True

    # --- get avatar after delete ---
    resp = session.post(f'{BASE_URL}/api/user/get_avatar', json={'id': admin_id})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'AVATAR_NOT_FOUND'

    # --- delete avatar again ---
    resp = session.post(f'{BASE_URL}/api/user/delete_avatar')
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'AVATAR_NOT_FOUND'

    # --- upload avatar without login ---
    session.post(f'{BASE_URL}/api/auth/logout')
    no_auth = requests.Session()
    resp = no_auth.post(f'{BASE_URL}/api/user/upload_avatar', files={
        'avatar': ('test.png', io.BytesIO(png), 'image/png')
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_LOGGED_IN'

    # --- delete avatar without login ---
    resp = no_auth.post(f'{BASE_URL}/api/user/delete_avatar')
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_LOGGED_IN'

    # --- change password ---
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'admin',
        'password': 'password123'
    })

    # change password: wrong old
    resp = session.post(f'{BASE_URL}/api/user/change_password', json={
        'old_password': 'wrong',
        'new_password': 'newpass123'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'OLD_PASSWORD_INCORRECT'

    # change password: empty old
    resp = session.post(f'{BASE_URL}/api/user/change_password', json={
        'old_password': '',
        'new_password': 'newpass123'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PASSWORD_EMPTY'

    # change password: empty new
    resp = session.post(f'{BASE_URL}/api/user/change_password', json={
        'old_password': 'password123',
        'new_password': ''
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PASSWORD_EMPTY'

    # change password: success
    resp = session.post(f'{BASE_URL}/api/user/change_password', json={
        'old_password': 'password123',
        'new_password': 'newpass123'
    })
    assert resp.json()['success'] is True

    # login with new password
    resp = session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'admin',
        'password': 'newpass123'
    })
    assert resp.json()['success'] is True

    # --- change password without login ---
    session.post(f'{BASE_URL}/api/auth/logout')
    resp = session.post(f'{BASE_URL}/api/user/change_password', json={
        'old_password': 'newpass123',
        'new_password': 'another'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_LOGGED_IN'

    # --- H1: upload_avatar should not delete CWD files when headicons dir is unreadable ---
    import shutil
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'admin',
        'password': 'newpass123'
    })
    workdir = os.path.dirname(root_path)
    sentinel_path = os.path.join(workdir, f'{admin_id}.txt')
    with open(sentinel_path, 'w') as f:
        f.write('this file should not be deleted')

    headicons_path = os.path.join(workdir, 'headicons')
    if os.path.isdir(headicons_path):
        shutil.rmtree(headicons_path)
    elif os.path.exists(headicons_path):
        os.remove(headicons_path)
    with open(headicons_path, 'w') as f:
        f.write('not a directory')

    resp = session.post(f'{BASE_URL}/api/user/upload_avatar', files={
        'avatar': ('avatar.png', io.BytesIO(png), 'image/png')
    })

    assert os.path.exists(sentinel_path), \
        'H1 BUG: upload_avatar fallback deleted a CWD file matching user_id stem!'

    os.remove(headicons_path)
    if os.path.exists(sentinel_path):
        os.remove(sentinel_path)

    # --- delete user should also delete backup rules ---
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'admin',
        'password': 'newpass123'
    })
    resp = session.post(f'{BASE_URL}/api/user/create', json={
        'username': 'backup_test_user',
        'password': 'test12345'
    })
    assert resp.json()['success'] is True
    resp = session.post(f'{BASE_URL}/api/user/list')
    bt_user = next(u for u in resp.json() if u['username'] == 'backup_test_user')
    bt_user_id = bt_user['id']

    session.post(f'{BASE_URL}/api/auth/logout')
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'backup_test_user',
        'password': 'test12345'
    })
    resp = session.post(f'{BASE_URL}/api/backup/create', json={
        'name': 'Test Backup Rule',
        'storage_type': 'webdav',
        'storage_config': {
            'address': 'http://127.0.0.1:15244',
            'username': 'testuser',
            'password': 'testpass123',
            'path': '/backup/test'
        },
        'local_path': '/data/backup',
        'encrypted': False,
        'cycle': 'daily',
        'backup_time': {'time': '08:00'}
    })
    assert resp.json()['success'] is True
    resp = session.post(f'{BASE_URL}/api/backup/list')
    assert len(resp.json()) == 1

    session.post(f'{BASE_URL}/api/auth/logout')
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'admin',
        'password': 'newpass123'
    })
    resp = session.post(f'{BASE_URL}/api/user/delete', json={'id': bt_user_id})
    assert resp.json()['success'] is True

    # create new user and verify backup rules are gone
    resp = session.post(f'{BASE_URL}/api/user/create', json={
        'username': 'backup_verify_user',
        'password': 'test12345'
    })
    session.post(f'{BASE_URL}/api/auth/logout')
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'backup_verify_user',
        'password': 'test12345'
    })
    resp = session.post(f'{BASE_URL}/api/backup/list')
    assert len(resp.json()) == 0


    # --- BUG: update_user recycle_bin_path validated against old root_path ---
    # When admin updates both root_path and recycle_bin_path in one request,
    # the recycle_bin_path should be validated against the NEW root_path,
    # but the backend validates against the OLD root_path from the database.
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'admin',
        'password': 'newpass123'
    })
    workdir = os.path.dirname(root_path)
    bug_root = os.path.join(workdir, 'bug_user_root')
    bug_recycle = os.path.join(workdir, 'bug_user_recycle')
    os.makedirs(bug_root, exist_ok=True)
    os.makedirs(bug_recycle, exist_ok=True)
    resp = session.post(f'{BASE_URL}/api/user/create', json={
        'username': 'bug_user',
        'password': 'pass12345',
        'root_path': bug_root,
        'recycle_bin_path': bug_recycle,
        'is_admin': False,
    })
    assert resp.json()['success'] is True
    bug_user = next(u for u in session.post(f'{BASE_URL}/api/user/list').json() if u['username'] == 'bug_user')
    bug_user_id = bug_user['id']

    new_root = os.path.join(workdir, 'bug_user_new_root')
    new_recycle_under_new = os.path.join(new_root, 'recycle')
    os.makedirs(new_root, exist_ok=True)

    # recycle_bin_path is under the NEW root_path → should be REJECTED as RECYCLE_BIN_PATH_INVALID
    # BUG: backend validates against OLD root_path (bug_root), so new_root/recycle is NOT under
    # bug_root, and the update incorrectly succeeds.
    resp = session.post(f'{BASE_URL}/api/user/update', json={
        'id': bug_user_id,
        'root_path': new_root,
        'recycle_bin_path': new_recycle_under_new,
    })
    assert resp.json()['success'] is False, \
        'BUG: update_user should reject recycle_bin_path under the NEW root_path, ' \
        'but it validates against the OLD root_path and incorrectly allows it.'
    assert resp.json()['fail_code'] == 'RECYCLE_BIN_PATH_INVALID'

    # cleanup
    session.post(f'{BASE_URL}/api/user/delete', json={'id': bug_user_id})


if __name__ == '__main__':
    run_tests(test_user_api)

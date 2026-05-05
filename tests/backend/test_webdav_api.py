import os
import requests
from test_utils import run_tests, BASE_URL


def test_webdav_api(session, root_path):
    # --- list: empty ---
    resp = session.post(f'{BASE_URL}/api/webdav/list')
    data = resp.json()
    assert data['success'] is True
    assert data['configs'] == []

    # --- BUG6: create with empty dav_path (non-global) should be rejected ---
    resp = session.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': '',
        'access_path': 'photos',
        'password': 'pass123',
        'permission': 'full_control'
    })
    data = resp.json()
    assert data['success'] is False, \
        f'BUG6: Empty dav_path for non-global should be rejected, got success={data["success"]}'
    assert data['fail_code'] == 'DAV_PATH_INVALID', \
        f'BUG6: Expected DAV_PATH_INVALID, got {data.get("fail_code")}'

    # --- create: invalid dav_path (slash) ---
    resp = session.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': 'bad/path',
        'access_path': 'photos',
        'password': 'pass123',
        'permission': 'full_control'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'DAV_PATH_INVALID'

    # --- create: invalid dav_path (space) ---
    resp = session.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': 'has space',
        'access_path': 'photos',
        'password': 'pass123',
        'permission': 'full_control'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'DAV_PATH_INVALID'

    # --- create: empty password ---
    resp = session.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': 'photos',
        'access_path': 'photos',
        'password': '',
        'permission': 'full_control'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PARAM_INVALID'

    # --- create: invalid permission ---
    resp = session.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': 'photos',
        'access_path': 'photos',
        'password': 'pass123',
        'permission': 'invalid'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PARAM_INVALID'

    # --- BUG55/57: dav_path format validation should work with global_access=true ---
    resp = session.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': '!!!invalid',
        'access_path': '',
        'password': 'pass123',
        'permission': 'full_control',
        'global_access': True
    })
    data = resp.json()
    assert data['success'] is False, \
        'BUG55: dav_path with invalid chars should be rejected even with global_access=true'
    assert data['fail_code'] == 'DAV_PATH_INVALID', \
        f'BUG55: Expected DAV_PATH_INVALID, got {data.get("fail_code")}'

    # --- create: success ---
    resp = session.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': 'photos',
        'access_path': 'photos',
        'password': 'pass123',
        'permission': 'full_control'
    })
    assert resp.json()['success'] is True

    # --- BUG6: update with empty dav_path (non-global) should be rejected ---
    resp_list = session.post(f'{BASE_URL}/api/webdav/list')
    photos_cfg = next(c for c in resp_list.json()['configs'] if c['dav_path'] == 'photos')
    resp = session.post(f'{BASE_URL}/api/webdav/update', json={
        'id': photos_cfg['id'],
        'dav_path': '',
        'access_path': 'photos',
        'password': 'pass123',
        'permission': 'full_control'
    })
    data = resp.json()
    assert data['success'] is False, \
        f'BUG6: Update with empty dav_path for non-global should be rejected, got success={data["success"]}'
    assert data['fail_code'] == 'DAV_PATH_INVALID', \
        f'BUG6: Expected DAV_PATH_INVALID on update, got {data.get("fail_code")}'

    # --- create: second config ---
    resp = session.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': 'docs',
        'access_path': 'docs',
        'password': 'docsecret',
        'permission': 'read_only'
    })
    assert resp.json()['success'] is True

    # --- list: 2 items, verify fields ---
    resp = session.post(f'{BASE_URL}/api/webdav/list')
    data = resp.json()
    assert data['success'] is True
    assert len(data['configs']) == 2
    for c in data['configs']:
        assert 'id' in c
        assert 'dav_path' in c
        assert 'access_path' in c
        assert 'permission' in c
        assert 'url' in c
        assert 'created_at' in c
        assert 'updated_at' in c
        if c['dav_path'] == 'photos':
            assert c['url'] == '/dav/photos/'
            assert c['permission'] == 'full_control'
        elif c['dav_path'] == 'docs':
            assert c['url'] == '/dav/docs/'
            assert c['permission'] == 'read_only'

    photos_config = next(c for c in data['configs'] if c['dav_path'] == 'photos')
    docs_config = next(c for c in data['configs'] if c['dav_path'] == 'docs')

    # --- create: duplicate dav_path ---
    resp = session.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': 'photos',
        'access_path': 'other',
        'password': 'pass',
        'permission': 'edit'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'DAV_PATH_DUPLICATE'

    # --- create: dash and underscore ---
    resp = session.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': 'my-files_v2',
        'access_path': 'files_v2',
        'password': 'pass',
        'permission': 'full_control'
    })
    assert resp.json()['success'] is True
    resp = session.post(f'{BASE_URL}/api/webdav/list')
    cfg = next(c for c in resp.json()['configs'] if c['dav_path'] == 'my-files_v2')
    assert cfg['url'] == '/dav/my-files_v2/'

    # cleanup extra
    session.post(f'{BASE_URL}/api/webdav/delete', json={'id': cfg['id']})

    # --- update ---
    resp = session.post(f'{BASE_URL}/api/webdav/update', json={
        'id': photos_config['id'],
        'dav_path': 'pictures',
        'access_path': 'pictures',
        'password': 'newsecret',
        'permission': 'edit'
    })
    assert resp.json()['success'] is True

    # --- verify update: url changed ---
    resp = session.post(f'{BASE_URL}/api/webdav/list')
    updated = next(c for c in resp.json()['configs'] if c['id'] == photos_config['id'])
    assert updated['dav_path'] == 'pictures'
    assert updated['access_path'] == 'pictures'
    assert updated['permission'] == 'edit'
    assert updated['url'] == '/dav/pictures/'

    # --- update without password field ---
    resp = session.post(f'{BASE_URL}/api/webdav/update', json={
        'id': updated['id'],
        'dav_path': 'pictures',
        'access_path': 'pictures_v2',
        'permission': 'read_only'
    })
    assert resp.json()['success'] is True

    # --- update: not found ---
    resp = session.post(f'{BASE_URL}/api/webdav/update', json={
        'id': 'nonexistent',
        'dav_path': 'test',
        'access_path': 'test',
        'password': '',
        'permission': 'read_only'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'DAV_CONFIG_NOT_FOUND'

    # --- update: duplicate dav_path ---
    resp = session.post(f'{BASE_URL}/api/webdav/update', json={
        'id': updated['id'],
        'dav_path': 'docs',
        'access_path': 'docs',
        'permission': 'full_control'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'DAV_PATH_DUPLICATE'

    # --- try global_access with existing configs ---
    resp = session.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': '',
        'access_path': '',
        'password': 'global',
        'permission': 'full_control',
        'global_access': True
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'DAV_CONFIGS_CONFLICT'

    # --- not logged in ---
    session.post(f'{BASE_URL}/api/auth/logout')
    resp = session.post(f'{BASE_URL}/api/webdav/list')
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_LOGGED_IN'

    # --- different users can have same dav_path ---
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'admin',
        'password': 'password123'
    })
    workdir = os.path.dirname(root_path)
    user2_root = os.path.join(workdir, 'webdav_user2')
    os.makedirs(user2_root)
    resp = session.post(f'{BASE_URL}/api/user/create', json={
        'username': 'webdav_user2',
        'password': 'pass12345',
        'root_path': user2_root
    })
    assert resp.json()['success'] is True

    session.post(f'{BASE_URL}/api/auth/logout')
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'webdav_user2',
        'password': 'pass12345'
    })
    resp = session.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': 'pictures',
        'access_path': 'user2_pics',
        'password': 'user2pass',
        'permission': 'edit'
    })
    assert resp.json()['success'] is True
    resp = session.post(f'{BASE_URL}/api/webdav/list')
    data = resp.json()
    assert len(data['configs']) == 1
    assert data['configs'][0]['dav_path'] == 'pictures'

    # back to admin
    session.post(f'{BASE_URL}/api/auth/logout')
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'admin',
        'password': 'password123'
    })

    # --- delete ---
    resp = session.post(f'{BASE_URL}/api/webdav/list')
    all_ids = [c['id'] for c in resp.json()['configs']]
    for cid in all_ids:
        resp = session.post(f'{BASE_URL}/api/webdav/delete', json={'id': cid})
        assert resp.json()['success'] is True

    # --- list: 0 after delete ---
    resp = session.post(f'{BASE_URL}/api/webdav/list')
    assert len(resp.json()['configs']) == 0

    # --- delete again: not found ---
    resp = session.post(f'{BASE_URL}/api/webdav/delete', json={'id': all_ids[0]})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'DAV_CONFIG_NOT_FOUND'

    # --- BUG4: global_access=true with non-empty dav_path should be rejected (create) ---
    resp = session.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': 'should-be-empty',
        'access_path': '',
        'password': 'pass123',
        'permission': 'full_control',
        'global_access': True
    })
    data = resp.json()
    assert data['success'] is False, \
        f'BUG4: global_access=true with non-empty dav_path should be rejected (create), got success={data["success"]}'
    assert data['fail_code'] == 'DAV_PATH_INVALID', \
        f'BUG4: Expected DAV_PATH_INVALID, got {data.get("fail_code")}'

    # --- BUG4: global_access=true with non-empty dav_path should be rejected (update) ---
    resp = session.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': 'bug4test',
        'access_path': 'bug4test',
        'password': 'pass123',
        'permission': 'full_control'
    })
    assert resp.json()['success'] is True
    resp_list = session.post(f'{BASE_URL}/api/webdav/list')
    bug4_cfg = next(c for c in resp_list.json()['configs'] if c['dav_path'] == 'bug4test')
    resp = session.post(f'{BASE_URL}/api/webdav/update', json={
        'id': bug4_cfg['id'],
        'dav_path': 'not-empty',
        'access_path': 'bug4test',
        'password': 'pass123',
        'permission': 'full_control',
        'global_access': True
    })
    data = resp.json()
    assert data['success'] is False, \
        f'BUG4: global_access=true with non-empty dav_path should be rejected (update), got success={data["success"]}'
    assert data['fail_code'] == 'DAV_PATH_INVALID', \
        f'BUG4: Expected DAV_PATH_INVALID on update, got {data.get("fail_code")}'

    session.post(f'{BASE_URL}/api/webdav/delete', json={'id': bug4_cfg['id']})


if __name__ == '__main__':
    run_tests(test_webdav_api)

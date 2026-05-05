import os
from test_utils import run_tests, BASE_URL, DEFAULT_USERNAME, DEFAULT_PASSWORD, DEFAULT_SYSTEM_NAME


def test_system_api(session, root_path):
    # --- 1. info: uninitialized ---
    resp = session.post(f'{BASE_URL}/api/system/info')
    data = resp.json()
    assert data['initialized'] is False
    assert data['logged_in'] is False
    assert data['system_name'] == 'BrookFile'

    # --- 2. browse: root (uninitialized) ---
    resp = session.post(f'{BASE_URL}/api/system/browse', json={})
    data = resp.json()
    assert 'folders' in data
    assert 'has_parent' in data
    assert data['has_parent'] is False

    # --- 3. browse: specific path ---
    os.makedirs(root_path, exist_ok=True)
    resp = session.post(f'{BASE_URL}/api/system/browse', json={'path': os.path.dirname(root_path)})
    data = resp.json()
    assert 'folders' in data
    assert data['has_parent'] is True
    assert 'parent_path' in data
    folder_names = [f['name'] for f in data['folders']]
    assert os.path.basename(root_path) in folder_names

    # --- 4. init: empty root_path ---
    resp = session.post(f'{BASE_URL}/api/system/init', json={
        'username': DEFAULT_USERNAME,
        'password': DEFAULT_PASSWORD,
        'system_name': DEFAULT_SYSTEM_NAME,
        'root_path': ''
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'ROOT_PATH_EMPTY'

    # --- 5. init: recycle_bin_path inside root_path ---
    os.makedirs(root_path, exist_ok=True)
    resp = session.post(f'{BASE_URL}/api/system/init', json={
        'username': DEFAULT_USERNAME,
        'password': DEFAULT_PASSWORD,
        'system_name': DEFAULT_SYSTEM_NAME,
        'root_path': root_path,
        'recycle_bin_path': os.path.join(root_path, 'sub')
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'RECYCLE_BIN_PATH_INVALID'

    # --- 6. init: recycle_bin_path equals root_path ---
    resp = session.post(f'{BASE_URL}/api/system/init', json={
        'username': DEFAULT_USERNAME,
        'password': DEFAULT_PASSWORD,
        'system_name': DEFAULT_SYSTEM_NAME,
        'root_path': root_path,
        'recycle_bin_path': root_path
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'RECYCLE_BIN_PATH_INVALID'

    # --- 7. init: success without recycle_bin_path ---
    workdir = os.path.dirname(root_path)
    init_root = os.path.join(workdir, 'init_test_data')
    os.makedirs(init_root)
    resp = session.post(f'{BASE_URL}/api/system/init', json={
        'username': DEFAULT_USERNAME,
        'password': DEFAULT_PASSWORD,
        'system_name': DEFAULT_SYSTEM_NAME,
        'root_path': init_root
    })
    data = resp.json()
    assert data['success'] is True

    # --- 8. info: initialized, not logged in ---
    resp = session.post(f'{BASE_URL}/api/system/info')
    data = resp.json()
    assert data['initialized'] is True
    assert data['logged_in'] is False
    assert data['system_name'] == DEFAULT_SYSTEM_NAME

    # --- 9. login after init ---
    resp = session.post(f'{BASE_URL}/api/auth/login', json={
        'username': DEFAULT_USERNAME,
        'password': DEFAULT_PASSWORD
    })
    assert resp.json()['success'] is True

    # --- 10. info: logged in ---
    resp = session.post(f'{BASE_URL}/api/system/info')
    data = resp.json()
    assert data['initialized'] is True
    assert data['logged_in'] is True
    assert data['system_name'] == DEFAULT_SYSTEM_NAME
    assert data['user']['username'] == DEFAULT_USERNAME
    assert data['user']['is_admin'] is True

    # --- 11. browse: after init ---
    resp = session.post(f'{BASE_URL}/api/system/browse', json={})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'SYSTEM_ALREADY_INITIALIZED'

    # --- 12. init: already initialized ---
    resp = session.post(f'{BASE_URL}/api/system/init', json={
        'username': DEFAULT_USERNAME,
        'password': DEFAULT_PASSWORD,
        'system_name': 'AnotherSystem',
        'root_path': root_path
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'SYSTEM_ALREADY_INITIALIZED'


if __name__ == '__main__':
    run_tests(test_system_api, init=False)

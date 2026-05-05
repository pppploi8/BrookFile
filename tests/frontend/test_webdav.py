import time
from test_utils import run_frontend_test, _ref


def _get_comp(agent):
    comps = agent.find_components_by_name('ProfileCenter')
    assert len(comps) > 0, 'ProfileCenter component not found'
    return comps[0]


def _navigate_to_profile(page, agent):
    page.evaluate('() => { const app = document.getElementById("app").__vue_app__; const router = app.config.globalProperties.$router; router.push("/profile"); }')
    time.sleep(2.0)
    agent.wait_ready(timeout=10000)
    comp = _get_comp(agent)
    return comp


def _api_call(page, url, data):
    return page.evaluate("""async ([url, data]) => {
        const r = await fetch(url, {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify(data),
            credentials: 'include'
        });
        return await r.json();
    }""", [url, data])


def _get_webdav_list_state(agent, uid):
    state = agent.get_component_state(uid)
    return _ref(state['setupState']['webdavList'])


def test_webdav(page, agent, root_path, workdir):
    comp = _navigate_to_profile(page, agent)
    uid = comp['id']

    # Switch to webdav tab
    agent.set_ref(uid, 'activeTab', 'webdav')
    time.sleep(1.0)

    # Verify initial state - empty list
    webdav_list = _get_webdav_list_state(agent, uid)
    assert isinstance(webdav_list, list)
    assert len(webdav_list) == 0

    # --- 1. Create WebDAV config (normal, non-global) ---
    res = _api_call(page, '/api/webdav/create', {
        'dav_path': 'photos',
        'access_path': 'photos',
        'password': 'testpass123',
        'permission': 'full_control',
        'global_access': False,
    })
    assert res['success'] is True

    # Reload list via component
    agent.call_method(uid, 'loadWebdavList')
    time.sleep(1.0)

    webdav_list = _get_webdav_list_state(agent, uid)
    assert len(webdav_list) == 1
    config = webdav_list[0]
    assert config['dav_path'] == 'photos'
    assert config['access_path'] == 'photos'
    assert config['permission'] == 'full_control'
    assert config['global_access'] is False
    assert config['url'] == '/dav/photos/'
    config_id_1 = config['id']

    # --- 2. Create second config (read_only) ---
    res = _api_call(page, '/api/webdav/create', {
        'dav_path': 'docs',
        'access_path': 'documents',
        'password': 'readonly_pass',
        'permission': 'read_only',
        'global_access': False,
    })
    assert res['success'] is True

    agent.call_method(uid, 'loadWebdavList')
    time.sleep(0.5)

    webdav_list = _get_webdav_list_state(agent, uid)
    assert len(webdav_list) == 2

    docs_config = next((c for c in webdav_list if c['dav_path'] == 'docs'), None)
    assert docs_config is not None
    assert docs_config['permission'] == 'read_only'
    assert docs_config['url'] == '/dav/docs/'

    # --- 3. Create with edit permission ---
    res = _api_call(page, '/api/webdav/create', {
        'dav_path': 'workspace',
        'access_path': 'work',
        'password': 'edit_pass',
        'permission': 'edit',
        'global_access': False,
    })
    assert res['success'] is True

    agent.call_method(uid, 'loadWebdavList')
    time.sleep(0.5)

    webdav_list = _get_webdav_list_state(agent, uid)
    assert len(webdav_list) == 3

    work_config = next((c for c in webdav_list if c['dav_path'] == 'workspace'), None)
    assert work_config is not None
    assert work_config['permission'] == 'edit'

    # --- 4. Cannot create global when non-global configs exist ---
    res = _api_call(page, '/api/webdav/create', {
        'dav_path': '',
        'access_path': '',
        'password': 'global_pass',
        'permission': 'full_control',
        'global_access': True,
    })
    assert res['success'] is False
    assert res['fail_code'] == 'DAV_CONFIGS_CONFLICT'

    # --- 5. Cannot create duplicate dav_path ---
    res = _api_call(page, '/api/webdav/create', {
        'dav_path': 'photos',
        'access_path': 'other',
        'password': 'dup_pass',
        'permission': 'read_only',
        'global_access': False,
    })
    assert res['success'] is False
    assert res['fail_code'] == 'DAV_PATH_DUPLICATE'

    # --- 6. Cannot create with invalid dav_path ---
    res = _api_call(page, '/api/webdav/create', {
        'dav_path': 'invalid/path',
        'access_path': 'test',
        'password': 'pass',
        'permission': 'full_control',
        'global_access': False,
    })
    assert res['success'] is False
    assert res['fail_code'] == 'DAV_PATH_INVALID'

    res = _api_call(page, '/api/webdav/create', {
        'dav_path': 'has space',
        'access_path': 'test',
        'password': 'pass',
        'permission': 'full_control',
        'global_access': False,
    })
    assert res['success'] is False
    assert res['fail_code'] == 'DAV_PATH_INVALID'

    # --- 7. Cannot create with empty password ---
    res = _api_call(page, '/api/webdav/create', {
        'dav_path': 'nopass',
        'access_path': 'test',
        'password': '',
        'permission': 'full_control',
        'global_access': False,
    })
    assert res['success'] is False
    assert res['fail_code'] == 'PARAM_INVALID'

    # --- 8. Cannot create with invalid permission ---
    res = _api_call(page, '/api/webdav/create', {
        'dav_path': 'badperm',
        'access_path': 'test',
        'password': 'pass',
        'permission': 'invalid',
        'global_access': False,
    })
    assert res['success'] is False
    assert res['fail_code'] == 'PARAM_INVALID'

    # --- 9. Update config (change permission) ---
    res = _api_call(page, '/api/webdav/update', {
        'id': config_id_1,
        'dav_path': 'photos',
        'access_path': 'photos',
        'password': 'newpass456',
        'permission': 'edit',
        'global_access': False,
    })
    assert res['success'] is True

    agent.call_method(uid, 'loadWebdavList')
    time.sleep(0.5)

    webdav_list = _get_webdav_list_state(agent, uid)
    updated = next((c for c in webdav_list if c['id'] == config_id_1), None)
    assert updated is not None
    assert updated['permission'] == 'edit'
    assert updated['dav_path'] == 'photos'

    # --- 10. Update without password (keep existing) ---
    res = _api_call(page, '/api/webdav/update', {
        'id': config_id_1,
        'dav_path': 'photos-v2',
        'access_path': 'photos',
        'permission': 'read_only',
        'global_access': False,
    })
    assert res['success'] is True

    agent.call_method(uid, 'loadWebdavList')
    time.sleep(0.5)

    webdav_list = _get_webdav_list_state(agent, uid)
    updated = next((c for c in webdav_list if c['id'] == config_id_1), None)
    assert updated['dav_path'] == 'photos-v2'
    assert updated['permission'] == 'read_only'

    # --- 11. Cannot update non-existent config ---
    res = _api_call(page, '/api/webdav/update', {
        'id': 'non-existent-id',
        'dav_path': 'test',
        'access_path': 'test',
        'password': 'pass',
        'permission': 'full_control',
        'global_access': False,
    })
    assert res['success'] is False
    assert res['fail_code'] == 'DAV_CONFIG_NOT_FOUND'

    # --- 12. Delete a config ---
    res = _api_call(page, '/api/webdav/delete', {'id': config_id_1})
    assert res['success'] is True

    agent.call_method(uid, 'loadWebdavList')
    time.sleep(0.5)

    webdav_list = _get_webdav_list_state(agent, uid)
    assert len(webdav_list) == 2
    assert not any(c['id'] == config_id_1 for c in webdav_list)

    # --- 13. Cannot delete non-existent config ---
    res = _api_call(page, '/api/webdav/delete', {'id': 'non-existent-id'})
    assert res['success'] is False
    assert res['fail_code'] == 'DAV_CONFIG_NOT_FOUND'

    # --- 14. Delete remaining configs, then create global ---
    for c in webdav_list:
        _api_call(page, '/api/webdav/delete', {'id': c['id']})

    agent.call_method(uid, 'loadWebdavList')
    time.sleep(0.5)

    webdav_list = _get_webdav_list_state(agent, uid)
    assert len(webdav_list) == 0

    # Create global config
    res = _api_call(page, '/api/webdav/create', {
        'dav_path': '',
        'access_path': '',
        'password': 'global_pass',
        'permission': 'full_control',
        'global_access': True,
    })
    assert res['success'] is True

    agent.call_method(uid, 'loadWebdavList')
    time.sleep(0.5)

    webdav_list = _get_webdav_list_state(agent, uid)
    assert len(webdav_list) == 1
    global_config = webdav_list[0]
    assert global_config['global_access'] is True
    assert global_config['dav_path'] == ''
    assert global_config['url'] == '/dav/'

    # --- 15. Cannot create another config when global exists ---
    res = _api_call(page, '/api/webdav/create', {
        'dav_path': 'another',
        'access_path': 'test',
        'password': 'pass',
        'permission': 'full_control',
        'global_access': False,
    })
    assert res['success'] is False
    assert res['fail_code'] == 'DAV_GLOBAL_EXISTS'

    # --- 16. Verify list endpoint directly ---
    list_res = _api_call(page, '/api/webdav/list', {})
    assert list_res['success'] is True
    assert len(list_res['configs']) == 1
    assert list_res['configs'][0]['global_access'] is True

    # Cleanup
    _api_call(page, '/api/webdav/delete', {'id': global_config['id']})


if __name__ == '__main__':
    run_frontend_test(test_webdav)

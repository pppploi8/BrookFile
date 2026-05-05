import os
import time
import requests
from test_utils import run_frontend_test, _ref, BASE_URL, FRONTEND_URL, DEFAULT_USERNAME, DEFAULT_PASSWORD


def _get_comp(agent, name):
    comps = agent.find_components_by_name(name)
    assert len(comps) > 0, f'{name} component not found'
    return comps[0]


def _navigate_to_profile(page, agent):
    page.evaluate('() => { const app = document.getElementById("app").__vue_app__; const router = app.config.globalProperties.$router; router.push("/profile"); }')
    time.sleep(1.5)
    agent.wait_ready(timeout=10000)


def _api_login(session):
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': DEFAULT_USERNAME,
        'password': DEFAULT_PASSWORD
    })


def test_profile_center(page, agent, root_path, workdir):
    _navigate_to_profile(page, agent)
    pc = _get_comp(agent, 'ProfileCenter')
    pc_uid = pc['id']

    # ===== Basic Info Tab =====
    state = agent.get_component_state(pc_uid)
    assert _ref(state['setupState']['activeTab']) == 'basic'

    user_info = _ref(state['setupState']['userInfo'])
    assert user_info['username'] == 'admin'

    # ===== 1. Change password - empty fields -> error =====
    pw_form = _ref(state['setupState']['passwordForm'])
    assert pw_form['currentPassword'] == ''
    assert pw_form['newPassword'] == ''
    assert pw_form['confirmPassword'] == ''

    agent.clear_messages()
    agent.call_method(pc_uid, 'handleChangePassword')
    msg = agent.wait_for_message('error', timeout=5000)
    assert msg is not None
    assert msg['key'] == 'profile.pleaseFillAllPasswordFields'

    # ===== 2. Change password - mismatch -> error =====
    agent.set_reactive_field(pc_uid, 'passwordForm', 'currentPassword', 'password123')
    agent.set_reactive_field(pc_uid, 'passwordForm', 'newPassword', 'newpass456')
    agent.set_reactive_field(pc_uid, 'passwordForm', 'confirmPassword', 'different789')

    agent.clear_messages()
    agent.call_method(pc_uid, 'handleChangePassword')
    msg = agent.wait_for_message('error', timeout=5000)
    assert msg is not None
    assert msg['key'] == 'profile.passwordMismatch'

    # ===== 3. Change password - wrong current password =====
    agent.set_reactive_field(pc_uid, 'passwordForm', 'currentPassword', 'wrongpassword')
    agent.set_reactive_field(pc_uid, 'passwordForm', 'newPassword', 'newpass456')
    agent.set_reactive_field(pc_uid, 'passwordForm', 'confirmPassword', 'newpass456')

    agent.call_method(pc_uid, 'handleChangePassword')
    time.sleep(1.0)

    # ===== 4. Change password - success via API =====
    time.sleep(1.0)
    s = requests.Session()
    _api_login(s)
    resp = s.post(f'{BASE_URL}/api/user/change_password', json={
        'old_password': 'password123',
        'new_password': 'newpass456'
    })
    assert resp.status_code == 200, f'change_password status={resp.status_code}'
    data = resp.json()
    assert data['success'] is True

    # Verify new password works
    s2 = requests.Session()
    resp2 = s2.post(f'{BASE_URL}/api/auth/login', json={'username': 'admin', 'password': 'newpass456'})
    assert resp2.json()['success'] is True

    # Reset password back
    s2.post(f'{BASE_URL}/api/user/change_password', json={
        'old_password': 'newpass456',
        'new_password': 'password123'
    })

    # Password change logs out browser - re-login
    time.sleep(3.0)
    page.goto(FRONTEND_URL + '/login')
    time.sleep(2.0)
    agent.wait_ready(timeout=10000)
    login_comp = _get_comp(agent, 'Login')
    login_uid = login_comp['id']
    agent.set_reactive_field(login_uid, 'form', 'username', 'admin')
    agent.set_reactive_field(login_uid, 'form', 'password', 'password123')
    agent.call_method(login_uid, 'handleLogin')
    page.wait_for_url('**/files', timeout=5000)

    # ===== 5. WebDAV Config Tab =====
    _navigate_to_profile(page, agent)
    pc = _get_comp(agent, 'ProfileCenter')
    pc_uid = pc['id']

    agent.call_method(pc_uid, 'handleTabChange', ['webdav'])
    time.sleep(1.0)

    state = agent.get_component_state(pc_uid)
    webdav_list = _ref(state['setupState']['webdavList'])
    assert isinstance(webdav_list, list)
    assert len(webdav_list) == 0

    # ===== 6. Add WebDAV configs via API =====
    s3 = requests.Session()
    _api_login(s3)

    resp = s3.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': 'photos',
        'access_path': 'photos',
        'password': 'davpass123',
        'permission': 'full_control',
        'global_access': False
    })
    assert resp.json()['success'] is True

    resp = s3.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': 'docs',
        'access_path': 'docs',
        'password': 'readonly123',
        'permission': 'read_only',
        'global_access': False
    })
    assert resp.json()['success'] is True

    # Reload WebDAV list in UI
    agent.call_method(pc_uid, 'loadWebdavList')
    time.sleep(0.5)

    state = agent.get_component_state(pc_uid)
    webdav_list = _ref(state['setupState']['webdavList'])
    assert len(webdav_list) == 2
    assert any(w['dav_path'] == 'photos' for w in webdav_list)
    assert any(w['dav_path'] == 'docs' for w in webdav_list)

    # ===== 7. Edit WebDAV config via drawer =====
    photos_config = next(w for w in webdav_list if w['dav_path'] == 'photos')
    agent.call_method(pc_uid, 'handleEditWebDav', [photos_config])
    time.sleep(0.5)

    state = agent.get_component_state(pc_uid)
    assert _ref(state['setupState']['webdavDrawerVisible']) is True
    assert _ref(state['setupState']['isEditWebDav']) is True

    form = _ref(agent.get_component_state(pc_uid)['setupState']['webdavForm'])
    assert form['dav_path'] == 'photos'

    # Update permission via API since FolderSelect in drawer is hard to interact with
    agent.set_ref(pc_uid, 'webdavDrawerVisible', False)
    time.sleep(0.3)

    s3.post(f'{BASE_URL}/api/webdav/update', json={
        'id': photos_config['id'],
        'dav_path': 'photos',
        'access_path': 'photos',
        'permission': 'edit',
        'global_access': False
    })
    agent.call_method(pc_uid, 'loadWebdavList')
    time.sleep(0.5)

    state = agent.get_component_state(pc_uid)
    webdav_list = _ref(state['setupState']['webdavList'])
    updated = next(w for w in webdav_list if w['dav_path'] == 'photos')
    assert updated['permission'] == 'edit'

    # ===== 8. Add global access config (should fail since others exist) =====
    resp = s3.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': '',
        'access_path': '',
        'password': 'globalpass123',
        'permission': 'full_control',
        'global_access': True
    })
    assert resp.json()['success'] is False

    # ===== 9. Delete one WebDAV config via API, verify list updates =====
    s3.post(f'{BASE_URL}/api/webdav/delete', json={'id': photos_config['id']})
    agent.call_method(pc_uid, 'loadWebdavList')
    time.sleep(0.5)

    state = agent.get_component_state(pc_uid)
    webdav_list = _ref(state['setupState']['webdavList'])
    assert len(webdav_list) == 1
    assert webdav_list[0]['dav_path'] == 'docs'

    # ===== 10. Delete remaining config =====
    s3.post(f'{BASE_URL}/api/webdav/delete', json={'id': webdav_list[0]['id']})
    agent.call_method(pc_uid, 'loadWebdavList')
    time.sleep(0.5)

    state = agent.get_component_state(pc_uid)
    webdav_list = _ref(state['setupState']['webdavList'])
    assert len(webdav_list) == 0

    # ===== 11. Add global access config (should succeed now that list is empty) =====
    resp = s3.post(f'{BASE_URL}/api/webdav/create', json={
        'dav_path': '',
        'access_path': '',
        'password': 'globalpass123',
        'permission': 'full_control',
        'global_access': True
    })
    assert resp.json()['success'] is True

    agent.call_method(pc_uid, 'loadWebdavList')
    time.sleep(0.5)

    state = agent.get_component_state(pc_uid)
    webdav_list = _ref(state['setupState']['webdavList'])
    assert len(webdav_list) == 1
    assert webdav_list[0]['global_access'] is True

    # Cleanup
    s3.post(f'{BASE_URL}/api/webdav/delete', json={'id': webdav_list[0]['id']})

    # ===== 12. Avatar upload/delete via API =====
    agent.call_method(pc_uid, 'handleTabChange', ['basic'])
    time.sleep(0.5)

    avatar_url = _ref(agent.get_component_state(pc_uid)['setupState']['avatarUrl'])
    assert avatar_url is None or avatar_url == ''

    test_image_path = os.path.join(workdir, 'test_avatar.png')
    with open(test_image_path, 'wb') as f:
        f.write(b'\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde\x00\x00\x00\x0cIDATx\x9cc\xf8\x0f\x00\x00\x01\x01\x00\x05\x18\xd8N\x00\x00\x00\x00IEND\xaeB`\x82')

    s4 = requests.Session()
    _api_login(s4)
    with open(test_image_path, 'rb') as f:
        resp = s4.post(f'{BASE_URL}/api/user/upload_avatar', files={'avatar': ('test.png', f, 'image/png')})
    assert resp.json()['success'] is True

    # Delete avatar
    resp = s4.post(f'{BASE_URL}/api/user/delete_avatar')
    assert resp.json()['success'] is True


if __name__ == '__main__':
    run_frontend_test(test_profile_center)

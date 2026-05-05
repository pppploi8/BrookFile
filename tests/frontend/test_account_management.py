import os
import time
import json
import requests
from test_utils import run_frontend_test, _ref, BASE_URL, DEFAULT_USERNAME, DEFAULT_PASSWORD


def _get_comp(agent, name):
    comps = agent.find_components_by_name(name)
    assert len(comps) > 0, f'{name} component not found'
    return comps[0]


def _navigate_to_accounts(page, agent):
    page.evaluate('() => { const app = document.getElementById("app").__vue_app__; const router = app.config.globalProperties.$router; router.push("/account-management"); }')
    time.sleep(1.5)
    agent.wait_ready(timeout=10000)


def _api_login(session):
    session.post(f'{BASE_URL}/api/auth/login', json={
        'username': DEFAULT_USERNAME,
        'password': DEFAULT_PASSWORD
    })


def test_account_management(page, agent, root_path, workdir):
    _navigate_to_accounts(page, agent)
    comp = _get_comp(agent, 'AccountManagement')
    uid = comp['id']

    state = agent.get_component_state(uid)
    user_list = _ref(state['setupState']['userList'])
    assert isinstance(user_list, list)
    admin_count = sum(1 for u in user_list if u.get('username') == 'admin')
    assert admin_count == 1

    # ===== 1. Add user - empty form validation =====
    agent.call_method(uid, 'handleAddUser')
    time.sleep(0.5)
    state = agent.get_component_state(uid)
    assert _ref(state['setupState']['drawerVisible']) is True
    assert _ref(state['setupState']['isEdit']) is False

    agent.call_method(uid, 'handleSubmit')
    time.sleep(0.3)
    assert _ref(agent.get_component_state(uid)['setupState']['drawerVisible']) is True

    # ===== 2. Add user - fill form and submit =====
    agent.set_reactive_field(uid, 'formData', 'username', 'testuser')
    agent.set_reactive_field(uid, 'formData', 'password', 'testpass123')
    agent.set_reactive_field(uid, 'formData', 'root_path', os.path.join(root_path, 'testuser_data'))
    agent.set_reactive_field(uid, 'formData', 'recycle_bin_path', os.path.join(root_path, 'testuser_recycle'))
    agent.set_reactive_field(uid, 'formData', 'is_admin', False)
    agent.set_reactive_field(uid, 'formData', 'remark', 'Test user remark')

    agent.clear_messages()
    agent.call_method(uid, 'handleSubmit')
    msg = agent.wait_for_message('success', timeout=5000)
    assert msg is not None
    assert msg['key'] == 'accountManagement.addSuccess'
    time.sleep(0.5)

    state = agent.get_component_state(uid)
    user_list = _ref(state['setupState']['userList'])
    assert any(u['username'] == 'testuser' for u in user_list)

    # ===== 3. Create root path dirs for testuser =====
    os.makedirs(os.path.join(root_path, 'testuser_data'), exist_ok=True)
    os.makedirs(os.path.join(root_path, 'testuser_recycle'), exist_ok=True)

    # ===== 4. Verify testuser can login =====
    session = requests.Session()
    resp = session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'testuser',
        'password': 'testpass123'
    })
    assert resp.json()['success'] is True

    # ===== 5. Add admin user =====
    comp = _get_comp(agent, 'AccountManagement')
    uid = comp['id']
    agent.call_method(uid, 'handleAddUser')
    time.sleep(0.5)

    agent.set_reactive_field(uid, 'formData', 'username', 'admin2')
    agent.set_reactive_field(uid, 'formData', 'password', 'adminpass123')
    agent.set_reactive_field(uid, 'formData', 'is_admin', True)

    agent.clear_messages()
    agent.call_method(uid, 'handleSubmit')
    msg = agent.wait_for_message('success', timeout=5000)
    assert msg is not None
    assert msg['key'] == 'accountManagement.addSuccess'
    time.sleep(0.5)

    # ===== 6. Edit user - modify remark =====
    state = agent.get_component_state(uid)
    user_list = _ref(state['setupState']['userList'])
    testuser = next(u for u in user_list if u['username'] == 'testuser')

    agent.call_method(uid, 'handleEdit', [testuser])
    time.sleep(0.5)
    state = agent.get_component_state(uid)
    assert _ref(state['setupState']['drawerVisible']) is True
    assert _ref(state['setupState']['isEdit']) is True

    form = _ref(agent.get_component_state(uid)['setupState']['formData'])
    assert form['username'] == 'testuser'

    agent.set_reactive_field(uid, 'formData', 'remark', 'Updated remark')

    agent.clear_messages()
    agent.call_method(uid, 'handleSubmit')
    msg = agent.wait_for_message('success', timeout=5000)
    assert msg is not None
    assert msg['key'] == 'accountManagement.editSuccess'
    time.sleep(0.5)

    # ===== 7. Delete user via API (ElMessageBox confirm is hard to interact via agent) =====
    state = agent.get_component_state(uid)
    user_list = _ref(state['setupState']['userList'])
    admin2 = next((u for u in user_list if u['username'] == 'admin2'), None)

    if admin2:
        s = requests.Session()
        _api_login(s)
        resp = s.post(f'{BASE_URL}/api/user/delete', json={'id': admin2['id']})
        assert resp.json()['success'] is True

        agent.call_method(uid, 'fetchUserList')
        time.sleep(0.5)
        state = agent.get_component_state(uid)
        user_list = _ref(state['setupState']['userList'])
        assert not any(u['username'] == 'admin2' for u in user_list)

    # ===== 8. Add user with expire time =====
    comp = _get_comp(agent, 'AccountManagement')
    uid = comp['id']
    agent.call_method(uid, 'handleAddUser')
    time.sleep(0.5)

    agent.set_reactive_field(uid, 'formData', 'username', 'expiring_user')
    agent.set_reactive_field(uid, 'formData', 'password', 'expirepass123')

    from datetime import datetime, timedelta, timezone
    expire = datetime.now(timezone.utc) + timedelta(days=30)
    agent.set_reactive_field(uid, 'formData', 'expire_at', expire.isoformat())

    agent.clear_messages()
    agent.call_method(uid, 'handleSubmit')
    msg = agent.wait_for_message('success', timeout=5000)
    assert msg is not None
    assert msg['key'] == 'accountManagement.addSuccess'
    time.sleep(0.5)

    # ===== 9. Verify expiring user appears in list =====
    state = agent.get_component_state(uid)
    user_list = _ref(state['setupState']['userList'])
    expiring = next((u for u in user_list if u['username'] == 'expiring_user'), None)
    assert expiring is not None
    assert expiring['expire_at'] is not None

    # ===== 10. Delete expiring user via API =====
    s = requests.Session()
    _api_login(s)
    s.post(f'{BASE_URL}/api/user/delete', json={'id': expiring['id']})

    # ===== 11. Close drawer if open =====
    state = agent.get_component_state(uid)
    if _ref(state['setupState']['drawerVisible']):
        agent.set_ref(uid, 'drawerVisible', False)
        time.sleep(0.3)

    # ===== 12. Verify admin cannot be deleted =====
    state = agent.get_component_state(uid)
    user_list = _ref(state['setupState']['userList'])
    admin_user = next(u for u in user_list if u['username'] == 'admin')

    s = requests.Session()
    _api_login(s)
    resp = s.post(f'{BASE_URL}/api/user/delete', json={'id': admin_user['id']})
    data = resp.json()
    assert data['success'] is False

    # ===== 13. H8: Canceling delete should not show NETWORK_ERROR =====
    comp = _get_comp(agent, 'AccountManagement')
    uid = comp['id']

    agent.clear_messages()
    page.evaluate(
        '() => {'
        '  const comps = window.__vue_agent__.findComponentsByName("AccountManagement");'
        '  if (comps.length === 0) return;'
        '  const uid = comps[0].id;'
        '  const state = window.__vue_agent__.getComponentState(uid);'
        '  const userList = state.setupState.userList.__ref || state.setupState.userList;'
        '  const firstUser = userList.value ? userList.value[0] : userList[0];'
        '  window.__vue_agent__.callMethod(uid, "handleDelete", [firstUser]);'
        '}'
    )
    time.sleep(1)

    try:
        msgbox = page.locator('.el-message-box')
        if msgbox.count() > 0:
            cancel_btns = page.locator('.el-message-box__btns .el-button--default')
            if cancel_btns.count() > 0:
                cancel_btns.first.click()
            else:
                msgbox.locator('button').first.click()
            time.sleep(1)
    except Exception:
        pass

    msgs = agent.get_messages()
    network_error_msgs = [m for m in msgs if m.get('type') == 'error' and 'NETWORK_ERROR' in json.dumps(m)]
    assert len(network_error_msgs) == 0, \
        'H8 BUG: Canceling delete confirmation shows false NETWORK_ERROR!'

    # ===== 14. H9: Form validation failure should not show NETWORK_ERROR =====
    _navigate_to_accounts(page, agent)
    comp = _get_comp(agent, 'AccountManagement')
    uid = comp['id']

    agent.call_method(uid, 'handleAddUser')
    time.sleep(0.5)

    agent.set_reactive_field(uid, 'formData', 'username', 'test_val_user')
    agent.set_reactive_field(uid, 'formData', 'password', '')

    agent.clear_messages()
    agent.call_method(uid, 'handleSubmit')
    time.sleep(1)

    msgs = agent.get_messages()
    network_error_msgs = [m for m in msgs if m.get('type') == 'error' and 'NETWORK_ERROR' in json.dumps(m)]
    assert len(network_error_msgs) == 0, \
        'H9 BUG: Form validation failure shows false NETWORK_ERROR!'

    state = agent.get_component_state(uid)
    if _ref(state['setupState']['drawerVisible']):
        agent.set_ref(uid, 'drawerVisible', False)
        time.sleep(0.3)


if __name__ == '__main__':
    run_frontend_test(test_account_management)

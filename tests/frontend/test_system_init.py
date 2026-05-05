import os
import time
from test_utils import run_frontend_test, FRONTEND_URL, _ref


def _get_init_uid(agent):
    comps = agent.find_components_by_name('Init')
    assert len(comps) > 0, 'Init component not found'
    return comps[0]['id']


def _get_login_uid(agent):
    comps = agent.find_components_by_name('Login')
    assert len(comps) > 0, 'Login component not found'
    return comps[0]['id']


def _test_init_page(page, agent, root_path, workdir):
    page.goto(FRONTEND_URL)
    page.wait_for_url('**/init', timeout=5000)
    agent.wait_ready(timeout=15000)

    uid = _get_init_uid(agent)

    # --- User store: uninitialized ---
    user_store = agent.get_store('user')
    assert user_store['initialized'] is False
    assert user_store['loggedIn'] is False
    assert user_store['systemName'] == 'BrookFile'

    # --- Form initial state ---
    state = agent.get_component_state(uid)
    form = _ref(state['setupState']['form'])
    assert form['systemName'] == 'BrookFile'
    assert form['username'] == ''
    assert form['password'] == ''
    assert form['confirmPassword'] == ''
    assert form['rootPath'] == ''
    assert form['recycleBinPath'] == ''
    assert _ref(state['setupState']['loading']) is False
    assert _ref(state['setupState']['folderDialogVisible']) is False

    # --- Submit with empty form → validation rejects ---
    agent.call_method(uid, 'handleInit')
    time.sleep(0.5)
    assert page.url.rstrip('/').endswith('/init')
    assert agent.get_store('user')['initialized'] is False

    # --- Fill only username, submit → still rejected ---
    agent.set_reactive_field(uid, 'form', 'username', 'admin')
    agent.call_method(uid, 'handleInit')
    time.sleep(0.5)
    assert page.url.rstrip('/').endswith('/init')

    # --- Password mismatch → rejected ---
    agent.set_reactive_field(uid, 'form', 'systemName', 'TestSystem')
    agent.set_reactive_field(uid, 'form', 'password', 'password123')
    agent.set_reactive_field(uid, 'form', 'confirmPassword', 'different')
    agent.call_method(uid, 'handleInit')
    time.sleep(0.5)
    assert page.url.rstrip('/').endswith('/init')

    # --- Folder dialog: browse, navigate, select, backfill rootPath ---
    browse_root = os.path.join(workdir, 'browse_test')
    sub_dir = os.path.join(browse_root, 'sub_folder')
    os.makedirs(sub_dir)

    agent.call_method(uid, 'openFolderDialog', ['rootPath'])
    time.sleep(0.5)
    assert _ref(agent.get_component_state(uid)['setupState']['folderDialogVisible']) is True

    agent.call_method(uid, 'loadFolders', [workdir])
    time.sleep(0.5)
    state = agent.get_component_state(uid)
    browse_data = _ref(state['setupState']['browseData'])
    assert len(browse_data['folders']) > 0
    assert browse_data['has_parent'] is True

    agent.call_method(uid, 'enterFolder', [next(f for f in browse_data['folders'] if f['name'] == 'browse_test')])
    time.sleep(0.5)
    assert _ref(agent.get_component_state(uid)['setupState']['currentBrowsePath']) == browse_root

    agent.call_method(uid, 'enterFolder', [next(f for f in _ref(agent.get_component_state(uid)['setupState']['browseData'])['folders'] if f['name'] == 'sub_folder')])
    time.sleep(0.5)
    assert _ref(agent.get_component_state(uid)['setupState']['currentBrowsePath']) == sub_dir

    agent.call_method(uid, 'goToParent')
    time.sleep(0.5)
    assert _ref(agent.get_component_state(uid)['setupState']['currentBrowsePath']) == browse_root

    agent.call_method(uid, 'confirmFolderSelection')
    time.sleep(0.3)
    assert _ref(agent.get_component_state(uid)['setupState']['form'])['rootPath'] == browse_root
    assert _ref(agent.get_component_state(uid)['setupState']['folderDialogVisible']) is False

    # --- Folder dialog: select recycleBinPath ---
    recycle_dir = os.path.join(workdir, 'recycle_select')
    os.makedirs(recycle_dir)
    agent.call_method(uid, 'openFolderDialog', ['recycleBinPath'])
    time.sleep(0.5)
    agent.call_method(uid, 'loadFolders', [recycle_dir])
    time.sleep(0.5)
    agent.call_method(uid, 'confirmFolderSelection')
    time.sleep(0.3)
    assert _ref(agent.get_component_state(uid)['setupState']['form'])['recycleBinPath'] == recycle_dir

    # --- Valid submit → init success → redirects to /login ---
    agent.set_reactive_field(uid, 'form', 'username', 'admin')
    agent.set_reactive_field(uid, 'form', 'password', 'password123')
    agent.set_reactive_field(uid, 'form', 'confirmPassword', 'password123')
    agent.call_method(uid, 'handleInit')
    page.wait_for_url('**/login', timeout=5000)

    user_store = agent.get_store('user')
    assert user_store['initialized'] is True
    assert user_store['systemName'] == 'TestSystem'

    # --- /init redirects to /login ---
    page.goto(FRONTEND_URL + '/init')
    agent.wait_ready()
    page.wait_for_url('**/login', timeout=5000)


def _test_login_page(page, agent):
    agent.wait_ready()
    uid = _get_login_uid(agent)

    # --- Form initial state ---
    state = agent.get_component_state(uid)
    form = _ref(state['setupState']['form'])
    assert form['username'] == ''
    assert form['password'] == ''
    assert _ref(state['setupState']['loading']) is False

    # --- System name displayed ---
    assert agent.get_store('user')['systemName'] == 'TestSystem'

    # --- Empty form → rejected ---
    agent.clear_messages()
    agent.call_method(uid, 'handleLogin')
    time.sleep(0.5)
    assert page.url.rstrip('/').endswith('/login')
    assert agent.get_store('user')['loggedIn'] is False

    # --- Only username → rejected ---
    agent.set_reactive_field(uid, 'form', 'username', 'admin')
    agent.call_method(uid, 'handleLogin')
    time.sleep(0.5)
    assert page.url.rstrip('/').endswith('/login')

    # --- Only password → rejected ---
    agent.set_reactive_field(uid, 'form', 'username', '')
    agent.set_reactive_field(uid, 'form', 'password', 'password123')
    agent.call_method(uid, 'handleLogin')
    time.sleep(0.5)
    assert page.url.rstrip('/').endswith('/login')

    # --- Wrong password → error ---
    agent.clear_messages()
    agent.set_reactive_field(uid, 'form', 'username', 'admin')
    agent.set_reactive_field(uid, 'form', 'password', 'wrongpassword')
    agent.call_method(uid, 'handleLogin')
    msg = agent.wait_for_message('error', timeout=5000)
    assert msg is not None
    assert msg['key'] is not None
    assert page.url.rstrip('/').endswith('/login')

    # --- Non-existent user → error ---
    agent.clear_messages()
    agent.set_reactive_field(uid, 'form', 'username', 'nonexistent')
    agent.set_reactive_field(uid, 'form', 'password', 'password123')
    agent.call_method(uid, 'handleLogin')
    msg = agent.wait_for_message('error', timeout=5000)
    assert msg is not None
    assert msg['key'] is not None
    assert page.url.rstrip('/').endswith('/login')

    # --- Valid login → success → /files ---
    agent.clear_messages()
    agent.set_reactive_field(uid, 'form', 'username', 'admin')
    agent.set_reactive_field(uid, 'form', 'password', 'password123')
    agent.call_method(uid, 'handleLogin')
    msg = agent.wait_for_message('success', timeout=5000)
    assert msg is not None
    assert msg['key'] == 'login.success'
    page.wait_for_url('**/files', timeout=5000)

    user_store = agent.get_store('user')
    assert user_store['loggedIn'] is True
    assert user_store['user'] is not None
    assert user_store['user']['username'] == 'admin'
    assert user_store['user']['is_admin'] is True


def test_system_init(page, agent, root_path, workdir):
    _test_init_page(page, agent, root_path, workdir)
    _test_login_page(page, agent)


def test_system_init_mobile(page, agent, root_path, workdir):
    _test_init_page(page, agent, root_path, workdir)
    _test_login_page(page, agent)


if __name__ == '__main__':
    import sys
    mobile = '--mobile' in sys.argv
    if mobile:
        run_frontend_test(test_system_init_mobile, init=False, viewport={'width': 375, 'height': 812})
    else:
        run_frontend_test(test_system_init, init=False)

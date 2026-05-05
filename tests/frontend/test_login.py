import time
from test_utils import run_frontend_test, _ref, BASE_URL


def _get_comp(agent, name):
    comps = agent.find_components_by_name(name)
    assert len(comps) > 0, f'{name} component not found'
    return comps[0]


def test_login(page, agent, root_path, workdir):
    # Already logged in via run_frontend_test - first verify we're on files page
    page.wait_for_url('**/files', timeout=5000)
    agent.wait_ready(timeout=10000)

    # ===== 1. Verify logged-in state =====
    user_store = agent.get_store('user')
    assert user_store['loggedIn'] is True
    assert user_store['user']['username'] == 'admin'

    # ===== 2. Logout via store, navigate to login page =====
    page.evaluate("""() => {
        const app = document.getElementById('app').__vue_app__;
        const store = app.config.globalProperties.$pinia._s.get('user');
        if (store) store.logout();
    }""")
    time.sleep(1.0)

    page.evaluate("""() => {
        const app = document.getElementById('app').__vue_app__;
        const router = app.config.globalProperties.$router;
        router.push('/login');
    }""")
    time.sleep(1.0)
    agent.wait_ready(timeout=10000)

    comp = _get_comp(agent, 'Login')
    uid = comp['id']

    # ===== 3. Initial form state =====
    state = agent.get_component_state(uid)
    form = _ref(state['setupState']['form'])
    assert form['username'] == ''
    assert form['password'] == ''
    assert _ref(state['setupState']['loading']) is False

    # ===== 4. Submit empty form -> validation rejects =====
    agent.call_method(uid, 'handleLogin')
    time.sleep(0.5)

    # ===== 5. Only username -> validation rejects =====
    agent.set_reactive_field(uid, 'form', 'username', 'admin')
    agent.call_method(uid, 'handleLogin')
    time.sleep(0.5)

    # ===== 6. Only password -> validation rejects =====
    agent.set_reactive_field(uid, 'form', 'username', '')
    agent.set_reactive_field(uid, 'form', 'password', 'password123')
    agent.call_method(uid, 'handleLogin')
    time.sleep(0.5)

    # ===== 7. Wrong password -> error message, stays on login =====
    agent.set_reactive_field(uid, 'form', 'username', 'admin')
    agent.set_reactive_field(uid, 'form', 'password', 'wrongpassword')
    agent.clear_messages()
    agent.call_method(uid, 'handleLogin')
    time.sleep(1.0)
    assert page.url.rstrip('/').endswith('/login')

    # ===== 8. Non-existent user -> error =====
    agent.set_reactive_field(uid, 'form', 'username', 'nonexistent')
    agent.set_reactive_field(uid, 'form', 'password', 'password123')
    agent.clear_messages()
    agent.call_method(uid, 'handleLogin')
    time.sleep(1.0)
    assert page.url.rstrip('/').endswith('/login')

    # ===== 9. Valid login -> success message + redirect to /files =====
    agent.set_reactive_field(uid, 'form', 'username', 'admin')
    agent.set_reactive_field(uid, 'form', 'password', 'password123')
    agent.clear_messages()
    agent.call_method(uid, 'handleLogin')
    msg = agent.wait_for_message('success', timeout=5000)
    assert msg is not None
    assert msg['key'] == 'login.success'
    page.wait_for_url('**/files', timeout=5000)

    # ===== 10. Verify user store after login =====
    user_store = agent.get_store('user')
    assert user_store['loggedIn'] is True
    assert user_store['user'] is not None
    assert user_store['user']['username'] == 'admin'
    assert user_store['user']['is_admin'] is True


if __name__ == '__main__':
    run_frontend_test(test_login, init=True)

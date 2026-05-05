import os
import time
from test_utils import run_frontend_test, _ref


def _get_share_comp(agent):
    comps = agent.find_components_by_name('SharePage')
    assert len(comps) > 0, 'SharePage component not found'
    return comps[0]


def _wait_share_loaded(agent, uid, timeout=10):
    deadline = time.time() + timeout
    while time.time() < deadline:
        state = agent.get_component_state(uid)
        share_info = _ref(state['setupState']['shareInfo'])
        error_type = _ref(state['setupState']['errorType'])
        if share_info is not None or error_type:
            return state
        time.sleep(0.3)
    return agent.get_component_state(uid)


def _create_share_via_api(page, file_path, share_mode='page', password=None, expire_type='permanent', max_downloads=None):
    body = {
        'file_path': file_path,
        'share_mode': share_mode,
        'expire_type': expire_type,
    }
    if password:
        body['password'] = password
    if max_downloads is not None:
        body['max_downloads'] = max_downloads

    result = page.evaluate("""async (body) => {
        const resp = await fetch('/api/share/create', {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify(body),
            credentials: 'include'
        });
        return await resp.json();
    }""", body)
    return result


def test_share_page(page, agent, root_path, workdir):
    page.wait_for_url('**/files', timeout=5000)
    agent.wait_ready(timeout=15000)

    with open(os.path.join(root_path, 'share_file.txt'), 'w') as f:
        f.write('shareable content here!')
    os.makedirs(os.path.join(root_path, 'share_folder'))
    with open(os.path.join(root_path, 'share_folder', 'inner.txt'), 'w') as f:
        f.write('inner file')

    # --- 1. No-password share ---
    result = _create_share_via_api(page, 'share_file.txt')
    assert result['success'], f'Create share failed: {result}'
    share_code = result['share_code']
    assert len(share_code) > 0

    page.evaluate('(code) => { const app = document.getElementById("app").__vue_app__; const router = app.config.globalProperties.$router; router.push("/s/" + code); }', share_code)
    time.sleep(2)

    comp = _get_share_comp(agent)
    uid = comp['id']
    state = _wait_share_loaded(agent, uid)

    share_info = _ref(state['setupState']['shareInfo'])
    assert share_info is not None
    assert share_info['file_name'] == 'share_file.txt'
    assert share_info['is_directory'] is False
    assert share_info['need_password'] is False
    assert share_info['share_mode'] == 'page'

    # Download - just trigger and verify downloading state changes
    agent.call_method(uid, 'handleDownload')
    time.sleep(3)
    state = agent.get_component_state(uid)
    downloading = _ref(state['setupState']['downloading'])
    assert downloading is False

    # --- 2. Password share - no password entered ---
    result = _create_share_via_api(page, 'share_folder', password='secret123')
    assert result['success'], f'Create share failed: {result}'
    pw_share_code = result['share_code']

    page.evaluate('(code) => { const app = document.getElementById("app").__vue_app__; const router = app.config.globalProperties.$router; router.push("/s/" + code); }', pw_share_code)
    time.sleep(2)

    comp = _get_share_comp(agent)
    uid = comp['id']
    state = _wait_share_loaded(agent, uid)

    share_info = _ref(state['setupState']['shareInfo'])
    assert share_info is not None
    assert share_info['need_password'] is True
    assert share_info['is_directory'] is True
    assert share_info['is_directory'] is True

    agent.set_ref(uid, 'password', '')
    agent.clear_messages()
    agent.call_method(uid, 'handleDownload')
    msg = agent.wait_for_message('warning', timeout=5000)
    assert msg is not None
    assert msg['key'] == 'share.passwordRequired'

    # --- 3. Password share - wrong password ---
    agent.set_ref(uid, 'password', 'wrongpass')
    agent.clear_messages()
    agent.call_method(uid, 'handleDownload')
    msg = agent.wait_for_message('error', timeout=5000)
    assert msg is not None
    assert msg['key'] == 'share.passwordWrong'

    # --- 4. Password share - correct password ---
    # Need to get the salt from shareInfo and compute HMAC-SHA256
    # The frontend does this internally via CryptoJS.HmacSHA256
    # So just set the correct password and let the component handle it
    agent.set_ref(uid, 'password', 'secret123')
    agent.clear_messages()
    agent.call_method(uid, 'handleDownload')
    time.sleep(3)

    state = agent.get_component_state(uid)
    downloading = _ref(state['setupState']['downloading'])
    assert downloading is False

    # --- 5. Non-existent share code ---
    page.evaluate('() => { const app = document.getElementById("app").__vue_app__; const router = app.config.globalProperties.$router; router.push("/s/ZZZZZZZZ"); }')
    time.sleep(2)

    comp = _get_share_comp(agent)
    uid = comp['id']
    state = _wait_share_loaded(agent, uid)

    error_type = _ref(state['setupState']['errorType'])
    assert error_type == 'SHARE_NOT_FOUND'
    share_info = _ref(state['setupState']['shareInfo'])
    assert share_info is None

    # --- 6. Direct mode share ---
    page.evaluate('() => { const app = document.getElementById("app").__vue_app__; const router = app.config.globalProperties.$router; router.push("/files"); }')
    time.sleep(1)

    # Delete existing share for share_file.txt first
    page.evaluate("""async () => {
        const res = await fetch('/api/share/get_by_path', {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({file_path: 'share_file.txt'}),
            credentials: 'include'
        });
        const data = await res.json();
        if (data.share && data.share.id) {
            await fetch('/api/share/delete', {
                method: 'POST',
                headers: {'Content-Type': 'application/json'},
                body: JSON.stringify({ids: [data.share.id]}),
                credentials: 'include'
            });
        }
    }""")

    result = _create_share_via_api(page, 'share_file.txt', share_mode='direct')
    assert result['success'], f'Direct share failed: {result}'
    direct_code = result['share_code']

    page.evaluate('(code) => { const app = document.getElementById("app").__vue_app__; const router = app.config.globalProperties.$router; router.push("/s/" + code); }', direct_code)
    time.sleep(2)

    comp = _get_share_comp(agent)
    uid = comp['id']
    state = _wait_share_loaded(agent, uid)

    share_info = _ref(state['setupState']['shareInfo'])
    assert share_info is not None
    assert share_info['share_mode'] == 'direct'


if __name__ == '__main__':
    run_frontend_test(test_share_page)

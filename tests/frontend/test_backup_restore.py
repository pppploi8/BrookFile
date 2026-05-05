import os
import time
import sys
import tempfile
import hashlib

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
from common import build_backend, start_backend, start_frontend, init_system, login, BASE_URL, FRONTEND_URL, DEFAULT_USERNAME, DEFAULT_PASSWORD
from test_utils import _ref, VueAgent

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'backend'))
from mock_webdav_server import MockWebDAVServer

MOCK_PORT = 15244
MOCK_URL = f'http://127.0.0.1:{MOCK_PORT}'
MOCK_USER = 'admin'
MOCK_PASS = 'admin123'
MOCK_PATH = '/testbackup'


def _storage_config(path=MOCK_PATH):
    return {
        'address': MOCK_URL,
        'username': MOCK_USER,
        'password': MOCK_PASS,
        'path': path,
    }


def _sha256(content):
    if isinstance(content, str):
        content = content.encode()
    return hashlib.sha256(content).hexdigest()


def _make_index(entries):
    lines = []
    for e in entries:
        line = f'{e[0]}\t{e[1]}\t{e[2]}'
        if len(e) >= 4 and e[3]:
            line += f'\t{e[3]}'
        lines.append(line)
    return '\n'.join(lines) + '\n'


def _get_comp(agent, name):
    comps = agent.find_components_by_name(name)
    assert len(comps) > 0, f'{name} component not found'
    return comps[0]


def _navigate_to_profile(page, agent):
    page.evaluate('() => { const app = document.getElementById("app").__vue_app__; const router = app.config.globalProperties.$router; router.push("/profile"); }')
    time.sleep(1.5)
    agent.wait_ready(timeout=10000)


def _switch_to_backup_tab(agent, uid):
    agent.call_method(uid, 'handleTabChange', ['backup'])
    time.sleep(0.5)


def _wait_message(agent, msg_type, timeout=5000):
    return agent.wait_for_message(msg_type, timeout=timeout)


def _wait_backup_done_via_api(session, rule_id, timeout=120):
    import requests as req
    deadline = time.time() + timeout
    while time.time() < deadline:
        resp = req.post(f'{BASE_URL}/api/backup/progress', json={'rule_id': rule_id})
        data = resp.json()
        if not data.get('is_running', False):
            return data
        time.sleep(1)
    raise AssertionError(f'Backup timeout ({timeout}s)')


def _wait_restore_done_via_api(task_id, timeout=120):
    import requests as req
    deadline = time.time() + timeout
    while time.time() < deadline:
        resp = req.post(f'{BASE_URL}/api/restore/progress', json={'task_id': task_id})
        data = resp.json()
        if not data.get('is_running', False):
            return data
        time.sleep(1)
    raise AssertionError(f'Restore timeout ({timeout}s)')


def test_backup_restore_frontend(page, agent, root_path, workdir):
    import requests as req

    _navigate_to_profile(page, agent)
    pc = _get_comp(agent, 'ProfileCenter')
    pc_uid = pc['id']

    _switch_to_backup_tab(agent, pc_uid)
    time.sleep(0.5)

    state = agent.get_component_state(pc_uid)
    backup_list = _ref(state['setupState']['backupList'])
    assert isinstance(backup_list, list)
    assert len(backup_list) == 0

    # ===== 1. Add backup rule (empty form -> validation) =====
    agent.call_method(pc_uid, 'handleAddBackup')
    time.sleep(0.5)
    state = agent.get_component_state(pc_uid)
    assert _ref(state['setupState']['drawerVisible']) is True
    assert _ref(state['setupState']['isEdit']) is False

    agent.clear_messages()
    agent.call_method(pc_uid, 'handleSaveBackup')
    time.sleep(0.5)
    assert _ref(agent.get_component_state(pc_uid)['setupState']['drawerVisible']) is True

    # ===== 2. Add backup rule (fill form -> success) =====
    form = _ref(agent.get_component_state(pc_uid)['setupState']['backupForm'])
    assert form['name'] == ''
    assert form['type'] == 'webdav'
    assert form['encrypted'] is False

    agent.set_reactive_field(pc_uid, 'backupForm', 'name', 'Test Backup')
    agent.set_reactive_field(pc_uid, 'backupForm', 'webdavAddress', MOCK_URL)
    agent.set_reactive_field(pc_uid, 'backupForm', 'webdavUsername', MOCK_USER)
    agent.set_reactive_field(pc_uid, 'backupForm', 'webdavPassword', MOCK_PASS)
    agent.set_reactive_field(pc_uid, 'backupForm', 'webdavPath', MOCK_PATH)
    agent.set_reactive_field(pc_uid, 'backupForm', 'path', 'backup_test')
    agent.set_reactive_field(pc_uid, 'backupForm', 'cycle', 'daily')
    agent.set_reactive_field(pc_uid, 'backupForm', 'backupTime', '08:00')

    test_dir = os.path.join(root_path, 'backup_test')
    os.makedirs(test_dir, exist_ok=True)
    with open(os.path.join(test_dir, 'hello.txt'), 'w') as f:
        f.write('Hello World\n')
    os.makedirs(os.path.join(test_dir, 'subdir'), exist_ok=True)
    with open(os.path.join(test_dir, 'subdir', 'nested.txt'), 'w') as f:
        f.write('Nested file content\n')

    agent.clear_messages()
    agent.call_method(pc_uid, 'handleSaveBackup')
    msg = _wait_message(agent, 'success')
    assert msg is not None
    assert msg['key'] == 'profile.addBackupSuccess'
    time.sleep(0.5)

    state = agent.get_component_state(pc_uid)
    backup_list = _ref(state['setupState']['backupList'])
    assert len(backup_list) == 1
    assert backup_list[0]['name'] == 'Test Backup'
    rule_id = backup_list[0]['id']

    # ===== 3. Edit backup rule =====
    agent.call_method(pc_uid, 'handleEditBackup', [backup_list[0]])
    time.sleep(0.5)
    state = agent.get_component_state(pc_uid)
    assert _ref(state['setupState']['drawerVisible']) is True
    assert _ref(state['setupState']['isEdit']) is True

    form = _ref(agent.get_component_state(pc_uid)['setupState']['backupForm'])
    assert form['name'] == 'Test Backup'
    assert form['webdavAddress'] == MOCK_URL

    agent.set_reactive_field(pc_uid, 'backupForm', 'name', 'Test Backup Edited')
    agent.clear_messages()
    agent.call_method(pc_uid, 'handleSaveBackup')
    msg = _wait_message(agent, 'success')
    assert msg is not None
    assert msg['key'] == 'profile.editBackupSuccess'
    time.sleep(0.5)

    state = agent.get_component_state(pc_uid)
    backup_list = _ref(state['setupState']['backupList'])
    assert backup_list[0]['name'] == 'Test Backup Edited'

    # ===== 4. Add encrypted backup rule without password -> error =====
    agent.call_method(pc_uid, 'handleAddBackup')
    time.sleep(0.5)
    agent.set_reactive_field(pc_uid, 'backupForm', 'name', 'Enc Backup')
    agent.set_reactive_field(pc_uid, 'backupForm', 'webdavAddress', MOCK_URL)
    agent.set_reactive_field(pc_uid, 'backupForm', 'webdavUsername', MOCK_USER)
    agent.set_reactive_field(pc_uid, 'backupForm', 'webdavPassword', MOCK_PASS)
    agent.set_reactive_field(pc_uid, 'backupForm', 'webdavPath', '/testbackup_enc')
    agent.set_reactive_field(pc_uid, 'backupForm', 'path', 'encrypt_test')
    agent.set_reactive_field(pc_uid, 'backupForm', 'encrypted', True)
    agent.set_reactive_field(pc_uid, 'backupForm', 'cycle', 'daily')
    agent.set_reactive_field(pc_uid, 'backupForm', 'backupTime', '12:00')

    enc_test_dir = os.path.join(root_path, 'encrypt_test')
    os.makedirs(enc_test_dir, exist_ok=True)
    with open(os.path.join(enc_test_dir, 'secret.txt'), 'w') as f:
        f.write('Secret data\n')

    agent.clear_messages()
    agent.call_method(pc_uid, 'handleSaveBackup')
    msg = _wait_message(agent, 'error')
    assert msg is not None
    assert msg['key'] == 'profile.backupPasswordRequired'

    # ===== 5. Add encrypted backup rule with password -> success =====
    agent.set_reactive_field(pc_uid, 'backupForm', 'backupPassword', 'enc_pass_123')
    agent.clear_messages()
    agent.call_method(pc_uid, 'handleSaveBackup')
    msg = _wait_message(agent, 'success')
    assert msg is not None
    assert msg['key'] == 'profile.addBackupSuccess'
    time.sleep(0.5)

    state = agent.get_component_state(pc_uid)
    backup_list = _ref(state['setupState']['backupList'])
    assert len(backup_list) == 2
    enc_rule_id = next(r['id'] for r in backup_list if r['name'] == 'Enc Backup')

    # ===== 6. Backup Now via BackupLogDrawer =====
    agent.call_method(pc_uid, 'handleViewLog', [backup_list[0]])
    time.sleep(1.0)

    log_comp = _get_comp(agent, 'BackupLogDrawer')
    log_uid = log_comp['id']
    assert _ref(agent.get_component_state(log_uid)['setupState']['visible']) is True

    agent.clear_messages()
    agent.call_method(log_uid, 'handleBackupNow')
    msg = _wait_message(agent, 'success')
    assert msg is not None
    assert msg['key'] == 'backupLog.startBackupSuccess'
    time.sleep(1.0)

    _wait_backup_done_via_api(req.Session(), rule_id)
    time.sleep(1.0)

    state = agent.get_component_state(log_uid)
    assert _ref(state['setupState']['isTaskRunning']) is False

    # ===== 7. Check backup progress shows completed status in history =====
    agent.call_method(log_uid, 'loadBackupLogs')
    time.sleep(0.5)
    state = agent.get_component_state(log_uid)
    history = _ref(state['setupState']['historyData'])
    assert len(history) >= 1
    assert history[0]['status'] in ('completed', 'partial')
    assert history[0]['backup_success_count'] >= 2

    # ===== 8. Close log drawer =====
    agent.set_ref(log_uid, 'visible', False)
    time.sleep(0.5)

    # ===== 9. Run encrypted backup via API, then test restore =====
    session = req.Session()
    session.post(f'{BASE_URL}/api/auth/login', json={'username': DEFAULT_USERNAME, 'password': DEFAULT_PASSWORD})
    session.post(f'{BASE_URL}/api/backup/start', json={'rule_id': enc_rule_id, 'mode': 'full'})
    _wait_backup_done_via_api(session, enc_rule_id)

    # ===== 10. Restore - open drawer, fill form =====
    pc = _get_comp(agent, 'ProfileCenter')
    pc_uid = pc['id']
    agent.call_method(pc_uid, 'handleRestoreBackup')
    time.sleep(1.0)

    restore_comp = _get_comp(agent, 'RestoreDrawer')
    restore_uid = restore_comp['id']
    assert _ref(agent.get_component_state(restore_uid)['setupState']['visible']) is True
    assert _ref(agent.get_component_state(restore_uid)['setupState']['currentStep']) == 0

    form_state = _ref(agent.get_component_state(restore_uid)['setupState']['restoreForm'])
    assert form_state['storageType'] == 'webdav'
    assert form_state['encrypted'] is False

    agent.set_reactive_field(restore_uid, 'restoreForm', 'webdavAddress', MOCK_URL)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'webdavUsername', MOCK_USER)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'webdavPassword', MOCK_PASS)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'webdavPath', '/testbackup_enc')
    agent.set_reactive_field(restore_uid, 'restoreForm', 'encrypted', True)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'backupPassword', 'enc_pass_123')

    restore_dir = os.path.join(root_path, 'enc_restored')
    os.makedirs(restore_dir, exist_ok=True)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'localPath', 'enc_restored')

    # ===== 11. Start restore -> success, step moves to 1 =====
    agent.clear_messages()
    agent.call_method(restore_uid, 'handleNextStep')
    time.sleep(2.0)

    state = agent.get_component_state(restore_uid)
    current_step = _ref(state['setupState']['currentStep'])
    if current_step == 0:
        msgs = agent.get_messages()
        err_msgs = [m for m in msgs if m['type'] == 'error']
        if err_msgs:
            raise AssertionError(f'Restore step did not advance, error: {err_msgs}')
        raise AssertionError(f'Restore step did not advance, messages: {msgs}')

    # ===== 12. Wait for restore completion via progress polling =====
    task_id_val = _ref(agent.get_component_state(restore_uid)['setupState']['taskId'])
    assert task_id_val is not None and len(str(task_id_val)) > 0

    _wait_restore_done_via_api(task_id_val)
    time.sleep(2.0)

    # ===== 13. Verify restore success message =====
    msgs = agent.get_messages()
    success_msgs = [m for m in msgs if m['type'] == 'success' and m.get('key') == 'restore.restoreSuccess']
    assert len(success_msgs) > 0, f'Expected restore.restoreSuccess, got messages: {[m.get("key") for m in msgs]}'

    state = agent.get_component_state(restore_uid)
    assert _ref(state['setupState']['restoreCompleted']) is True

    # ===== 14. Verify restored files on disk =====
    restored_path = os.path.join(root_path, 'enc_restored')
    assert os.path.isfile(os.path.join(restored_path, 'secret.txt'))
    with open(os.path.join(restored_path, 'secret.txt'), 'r') as f:
        assert f.read() == 'Secret data\n'

    # ===== 15. Close restore drawer =====
    agent.call_method(restore_uid, 'handleClose')
    time.sleep(0.5)

    # ===== 16. Restore with wrong password -> error =====
    pc = _get_comp(agent, 'ProfileCenter')
    pc_uid = pc['id']
    agent.call_method(pc_uid, 'handleRestoreBackup')
    time.sleep(1.0)

    restore_comp = _get_comp(agent, 'RestoreDrawer')
    restore_uid = restore_comp['id']

    agent.set_reactive_field(restore_uid, 'restoreForm', 'webdavAddress', MOCK_URL)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'webdavUsername', MOCK_USER)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'webdavPassword', MOCK_PASS)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'webdavPath', '/testbackup_enc')
    agent.set_reactive_field(restore_uid, 'restoreForm', 'encrypted', True)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'backupPassword', 'wrong_password')

    restore_dir2 = os.path.join(root_path, 'wrong_pw_restore')
    os.makedirs(restore_dir2, exist_ok=True)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'localPath', 'wrong_pw_restore')

    agent.clear_messages()
    agent.call_method(restore_uid, 'handleNextStep')
    time.sleep(2.0)

    msgs = agent.get_messages()
    error_msgs = [m for m in msgs if m['type'] == 'error']
    assert len(error_msgs) > 0, f'Expected error message for wrong password, got: {[m.get("key") for m in msgs]}'

    state = agent.get_component_state(restore_uid)
    assert _ref(state['setupState']['currentStep']) == 0

    # ===== 17. Restore with bad connection -> error message =====
    agent.call_method(restore_uid, 'resetForm')
    time.sleep(0.3)

    agent.set_reactive_field(restore_uid, 'restoreForm', 'webdavAddress', 'http://127.0.0.1:19999')
    agent.set_reactive_field(restore_uid, 'restoreForm', 'webdavUsername', MOCK_USER)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'webdavPassword', MOCK_PASS)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'webdavPath', MOCK_PATH)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'encrypted', False)

    restore_dir3 = os.path.join(root_path, 'bad_conn_restore')
    os.makedirs(restore_dir3, exist_ok=True)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'localPath', 'bad_conn_restore')

    agent.clear_messages()
    agent.call_method(restore_uid, 'handleNextStep')
    time.sleep(3.0)

    msgs = agent.get_messages()
    error_msgs = [m for m in msgs if m['type'] == 'error']
    assert len(error_msgs) > 0, f'Expected error for bad connection, got: {[m.get("key") for m in msgs]}'
    restore_err_keys = [m.get('key') for m in error_msgs]
    assert any(k in ('restore.storageConnectionError', 'restore.startRestoreFailed') for k in restore_err_keys), f'Expected restore error key, got: {restore_err_keys}'

    # ===== 18. Restore with partial download failure =====
    agent.call_method(restore_uid, 'resetForm')
    time.sleep(0.3)

    mock_state = mock_server.state
    mock_state.reset()

    content_a = 'file A content\n'
    content_b = 'file B content\n'
    sha_a = _sha256(content_a)
    sha_b = _sha256(content_b)
    index_content = _make_index([
        ('file_a.txt', len(content_a), sha_a),
        ('file_b.txt', len(content_b), sha_b),
    ])
    mock_state.files['partial_test/.index'] = index_content
    mock_state.files['partial_test/file_a.txt'] = content_a.encode()
    mock_state.files['partial_test/file_b.txt'] = content_b.encode()
    mock_state.download_fail_paths = {'partial_test/file_b.txt'}

    agent.set_reactive_field(restore_uid, 'restoreForm', 'webdavAddress', MOCK_URL)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'webdavUsername', MOCK_USER)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'webdavPassword', MOCK_PASS)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'webdavPath', '/partial_test')
    agent.set_reactive_field(restore_uid, 'restoreForm', 'encrypted', False)

    partial_dir = os.path.join(root_path, 'partial_restore')
    os.makedirs(partial_dir, exist_ok=True)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'localPath', 'partial_restore')

    agent.clear_messages()
    agent.call_method(restore_uid, 'handleNextStep')
    time.sleep(2.0)

    state = agent.get_component_state(restore_uid)
    task_id_val = _ref(state['setupState']['taskId'])
    if task_id_val:
        _wait_restore_done_via_api(task_id_val)
        time.sleep(2.0)

        msgs = agent.get_messages()
        warn_msgs = [m for m in msgs if m['type'] == 'warning' and m.get('key') == 'restore.restorePartialSuccess']
        assert len(warn_msgs) > 0, f'Expected restore.restorePartialSuccess, got: {[m.get("key") for m in msgs]}'

        state = agent.get_component_state(restore_uid)
        display_items = _ref(state['setupState']['progress'])
        if display_items:
            failed_items = display_items.get('failed_items', [])
            if failed_items:
                failed_name = failed_items[0].get('name', '')

                # ===== 19. Retry failed file =====
                agent.clear_messages()
                agent.call_method(restore_uid, 'handleRetryFile', [failed_name])
                msg = _wait_message(agent, 'success')
                assert msg is not None
                assert msg['key'] == 'restore.retryStarted'

    # ===== 20. Cancel running restore =====
    agent.call_method(restore_uid, 'handleClose')
    time.sleep(0.5)

    pc = _get_comp(agent, 'ProfileCenter')
    pc_uid = pc['id']
    agent.call_method(pc_uid, 'handleRestoreBackup')
    time.sleep(1.0)

    restore_comp = _get_comp(agent, 'RestoreDrawer')
    restore_uid = restore_comp['id']

    mock_state.reset()
    mock_state.upload_delay = 10
    mock_state.files['cancel_test/.index'] = 'name\t1\tabc\n'
    mock_state.files['cancel_test/big_file.txt'] = b'x' * 1000

    agent.set_reactive_field(restore_uid, 'restoreForm', 'webdavAddress', MOCK_URL)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'webdavUsername', MOCK_USER)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'webdavPassword', MOCK_PASS)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'webdavPath', '/cancel_test')
    agent.set_reactive_field(restore_uid, 'restoreForm', 'encrypted', False)

    cancel_dir = os.path.join(root_path, 'cancel_restore')
    os.makedirs(cancel_dir, exist_ok=True)
    agent.set_reactive_field(restore_uid, 'restoreForm', 'localPath', 'cancel_restore')

    mock_state.download_fail_paths = set()

    agent.clear_messages()
    agent.call_method(restore_uid, 'handleNextStep')
    time.sleep(3.0)

    state = agent.get_component_state(restore_uid)
    is_running = _ref(state['setupState']['progress'])
    if is_running and is_running.get('is_running'):
        agent.clear_messages()
        agent.call_method(restore_uid, 'handleCancelRestore')
        time.sleep(2.0)

    # ===== 21. Delete backup rules =====
    agent.call_method(restore_uid, 'handleClose')
    time.sleep(0.5)

    pc = _get_comp(agent, 'ProfileCenter')
    pc_uid = pc['id']
    state = agent.get_component_state(pc_uid)
    backup_list = _ref(state['setupState']['backupList'])

    for rule in backup_list:
        session.post(f'{BASE_URL}/api/backup/delete', json={'id': rule['id']})

    agent.call_method(pc_uid, 'loadBackupList')
    time.sleep(0.5)
    state = agent.get_component_state(pc_uid)
    backup_list = _ref(state['setupState']['backupList'])
    assert len(backup_list) == 0


def main():
    import subprocess
    import shutil
    from common import print_error_log

    workdir = tempfile.mkdtemp(prefix='brookfile_brftest_')
    root_path = os.path.join(workdir, 'data')
    recycle_bin_path = os.path.join(workdir, 'recycle')
    os.makedirs(root_path)
    os.makedirs(recycle_bin_path)

    build_backend()

    global mock_server
    mock_server = MockWebDAVServer(port=MOCK_PORT)
    mock_server.start()
    print(f'Mock WebDAV started: {mock_server.url}')

    backend_proc = start_backend(workdir)
    session = req.Session()
    init_system(session, root_path, recycle_bin_path)
    login(session)

    frontend_proc = start_frontend()

    from playwright.sync_api import sync_playwright
    pw = sync_playwright().start()
    browser = pw.chromium.launch(headless=True)
    context = browser.new_context()
    page = context.new_page()
    agent = VueAgent(page)

    page.goto(FRONTEND_URL)

    page.wait_for_url('**/login', timeout=5000)
    agent.wait_ready()
    comps = agent.find_components_by_name('Login')
    assert len(comps) > 0, 'Login component not found'
    login_uid = comps[0]['id']
    agent.set_reactive_field(login_uid, 'form', 'username', DEFAULT_USERNAME)
    agent.set_reactive_field(login_uid, 'form', 'password', DEFAULT_PASSWORD)
    agent.call_method(login_uid, 'handleLogin')
    page.wait_for_url('**/files', timeout=5000)

    try:
        print(f'\n=== [test_backup_restore_frontend] Running test ===')
        test_backup_restore_frontend(page, agent, root_path, workdir)
        print(f'\n=== [test_backup_restore_frontend] All tests passed ===')
    except Exception:
        print(f'\n=== [test_backup_restore_frontend] Test failed ===')
        raise
    finally:
        print_error_log(workdir, 'test_backup_restore_frontend')
        browser.close()
        pw.stop()
        frontend_proc.terminate()
        try:
            frontend_proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            frontend_proc.kill()
            frontend_proc.wait()
        backend_proc.terminate()
        try:
            backend_proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            backend_proc.kill()
            backend_proc.wait()
        mock_server.stop()
        print(f'=== Cleaning up: {workdir} ===')
        shutil.rmtree(workdir, ignore_errors=True)


if __name__ == '__main__':
    import requests as req
    main()

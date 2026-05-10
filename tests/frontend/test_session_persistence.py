import time
import os
import sys
import subprocess
import tempfile
import shutil

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
from common import (
    build_backend, start_backend, start_frontend,
    FRONTEND_URL,
    DEFAULT_USERNAME, DEFAULT_PASSWORD, DEFAULT_SYSTEM_NAME,
    print_error_log,
)
from test_utils import VueAgent

from playwright.sync_api import sync_playwright


def test_session_persistence():
    test_name = 'test_session_persistence'
    workdir = tempfile.mkdtemp(prefix='brookfile_stest_')
    root_path = os.path.join(workdir, 'data')
    recycle_bin_path = os.path.join(workdir, 'recycle')
    os.makedirs(root_path)
    os.makedirs(recycle_bin_path)

    build_backend()

    backend_proc = None
    frontend_proc = None
    pw = None
    browser = None

    try:
        backend_proc = start_backend(workdir)
        frontend_proc = start_frontend()

        pw = sync_playwright().start()
        browser = pw.chromium.launch(headless=True)
        context = browser.new_context()
        page = context.new_page()
        agent = VueAgent(page)

        page.goto(FRONTEND_URL)

        page.wait_for_url('**/init', timeout=5000)
        agent.wait_ready()
        comps = agent.find_components_by_name('Init')
        assert len(comps) > 0
        uid = comps[0]['id']
        agent.set_reactive_field(uid, 'form', 'systemName', DEFAULT_SYSTEM_NAME)
        agent.set_reactive_field(uid, 'form', 'username', DEFAULT_USERNAME)
        agent.set_reactive_field(uid, 'form', 'password', DEFAULT_PASSWORD)
        agent.set_reactive_field(uid, 'form', 'confirmPassword', DEFAULT_PASSWORD)
        agent.set_reactive_field(uid, 'form', 'rootPath', root_path)
        agent.set_reactive_field(uid, 'form', 'recycleBinPath', recycle_bin_path)
        agent.call_method(uid, 'handleInit')
        page.wait_for_url('**/login', timeout=5000)

        comps = agent.find_components_by_name('Login')
        assert len(comps) > 0
        uid = comps[0]['id']
        agent.set_reactive_field(uid, 'form', 'username', DEFAULT_USERNAME)
        agent.set_reactive_field(uid, 'form', 'password', DEFAULT_PASSWORD)
        agent.call_method(uid, 'handleLogin')
        page.wait_for_url('**/files', timeout=5000)
        agent.wait_ready(timeout=10000)

        user_store = agent.get_store('user')
        assert user_store['loggedIn'] is True
        assert user_store['user']['username'] == DEFAULT_USERNAME

        time.sleep(1)

        backend_proc.terminate()
        try:
            backend_proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            backend_proc.kill()
            backend_proc.wait()

        backend_proc = start_backend(workdir)
        time.sleep(2)

        page.reload(wait_until='networkidle')
        time.sleep(2)

        user_store = agent.get_store('user')
        assert user_store['loggedIn'] is True, (
            f'Session should persist after backend restart, got loggedIn={user_store["loggedIn"]}'
        )
        assert user_store['user'] is not None
        assert user_store['user']['username'] == DEFAULT_USERNAME

        print(f'\n=== [{test_name}] 所有测试通过 ===')

    except Exception:
        print(f'\n=== [{test_name}] 测试失败 ===')
        raise
    finally:
        print_error_log(workdir, test_name)
        if browser:
            browser.close()
        if pw:
            pw.stop()
        if frontend_proc:
            frontend_proc.terminate()
            try:
                frontend_proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                frontend_proc.kill()
                frontend_proc.wait()
        if backend_proc:
            backend_proc.terminate()
            try:
                backend_proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                backend_proc.kill()
                backend_proc.wait()
        print(f'=== [{test_name}] 清理临时目录: {workdir} ===')
        shutil.rmtree(workdir, ignore_errors=True)
        print('清理完成')


if __name__ == '__main__':
    test_session_persistence()

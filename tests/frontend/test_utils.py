import subprocess
import tempfile
import os
import sys
import shutil
from playwright.sync_api import sync_playwright, Page

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
from common import (
    build_backend, start_backend, start_frontend,
    init_system, login,
    print_error_log,
    BASE_URL, FRONTEND_URL,
    DEFAULT_USERNAME, DEFAULT_PASSWORD, DEFAULT_SYSTEM_NAME,
)


def _ref(state_val):
    if isinstance(state_val, dict) and '__type' in state_val:
        return state_val['value']
    return state_val


class VueAgent:
    def __init__(self, page: Page):
        self._page = page

    def _eval(self, expr: str):
        return self._page.evaluate(f'() => {{ return {expr}; }}')

    def wait_ready(self, timeout=15000):
        self._page.wait_for_function(
            '() => window.__vue_agent__ && window.__vue_agent__.isReady()',
            timeout=timeout
        )

    def find_components_by_name(self, name: str) -> list:
        return self._eval(f'window.__vue_agent__.findComponentsByName("{name}")')

    def find_component_by_route(self, route_name: str) -> dict | None:
        return self._eval(f'window.__vue_agent__.findComponentByRoute("{route_name}")')

    def get_component_state(self, uid: int) -> dict | None:
        return self._eval(f'window.__vue_agent__.getComponentState({uid})')

    def call_method(self, uid: int, method_name: str, args=None):
        return self._page.evaluate(
            '([uid, methodName, args]) => window.__vue_agent__.callMethod(uid, methodName, args)',
            [uid, method_name, args or []]
        )

    def set_ref(self, uid: int, key: str, value):
        return self._page.evaluate(
            '([uid, key, value]) => {'
            '  const comps = window.__vue_agent__.listAllComponents();'
            '  return window.__vue_agent__.setRef(uid, key, value);'
            '}',
            [uid, key, value]
        )

    def set_reactive_field(self, uid: int, key: str, field: str, value):
        return self._page.evaluate(
            '([uid, key, field, value]) => window.__vue_agent__.setReactiveField(uid, key, field, value)',
            [uid, key, field, value]
        )

    def get_store(self, store_name: str) -> dict | None:
        return self._eval(f'window.__vue_agent__.getStore("{store_name}")')

    def list_stores(self) -> list:
        return self._eval('window.__vue_agent__.listStores()')

    def dispatch_store(self, store_name: str, action_name: str, args=None):
        return self._page.evaluate(
            '([storeName, actionName, args]) => window.__vue_agent__.dispatchStore(storeName, actionName, args)',
            [store_name, action_name, args or []]
        )

    def list_all_components(self) -> list:
        return self._eval('window.__vue_agent__.listAllComponents()')

    def get_component_by_name(self, name: str) -> dict | None:
        comps = self.find_components_by_name(name)
        return comps[0] if comps else None

    def get_messages(self) -> list:
        return self._eval('window.__test_messages || []')

    def clear_messages(self):
        self._eval('window.__test_clear_messages ? window.__test_clear_messages() : void 0')

    def wait_for_message(self, msg_type: str, timeout=5000) -> dict:
        self._page.wait_for_function(
            f'() => (window.__test_messages || []).some(m => m.type === "{msg_type}")',
            timeout=timeout
        )
        msgs = [m for m in self.get_messages() if m['type'] == msg_type]
        return msgs[-1] if msgs else None


def _do_init(page, agent, root_path, recycle_bin_path):
    print('=== 初始化系统 ===')
    page.wait_for_url('**/init', timeout=5000)
    agent.wait_ready()

    comps = agent.find_components_by_name('Init')
    assert len(comps) > 0, 'Init component not found during auto-init'
    uid = comps[0]['id']

    agent.set_reactive_field(uid, 'form', 'systemName', DEFAULT_SYSTEM_NAME)
    agent.set_reactive_field(uid, 'form', 'username', DEFAULT_USERNAME)
    agent.set_reactive_field(uid, 'form', 'password', DEFAULT_PASSWORD)
    agent.set_reactive_field(uid, 'form', 'confirmPassword', DEFAULT_PASSWORD)
    agent.set_reactive_field(uid, 'form', 'rootPath', root_path)
    if recycle_bin_path:
        agent.set_reactive_field(uid, 'form', 'recycleBinPath', recycle_bin_path)

    agent.call_method(uid, 'handleInit')
    page.wait_for_url('**/login', timeout=5000)
    print('系统初始化成功')


def _do_login(page, agent):
    print('=== 登录 ===')
    page.wait_for_url('**/login', timeout=5000)
    agent.wait_ready()

    comps = agent.find_components_by_name('Login')
    assert len(comps) > 0, 'Login component not found during auto-login'
    uid = comps[0]['id']

    agent.set_reactive_field(uid, 'form', 'username', DEFAULT_USERNAME)
    agent.set_reactive_field(uid, 'form', 'password', DEFAULT_PASSWORD)

    agent.call_method(uid, 'handleLogin')
    page.wait_for_url('**/files', timeout=5000)
    print('登录成功')


def run_frontend_test(test_func, init=True, viewport=None):
    test_name = test_func.__name__
    workdir = tempfile.mkdtemp(prefix='brookfile_ftest_')
    root_path = os.path.join(workdir, 'data')
    recycle_bin_path = os.path.join(workdir, 'recycle')

    build_backend()

    backend_proc = None
    frontend_proc = None
    pw = None
    browser = None

    try:
        backend_proc = start_backend(workdir)

        if init:
            os.makedirs(root_path)
            os.makedirs(recycle_bin_path)

        frontend_proc = start_frontend()

        pw = sync_playwright().start()
        browser = pw.chromium.launch(headless=True)
        ctx_opts = {}
        if viewport:
            ctx_opts['viewport'] = viewport
            ctx_opts['is_mobile'] = True
            ctx_opts['has_touch'] = True
        context = browser.new_context(**ctx_opts)
        page = context.new_page()

        agent = VueAgent(page)

        if init:
            page.goto(FRONTEND_URL)
            _do_init(page, agent, root_path, recycle_bin_path)
            _do_login(page, agent)

        print(f'\n=== [{test_name}] 运行测试 ===')
        test_func(page, agent, root_path, workdir)
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

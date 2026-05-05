import subprocess
import time
import os
import sys
import shutil
import requests

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_DIR = os.path.join(SCRIPT_DIR, '..')
BACKEND_DIR = os.path.join(PROJECT_DIR, 'backend')
FRONTEND_DIR = os.path.join(PROJECT_DIR, 'frontend')
BACKEND_EXE = os.path.join(BACKEND_DIR, 'target', 'debug', 'backend.exe')
BASE_URL = 'http://127.0.0.1:3000'
FRONTEND_URL = 'http://127.0.0.1:8080'

DEFAULT_USERNAME = 'admin'
DEFAULT_PASSWORD = 'password123'
DEFAULT_SYSTEM_NAME = 'TestSystem'


def build_backend():
    print('=== 编译后端 ===')
    result = subprocess.run('cargo build', shell=True, cwd=BACKEND_DIR, capture_output=True, text=True)
    if result.returncode != 0:
        print(f'编译失败:\n{result.stderr}')
        sys.exit(1)
    print('编译成功')


def start_backend(workdir):
    print(f'=== 启动后端 (workdir: {workdir}) ===')
    process = subprocess.Popen(
        [BACKEND_EXE],
        cwd=workdir,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL
    )
    print(f'后端已启动 (PID: {process.pid})，等待就绪...')
    for i in range(30):
        try:
            requests.post(f'{BASE_URL}/api/system/info', timeout=1)
            break
        except requests.ConnectionError:
            time.sleep(0.5)
    else:
        print('后端启动超时')
        sys.exit(1)
    print('后端就绪')
    return process


def start_frontend():
    print('=== 启动前端 ===')
    npm_cmd = 'npm.cmd' if os.name == 'nt' else 'npm'
    process = subprocess.Popen(
        [npm_cmd, 'run', 'dev'],
        cwd=FRONTEND_DIR,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    print(f'前端已启动 (PID: {process.pid})，等待就绪...')
    for i in range(60):
        try:
            resp = requests.get(FRONTEND_URL, timeout=1)
            if resp.status_code == 200:
                break
        except requests.ConnectionError:
            time.sleep(0.5)
    else:
        print('前端启动超时')
        process.terminate()
        sys.exit(1)
    print('前端就绪')
    return process


def init_system(session, root_path, recycle_bin_path):
    print('=== 初始化系统 ===')
    resp = session.post(f'{BASE_URL}/api/system/init', json={
        'username': DEFAULT_USERNAME,
        'password': DEFAULT_PASSWORD,
        'system_name': DEFAULT_SYSTEM_NAME,
        'root_path': root_path,
        'recycle_bin_path': recycle_bin_path
    })
    data = resp.json()
    if not data.get('success'):
        print(f'系统初始化失败: {data}')
        sys.exit(1)
    print('系统初始化成功')


def login(session):
    print('=== 登录 ===')
    resp = session.post(f'{BASE_URL}/api/auth/login', json={
        'username': DEFAULT_USERNAME,
        'password': DEFAULT_PASSWORD
    })
    data = resp.json()
    if not data.get('success'):
        print(f'登录失败: {data}')
        sys.exit(1)
    print('登录成功')


def print_error_log(workdir, test_name):
    error_log = os.path.join(workdir, 'error.log')
    print(f'\n=== [{test_name}] Error Log ===')
    if os.path.exists(error_log):
        with open(error_log, 'r', encoding='utf-8', errors='replace') as f:
            content = f.read()
        if content.strip():
            print(content)
        else:
            print('(empty)')
    else:
        print('(no error.log)')


def stop_backend(process, workdir, test_name):
    print(f'\n=== [{test_name}] 停止后端 ===')
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()
    print('后端已停止')

    print(f'=== [{test_name}] 清理临时目录: {workdir} ===')
    shutil.rmtree(workdir, ignore_errors=True)
    print('清理完成')

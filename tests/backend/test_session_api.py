import os
import sqlite3
import subprocess
import tempfile
import time
import shutil
import requests
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
from common import (
    build_backend, start_backend, init_system, login,
    print_error_log, BASE_URL, DEFAULT_USERNAME, DEFAULT_PASSWORD,
    BACKEND_EXE,
)


def test_session_api(session, root_path):
    workdir = os.path.dirname(root_path)
    backend_exe = BACKEND_EXE
    db_path = os.path.join(workdir, 'database.db')

    # ================================================================
    # 测试 1：设置接口验证
    # ================================================================
    print('\n--- 测试 1：设置接口验证 ---')

    resp = session.post(f'{BASE_URL}/api/system/get_settings')
    data = resp.json()
    assert data['success'] is True
    assert 'session_timeout_days' in data
    assert 'max_login_devices' in data
    original_timeout_days = data['session_timeout_days']
    original_max_devices = data['max_login_devices']
    original_system_name = data['system_name']
    original_fulltext = data.get('notebook_fulltext_search', True)

    resp = session.post(f'{BASE_URL}/api/system/update_settings', json={
        'system_name': original_system_name,
        'session_timeout_days': 0,
        'max_login_devices': 3,
        'notebook_fulltext_search': original_fulltext,
    })
    assert resp.json()['success'] is False
    assert resp.json()['fail_code'] == 'INVALID_PARAM'

    resp = session.post(f'{BASE_URL}/api/system/update_settings', json={
        'system_name': original_system_name,
        'session_timeout_days': 3,
        'max_login_devices': 0,
        'notebook_fulltext_search': original_fulltext,
    })
    assert resp.json()['success'] is False
    assert resp.json()['fail_code'] == 'INVALID_PARAM'

    resp = session.post(f'{BASE_URL}/api/system/update_settings', json={
        'system_name': original_system_name,
        'session_timeout_days': 5,
        'max_login_devices': 2,
        'notebook_fulltext_search': original_fulltext,
    })
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/system/get_settings')
    data2 = resp.json()
    assert data2['session_timeout_days'] == 5
    assert data2['max_login_devices'] == 2

    session.post(f'{BASE_URL}/api/system/update_settings', json={
        'system_name': original_system_name,
        'session_timeout_days': original_timeout_days,
        'max_login_devices': original_max_devices,
        'notebook_fulltext_search': original_fulltext,
    })

    # ================================================================
    # 测试 2：Max Login Devices 限制
    # ================================================================
    print('\n--- 测试 2：Max Login Devices 限制 ---')

    user_root = os.path.join(workdir, 'user1_data')
    user_recycle = os.path.join(workdir, 'user1_recycle')
    os.makedirs(user_root, exist_ok=True)
    os.makedirs(user_recycle, exist_ok=True)
    resp = session.post(f'{BASE_URL}/api/user/create', json={
        'username': 'session_user',
        'password': 'pass12345',
        'root_path': user_root,
        'recycle_bin_path': user_recycle,
        'is_admin': False,
    })
    assert resp.json()['success'] is True

    session.post(f'{BASE_URL}/api/system/update_settings', json={
        'system_name': original_system_name,
        'session_timeout_days': original_timeout_days,
        'max_login_devices': 2,
        'notebook_fulltext_search': original_fulltext,
    })

    sess_b = requests.Session()
    sess_c = requests.Session()
    sess_d = requests.Session()

    resp = sess_b.post(f'{BASE_URL}/api/auth/login', json={'username': 'session_user', 'password': 'pass12345'})
    assert resp.json()['success'] is True
    resp = sess_c.post(f'{BASE_URL}/api/auth/login', json={'username': 'session_user', 'password': 'pass12345'})
    assert resp.json()['success'] is True
    resp = sess_d.post(f'{BASE_URL}/api/auth/login', json={'username': 'session_user', 'password': 'pass12345'})
    assert resp.json()['success'] is True

    resp = sess_b.post(f'{BASE_URL}/api/system/info')
    assert resp.json()['logged_in'] is False, 'sess_b 应该被踢掉'

    resp = sess_c.post(f'{BASE_URL}/api/system/info')
    assert resp.json()['logged_in'] is True, 'sess_c 应该仍然有效'

    resp = sess_d.post(f'{BASE_URL}/api/system/info')
    assert resp.json()['logged_in'] is True, 'sess_d 应该仍然有效'

    session.post(f'{BASE_URL}/api/system/update_settings', json={
        'system_name': original_system_name,
        'session_timeout_days': original_timeout_days,
        'max_login_devices': original_max_devices,
        'notebook_fulltext_search': original_fulltext,
    })

    # ================================================================
    # 测试 3：Session 持久化（服务器重启后 session 仍有效）
    # ================================================================
    print('\n--- 测试 3：Session 持久化 ---')

    session.post(f'{BASE_URL}/api/auth/logout')
    resp = session.post(f'{BASE_URL}/api/auth/login', json={'username': 'admin', 'password': DEFAULT_PASSWORD})
    assert resp.json()['success'] is True
    resp = session.post(f'{BASE_URL}/api/system/info')
    assert resp.json()['logged_in'] is True

    saved_cookies = dict(session.cookies)

    time.sleep(1)

    new_proc = subprocess.Popen([backend_exe], cwd=workdir, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for i in range(30):
        try:
            requests.post(f'{BASE_URL}/api/system/info', timeout=1)
            break
        except requests.ConnectionError:
            time.sleep(0.5)
    else:
        new_proc.terminate()
        raise AssertionError('后端重启超时')
    print('后端重启完成')

    new_session = requests.Session()
    for k, v in saved_cookies.items():
        new_session.cookies.set(k, v)
    resp = new_session.post(f'{BASE_URL}/api/system/info')
    info = resp.json()
    assert info['logged_in'] is True, f'Session 应该在重启后仍然有效, got {info}'
    new_session.post(f'{BASE_URL}/api/auth/logout')

    new_proc.terminate()
    try:
        new_proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        new_proc.kill()
        new_proc.wait()

    # ================================================================
    # 测试 4：Session Timeout 过期（改 DB + 重启）
    # ================================================================
    print('\n--- 测试 4：Session Timeout 过期 ---')

    new_proc = subprocess.Popen([backend_exe], cwd=workdir, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for i in range(30):
        try:
            requests.post(f'{BASE_URL}/api/system/info', timeout=1)
            break
        except requests.ConnectionError:
            time.sleep(0.5)
    else:
        new_proc.terminate()
        raise AssertionError('后端启动超时')

    login_sess = requests.Session()
    resp = login_sess.post(f'{BASE_URL}/api/auth/login', json={'username': 'admin', 'password': DEFAULT_PASSWORD})
    assert resp.json()['success'] is True
    resp = login_sess.post(f'{BASE_URL}/api/system/info')
    assert resp.json()['logged_in'] is True

    expired_cookies = dict(login_sess.cookies)
    login_sess.post(f'{BASE_URL}/api/auth/logout')

    new_proc.terminate()
    try:
        new_proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        new_proc.kill()
        new_proc.wait()

    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    eight_days_ago = int(time.time()) - 8 * 86400
    cursor.execute("UPDATE sessions SET last_access_time = ?", (eight_days_ago,))
    conn.commit()
    conn.close()

    new_proc = subprocess.Popen([backend_exe], cwd=workdir, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for i in range(30):
        try:
            requests.post(f'{BASE_URL}/api/system/info', timeout=1)
            break
        except requests.ConnectionError:
            time.sleep(0.5)
    else:
        new_proc.terminate()
        raise AssertionError('后端启动超时')

    expired_sess = requests.Session()
    for k, v in expired_cookies.items():
        expired_sess.cookies.set(k, v)
    resp = expired_sess.post(f'{BASE_URL}/api/system/info')
    info = resp.json()
    assert info['logged_in'] is False, f'过期 Session 应该无效, got {info}'

    new_proc.terminate()
    try:
        new_proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        new_proc.kill()
        new_proc.wait()

    # ================================================================
    # 测试 5：删除用户使其所有 session 失效
    # ================================================================
    print('\n--- 测试 5：删除用户使其所有 session 失效 ---')

    new_proc = subprocess.Popen([backend_exe], cwd=workdir, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for i in range(30):
        try:
            requests.post(f'{BASE_URL}/api/system/info', timeout=1)
            break
        except requests.ConnectionError:
            time.sleep(0.5)
    else:
        new_proc.terminate()
        raise AssertionError('后端启动超时')

    admin_sess = requests.Session()
    admin_sess.post(f'{BASE_URL}/api/auth/login', json={'username': 'admin', 'password': DEFAULT_PASSWORD})

    user2_root = os.path.join(workdir, 'user2_data')
    user2_recycle = os.path.join(workdir, 'user2_recycle')
    os.makedirs(user2_root, exist_ok=True)
    os.makedirs(user2_recycle, exist_ok=True)
    resp = admin_sess.post(f'{BASE_URL}/api/user/create', json={
        'username': 'del_user',
        'password': 'pass12345',
        'root_path': user2_root,
        'recycle_bin_path': user2_recycle,
        'is_admin': False,
    })
    assert resp.json()['success'] is True

    user_sess = requests.Session()
    resp = user_sess.post(f'{BASE_URL}/api/auth/login', json={'username': 'del_user', 'password': 'pass12345'})
    assert resp.json()['success'] is True
    resp = user_sess.post(f'{BASE_URL}/api/system/info')
    assert resp.json()['logged_in'] is True

    users = admin_sess.post(f'{BASE_URL}/api/user/list').json()
    del_user_id = next(u['id'] for u in users if u['username'] == 'del_user')
    resp = admin_sess.post(f'{BASE_URL}/api/user/delete', json={'id': del_user_id})
    assert resp.json()['success'] is True

    resp = user_sess.post(f'{BASE_URL}/api/system/info')
    assert resp.json()['logged_in'] is False, '删除用户后 session 应该失效'

    new_proc.terminate()
    try:
        new_proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        new_proc.kill()
        new_proc.wait()

    # ================================================================
    # 测试 6：修改密码使其所有 session 失效
    # ================================================================
    print('\n--- 测试 6：修改密码使其所有 session 失效 ---')

    new_proc = subprocess.Popen([backend_exe], cwd=workdir, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for i in range(30):
        try:
            requests.post(f'{BASE_URL}/api/system/info', timeout=1)
            break
        except requests.ConnectionError:
            time.sleep(0.5)
    else:
        new_proc.terminate()
        raise AssertionError('后端启动超时')

    admin_sess = requests.Session()
    admin_sess.post(f'{BASE_URL}/api/auth/login', json={'username': 'admin', 'password': DEFAULT_PASSWORD})

    user3_root = os.path.join(workdir, 'user3_data')
    user3_recycle = os.path.join(workdir, 'user3_recycle')
    os.makedirs(user3_root, exist_ok=True)
    os.makedirs(user3_recycle, exist_ok=True)
    admin_sess.post(f'{BASE_URL}/api/user/create', json={
        'username': 'pw_user',
        'password': 'oldpass123',
        'root_path': user3_root,
        'recycle_bin_path': user3_recycle,
        'is_admin': False,
    })

    pw_sess = requests.Session()
    resp = pw_sess.post(f'{BASE_URL}/api/auth/login', json={'username': 'pw_user', 'password': 'oldpass123'})
    assert resp.json()['success'] is True
    resp = pw_sess.post(f'{BASE_URL}/api/system/info')
    assert resp.json()['logged_in'] is True

    resp = pw_sess.post(f'{BASE_URL}/api/user/change_password', json={
        'old_password': 'oldpass123',
        'new_password': 'newpass456',
    })
    assert resp.json()['success'] is True

    resp = pw_sess.post(f'{BASE_URL}/api/system/info')
    assert resp.json()['logged_in'] is False, '修改密码后 session 应该失效'

    new_proc.terminate()
    try:
        new_proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        new_proc.kill()
        new_proc.wait()

    print_error_log(workdir, 'test_session_api')
    shutil.rmtree(workdir, ignore_errors=True)


if __name__ == '__main__':
    test_name = test_session_api.__name__
    workdir = tempfile.mkdtemp(prefix='brookfile_stest_')
    root_path = os.path.join(workdir, 'data')
    recycle_bin_path = os.path.join(workdir, 'recycle')

    build_backend()

    try:
        os.makedirs(root_path, exist_ok=True)
        os.makedirs(recycle_bin_path, exist_ok=True)
        process = start_backend(workdir)
        s = requests.Session()
        init_system(s, root_path, recycle_bin_path)
        login(s)

        print(f'\n=== [{test_name}] 运行测试 ===')
        test_session_api(s, root_path)
        print(f'\n=== [{test_name}] 所有测试通过 ===')
    except Exception:
        print(f'\n=== [{test_name}] 测试失败 ===')
        print_error_log(workdir, test_name)
        raise

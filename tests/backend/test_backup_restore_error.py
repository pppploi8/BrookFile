import subprocess
import tempfile
import time
import os
import sys
import shutil
import requests
import hashlib

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
from common import build_backend, start_backend, init_system, login, print_error_log, BASE_URL

MOCK_ALIST_PORT = 15244
MOCK_ALIST_URL = f'http://127.0.0.1:{MOCK_ALIST_PORT}'
MOCK_USERNAME = 'admin'
MOCK_PASSWORD = 'admin123'
MOCK_STORAGE_PATH = '/testbackup'


def _wait_backup_done(session, rule_id, timeout=120):
    deadline = time.time() + timeout
    while time.time() < deadline:
        resp = session.post(f'{BASE_URL}/api/backup/progress', json={'rule_id': rule_id})
        data = resp.json()
        if not data.get('is_running', False):
            return data
        time.sleep(2)
    raise AssertionError(f'备份超时 ({timeout}s)')


def _wait_restore_done(session, task_id, timeout=120):
    deadline = time.time() + timeout
    last_progress = None
    while time.time() < deadline:
        resp = session.post(f'{BASE_URL}/api/restore/progress', json={'task_id': task_id})
        data = resp.json()
        last_progress = data
        if not data.get('is_running', False):
            return data
        time.sleep(2)
    raise AssertionError(f'恢复超时 ({timeout}s), last progress: {last_progress}')


def _storage_config():
    return {
        'address': MOCK_ALIST_URL,
        'username': MOCK_USERNAME,
        'password': MOCK_PASSWORD,
        'path': MOCK_STORAGE_PATH,
    }


def _make_index(entries):
    import json
    lines = []
    for e in entries:
        obj = {'path': e[0], 'size': e[1], 'sha256': e[2]}
        if len(e) >= 4 and e[3]:
            obj['file_id'] = e[3]
        lines.append(json.dumps(obj, ensure_ascii=False))
    return '\n'.join(lines) + '\n'


def _sha256(content):
    if isinstance(content, str):
        content = content.encode()
    return hashlib.sha256(content).hexdigest()


def test_backup_restore_error():
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    from mock_webdav_server import MockWebDAVServer

    workdir = tempfile.mkdtemp(prefix='brookfile_errtest_')
    root_path = os.path.join(workdir, 'data')
    recycle_bin_path = os.path.join(workdir, 'recycle')
    os.makedirs(root_path)
    os.makedirs(recycle_bin_path)

    build_backend()

    mock = MockWebDAVServer(port=MOCK_ALIST_PORT)
    mock.start()
    print(f'Mock WebDAV 已启动: {mock.url}')

    backend_proc = start_backend(workdir)

    session = requests.Session()
    init_system(session, root_path, recycle_bin_path)
    login(session)

    passed = 0

    try:
        # ===== Test 1: Login failure during backup =====
        print('\n--- Test 1: WebDAV auth failure during backup ---')
        mock.state.reset()
        mock.state.auth_fail = True

        test_dir = os.path.join(root_path, 'login_fail_test')
        os.makedirs(test_dir, exist_ok=True)
        with open(os.path.join(test_dir, 'file.txt'), 'w') as f:
            f.write('test content\n')

        resp = session.post(f'{BASE_URL}/api/backup/create', json={
            'name': 'Login Fail Backup',
            'storage_type': 'webdav',
            'storage_config': _storage_config(),
            'local_path': 'login_fail_test',
            'encrypted': False,
            'cycle': 'daily',
            'backup_time': {'time': '08:00'},
        })
        assert resp.json()['success'] is True
        rules = session.post(f'{BASE_URL}/api/backup/list').json()
        rule_id = rules[0]['id']

        resp = session.post(f'{BASE_URL}/api/backup/start', json={'rule_id': rule_id, 'mode': 'full'})
        assert resp.json().get('task_id') is not None

        progress = _wait_backup_done(session, rule_id)
        resp = session.post(f'{BASE_URL}/api/backup/logs', json={'rule_id': rule_id, 'page': 1, 'page_size': 10})
        logs = resp.json()
        log = logs['items'][0]
        print(f'  status={log["status"]}, success={log["backup_success_count"]}, fail={log["backup_fail_count"]}')
        assert log['status'] in ('failed', 'partial'), f'Expected failed/partial, got {log["status"]}'
        assert log['backup_success_count'] == 0
        print('PASSED')
        passed += 1

        session.post(f'{BASE_URL}/api/backup/delete', json={'id': rule_id})

        # ===== Test 2: Upload failure during backup =====
        print('\n--- Test 2: Upload failure during backup ---')
        mock.state.reset()
        mock.state.upload_fail = True

        test_dir = os.path.join(root_path, 'upload_fail_test')
        os.makedirs(test_dir, exist_ok=True)
        with open(os.path.join(test_dir, 'file1.txt'), 'w') as f:
            f.write('content 1\n')
        with open(os.path.join(test_dir, 'file2.txt'), 'w') as f:
            f.write('content 2\n')

        resp = session.post(f'{BASE_URL}/api/backup/create', json={
            'name': 'Upload Fail Backup',
            'storage_type': 'webdav',
            'storage_config': _storage_config(),
            'local_path': 'upload_fail_test',
            'encrypted': False,
            'cycle': 'daily',
            'backup_time': {'time': '08:00'},
        })
        rules = session.post(f'{BASE_URL}/api/backup/list').json()
        rule_id = rules[0]['id']

        resp = session.post(f'{BASE_URL}/api/backup/start', json={'rule_id': rule_id, 'mode': 'full'})
        progress = _wait_backup_done(session, rule_id, timeout=180)

        resp = session.post(f'{BASE_URL}/api/backup/logs', json={'rule_id': rule_id, 'page': 1, 'page_size': 10})
        log = resp.json()['items'][0]
        print(f'  status={log["status"]}, success={log["backup_success_count"]}, fail={log["backup_fail_count"]}')
        assert log['backup_fail_count'] > 0 or log['status'] == 'failed'
        print('PASSED')
        passed += 1

        session.post(f'{BASE_URL}/api/backup/delete', json={'id': rule_id})

        # ===== Test 3: Restore with partial download failure =====
        print('\n--- Test 3: Restore - partial download failure ---')
        mock.state.reset()

        content_a = 'file A content\n'
        content_b = 'file B content\n'
        sha_a = _sha256(content_a)
        sha_b = _sha256(content_b)

        index_content = _make_index([
            ('file_a.txt', len(content_a), sha_a),
            ('file_b.txt', len(content_b), sha_b),
        ])
        mock.seed_file('testbackup/.index', index_content)
        mock.seed_file('testbackup/file_a.txt', content_a)
        mock.seed_file('testbackup/file_b.txt', content_b)

        mock.state.download_fail_paths = {'testbackup/file_b.txt'}

        restore_dir = os.path.join(root_path, 'partial_restore')
        os.makedirs(restore_dir, exist_ok=True)

        resp = session.post(f'{BASE_URL}/api/restore/start', json={
            'storage_type': 'webdav',
            'storage_config': _storage_config(),
            'encrypted': False,
            'local_path': 'partial_restore',
        })
        data = resp.json()
        assert data['success'] is True
        task_id = data['task_id']

        progress = _wait_restore_done(session, task_id)
        success_count = progress.get('success_count', 0)
        failed_items = progress.get('failed_items', [])
        failed_names = [f['name'] for f in failed_items]
        print(f'  success={success_count}, failed={len(failed_items)}')

        assert os.path.isfile(os.path.join(restore_dir, 'file_a.txt'))
        with open(os.path.join(restore_dir, 'file_a.txt'), 'r') as f:
            assert f.read() == content_a
        assert len(failed_items) >= 1
        assert 'file_b.txt' in failed_names
        print('PASSED')
        passed += 1

        # ===== Test 4: Restore - index not found =====
        print('\n--- Test 4: Restore - index not found ---')
        mock.state.reset()

        resp = session.post(f'{BASE_URL}/api/restore/start', json={
            'storage_type': 'webdav',
            'storage_config': _storage_config(),
            'encrypted': False,
            'local_path': 'partial_restore',
        })
        data = resp.json()
        print(f'  Response: success={data.get("success")}, fail_code={data.get("fail_code")}')
        assert data['success'] is False
        print('PASSED')
        passed += 1

        # ===== Test 5: Restore - wrong credentials =====
        print('\n--- Test 5: Restore - wrong WebDAV credentials ---')
        mock.state.reset()
        mock.seed_file('testbackup/.index', 'dummy\n')

        bad_config = {
            'address': MOCK_ALIST_URL,
            'username': 'wrong_user',
            'password': 'wrong_pass',
            'path': MOCK_STORAGE_PATH,
        }
        resp = session.post(f'{BASE_URL}/api/restore/start', json={
            'storage_type': 'webdav',
            'storage_config': bad_config,
            'encrypted': False,
            'local_path': 'partial_restore',
        })
        data = resp.json()
        print(f'  Response: success={data.get("success")}, fail_code={data.get("fail_code")}')
        assert data['success'] is False
        print('PASSED')
        passed += 1

        # ===== Test 6: Backup - connection refused =====
        print('\n--- Test 6: Backup - WebDAV connection refused ---')
        mock.state.reset()

        test_dir = os.path.join(root_path, 'conn_refused_test')
        os.makedirs(test_dir, exist_ok=True)
        with open(os.path.join(test_dir, 'file.txt'), 'w') as f:
            f.write('test\n')

        bad_url_config = {
            'address': 'http://127.0.0.1:19999',
            'username': MOCK_USERNAME,
            'password': MOCK_PASSWORD,
            'path': MOCK_STORAGE_PATH,
        }
        resp = session.post(f'{BASE_URL}/api/backup/create', json={
            'name': 'Conn Refused Backup',
            'storage_type': 'webdav',
            'storage_config': bad_url_config,
            'local_path': 'conn_refused_test',
            'encrypted': False,
            'cycle': 'daily',
            'backup_time': {'time': '08:00'},
        })
        rules = session.post(f'{BASE_URL}/api/backup/list').json()
        rule_id = rules[0]['id']

        resp = session.post(f'{BASE_URL}/api/backup/start', json={'rule_id': rule_id, 'mode': 'full'})
        progress = _wait_backup_done(session, rule_id, timeout=180)

        resp = session.post(f'{BASE_URL}/api/backup/logs', json={'rule_id': rule_id, 'page': 1, 'page_size': 10})
        log = resp.json()['items'][0]
        print(f'  status={log["status"]}, fail_reason={log.get("fail_reason", "")}')
        assert log['status'] in ('failed', 'partial')
        print('PASSED')
        passed += 1

        session.post(f'{BASE_URL}/api/backup/delete', json={'id': rule_id})

        # ===== Test 7: Restore - all downloads fail =====
        print('\n--- Test 7: Restore - all downloads fail ---')
        mock.state.reset()

        content = 'all fail content\n'
        sha = _sha256(content)
        index_content = _make_index([
            ('fail1.txt', len(content), sha),
            ('fail2.txt', len(content), sha),
        ])
        mock.seed_file('testbackup/.index', index_content)
        mock.state.download_fail_paths = {'testbackup/fail1.txt', 'testbackup/fail2.txt'}

        restore_dir = os.path.join(root_path, 'all_fail_restore')
        os.makedirs(restore_dir, exist_ok=True)

        resp = session.post(f'{BASE_URL}/api/restore/start', json={
            'storage_type': 'webdav',
            'storage_config': _storage_config(),
            'encrypted': False,
            'local_path': 'all_fail_restore',
        })
        data = resp.json()
        assert data['success'] is True
        progress = _wait_restore_done(session, data['task_id'])
        success_count = progress.get('success_count', 0)
        failed_items = progress.get('failed_items', [])
        failed_names = [f['name'] for f in failed_items]
        print(f'  success={success_count}, failed={len(failed_items)}')
        assert success_count == 0
        assert len(failed_items) == 2
        print('PASSED')
        passed += 1

        # ===== Test 8: Restore - file content corrupted (SHA-256 check) =====
        print('\n--- Test 8: Restore - corrupted file detected by SHA-256 ---')
        mock.state.reset()

        good_content = 'correct content\n'
        bad_content = 'corrupted content\n'
        sha_good = _sha256(good_content)
        index_content = _make_index([
            ('corrupted.txt', len(good_content), sha_good),
        ])
        mock.seed_file('testbackup/.index', index_content)
        mock.seed_file('testbackup/corrupted.txt', bad_content)

        restore_dir = os.path.join(root_path, 'sha256_restore')
        os.makedirs(restore_dir, exist_ok=True)

        resp = session.post(f'{BASE_URL}/api/restore/start', json={
            'storage_type': 'webdav',
            'storage_config': _storage_config(),
            'encrypted': False,
            'local_path': 'sha256_restore',
        })
        data = resp.json()
        assert data['success'] is True
        progress = _wait_restore_done(session, data['task_id'])
        print(f'  success={progress.get("success_count", 0)}, failed={len(progress.get("failed_items", []))}')

        assert not os.path.isfile(os.path.join(restore_dir, 'corrupted.txt')), \
            'Corrupted file should not be written when SHA-256 mismatch'
        failed_items = progress.get('failed_items', [])
        assert len(failed_items) >= 1, 'Should report at least 1 failed item for corrupted file'
        print('  corrupted file rejected by SHA-256 check (expected behavior)')
        print('PASSED')
        passed += 1

        # ===== Test 9: Backup then restore - server becomes unavailable during restore =====
        print('\n--- Test 9: Backup ok, then list fails during restore ---')
        mock.state.reset()

        test_dir = os.path.join(root_path, 'normal_backup')
        os.makedirs(test_dir, exist_ok=True)
        with open(os.path.join(test_dir, 'ok_file.txt'), 'w') as f:
            f.write('normal backup content\n')

        resp = session.post(f'{BASE_URL}/api/backup/create', json={
            'name': 'Normal Then Fail',
            'storage_type': 'webdav',
            'storage_config': _storage_config(),
            'local_path': 'normal_backup',
            'encrypted': False,
            'cycle': 'daily',
            'backup_time': {'time': '08:00'},
        })
        rules = session.post(f'{BASE_URL}/api/backup/list').json()
        rule_id = rules[0]['id']

        resp = session.post(f'{BASE_URL}/api/backup/start', json={'rule_id': rule_id, 'mode': 'full'})
        progress = _wait_backup_done(session, rule_id)
        resp = session.post(f'{BASE_URL}/api/backup/logs', json={'rule_id': rule_id, 'page': 1, 'page_size': 10})
        log = resp.json()['items'][0]
        print(f'  backup: status={log["status"]}, success={log["backup_success_count"]}')
        assert log['status'] == 'completed'

        mock.state.download_fail_paths = {'testbackup/ok_file.txt'}

        restore_dir = os.path.join(root_path, 'restore_after_backup')
        os.makedirs(restore_dir, exist_ok=True)
        resp = session.post(f'{BASE_URL}/api/restore/start', json={
            'storage_type': 'webdav',
            'storage_config': _storage_config(),
            'encrypted': False,
            'local_path': 'restore_after_backup',
        })
        data = resp.json()
        assert data['success'] is True
        progress = _wait_restore_done(session, data['task_id'])
        failed_items = progress.get('failed_items', [])
        failed_names = [f['name'] for f in failed_items]
        print(f'  restore: success={progress.get("success_count", 0)}, failed={len(failed_items)}')
        assert len(failed_items) >= 1
        assert 'ok_file.txt' in failed_names
        print('PASSED')
        passed += 1

        session.post(f'{BASE_URL}/api/backup/delete', json={'id': rule_id})

        # ===== Test 10: Backup - one file succeeds, one fails =====
        print('\n--- Test 10: Backup - partial upload failure ---')
        mock.state.reset()

        test_dir = os.path.join(root_path, 'partial_upload')
        os.makedirs(test_dir, exist_ok=True)
        with open(os.path.join(test_dir, 'good.txt'), 'w') as f:
            f.write('good file\n')
        with open(os.path.join(test_dir, 'bad.txt'), 'w') as f:
            f.write('bad file\n')

        bad_path = f'{MOCK_STORAGE_PATH.lstrip("/")}/bad.txt'
        mock.state.upload_fail_filter = lambda p: p == bad_path

        resp = session.post(f'{BASE_URL}/api/backup/create', json={
            'name': 'Partial Upload',
            'storage_type': 'webdav',
            'storage_config': _storage_config(),
            'local_path': 'partial_upload',
            'encrypted': False,
            'cycle': 'daily',
            'backup_time': {'time': '08:00'},
        })
        rules = session.post(f'{BASE_URL}/api/backup/list').json()
        rule_id = rules[0]['id']

        resp = session.post(f'{BASE_URL}/api/backup/start', json={'rule_id': rule_id, 'mode': 'full'})
        progress = _wait_backup_done(session, rule_id, timeout=180)

        resp = session.post(f'{BASE_URL}/api/backup/logs', json={'rule_id': rule_id, 'page': 1, 'page_size': 10})
        log = resp.json()['items'][0]
        print(f'  status={log["status"]}, success={log["backup_success_count"]}, fail={log["backup_fail_count"]}')
        assert log['backup_success_count'] >= 1
        assert log['status'] == 'failed'
        print('PASSED')
        passed += 1

        session.post(f'{BASE_URL}/api/backup/delete', json={'id': rule_id})

        # ===== Test 11: Cancel running backup =====
        print('\n--- Test 11: Cancel running backup ---')
        mock.state.reset()
        mock.state.upload_delay = 5

        test_dir = os.path.join(root_path, 'cancel_test')
        os.makedirs(test_dir, exist_ok=True)
        for i in range(5):
            with open(os.path.join(test_dir, f'file{i}.txt'), 'w') as f:
                f.write(f'file {i} content with padding to make upload take longer\n')

        resp = session.post(f'{BASE_URL}/api/backup/create', json={
            'name': 'Cancel Test',
            'storage_type': 'webdav',
            'storage_config': _storage_config(),
            'local_path': 'cancel_test',
            'encrypted': False,
            'cycle': 'daily',
            'backup_time': {'time': '08:00'},
        })
        rules = session.post(f'{BASE_URL}/api/backup/list').json()
        rule_id = rules[0]['id']

        resp = session.post(f'{BASE_URL}/api/backup/start', json={'rule_id': rule_id, 'mode': 'full'})

        time.sleep(2)
        resp = session.post(f'{BASE_URL}/api/backup/cancel', json={'rule_id': rule_id})
        data = resp.json()
        print(f'  cancel response: {data}')
        mock.state.upload_delay = 0
        assert data.get('success') is True

        progress = _wait_backup_done(session, rule_id)
        resp = session.post(f'{BASE_URL}/api/backup/logs', json={'rule_id': rule_id, 'page': 1, 'page_size': 10})
        log = resp.json()['items'][0]
        print(f'  status={log["status"]}')
        assert log['status'] in ('cancelled', 'failed', 'partial')
        print('PASSED')
        passed += 1

        session.post(f'{BASE_URL}/api/backup/delete', json={'id': rule_id})

        # ===== Test 12: Restore retry failed file =====
        print('\n--- Test 12: Restore - retry failed file ---')
        mock.state.reset()

        content_a = 'retry file a\n'
        content_b = 'retry file b\n'
        sha_a = _sha256(content_a)
        sha_b = _sha256(content_b)

        index_content = _make_index([
            ('retry_a.txt', len(content_a), sha_a),
            ('retry_b.txt', len(content_b), sha_b),
        ])
        mock.seed_file('testbackup/.index', index_content)
        mock.seed_file('testbackup/retry_a.txt', content_a)
        mock.seed_file('testbackup/retry_b.txt', content_b)

        mock.state.download_fail_paths = {'testbackup/retry_b.txt'}

        restore_dir = os.path.join(root_path, 'retry_restore')
        os.makedirs(restore_dir, exist_ok=True)

        resp = session.post(f'{BASE_URL}/api/restore/start', json={
            'storage_type': 'webdav',
            'storage_config': _storage_config(),
            'encrypted': False,
            'local_path': 'retry_restore',
        })
        data = resp.json()
        assert data['success'] is True
        task_id = data['task_id']

        progress = _wait_restore_done(session, task_id)
        failed_items = progress.get('failed_items', [])
        failed_names = [f['name'] for f in failed_items]
        assert 'retry_b.txt' in failed_names
        print(f'  first pass: failed={failed_names}')

        mock.state.download_fail_paths.clear()

        resp = session.post(f'{BASE_URL}/api/restore/retry_file', json={
            'task_id': task_id,
            'file_path': 'retry_b.txt',
        })
        data = resp.json()
        print(f'  retry response: {data}')
        assert data.get('success') is True

        time.sleep(3)

        resp = session.post(f'{BASE_URL}/api/restore/progress', json={'task_id': task_id})
        progress = resp.json()
        remaining_failed = [f['name'] for f in progress.get('failed_items', [])]
        print(f'  after retry: success={progress.get("success_count")}, failed={remaining_failed}')

        assert os.path.isfile(os.path.join(restore_dir, 'retry_b.txt'))
        with open(os.path.join(restore_dir, 'retry_b.txt'), 'r') as f:
            assert f.read() == content_b
        print('PASSED')
        passed += 1

        print(f'\n=== {passed} 个测试通过 ===')

    except Exception:
        print(f'\n=== 测试失败 (已通过 {passed} 个) ===')
        raise
    finally:
        print_error_log(workdir, 'test_backup_restore_error')
        backend_proc.terminate()
        try:
            backend_proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            backend_proc.kill()
            backend_proc.wait()
        mock.stop()
        shutil.rmtree(workdir, ignore_errors=True)


if __name__ == '__main__':
    test_backup_restore_error()

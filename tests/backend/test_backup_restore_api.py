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

MOCK_PORT = 15244
MOCK_URL = f'http://127.0.0.1:{MOCK_PORT}'
MOCK_USER = 'admin'
MOCK_PASS = 'admin123'
MOCK_PATH = '/testbackup'


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
    while time.time() < deadline:
        resp = session.post(f'{BASE_URL}/api/restore/progress', json={'task_id': task_id})
        data = resp.json()
        if not data.get('is_running', False):
            return data
        time.sleep(1)
    raise AssertionError(f'恢复超时 ({timeout}s)')


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


def test_backup_restore_api():
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    from mock_webdav_server import MockWebDAVServer

    workdir = tempfile.mkdtemp(prefix='brookfile_brtest_')
    root_path = os.path.join(workdir, 'data')
    recycle_bin_path = os.path.join(workdir, 'recycle')

    build_backend()

    mock = MockWebDAVServer(port=MOCK_PORT)
    mock.start()
    print(f'Mock WebDAV 已启动: {mock.url}')

    backend_proc = None

    try:
        os.makedirs(root_path)
        os.makedirs(recycle_bin_path)

        backend_proc = start_backend(workdir)

        session = requests.Session()
        init_system(session, root_path, recycle_bin_path)
        login(session)

        test_dir = os.path.join(root_path, 'backup_test')
        os.makedirs(test_dir, exist_ok=True)
        with open(os.path.join(test_dir, 'hello.txt'), 'w') as f:
            f.write('Hello World\n')
        os.makedirs(os.path.join(test_dir, 'subdir'), exist_ok=True)
        with open(os.path.join(test_dir, 'subdir', 'nested.txt'), 'w') as f:
            f.write('Nested file content\n')

        for i in range(50):
            with open(os.path.join(test_dir, f'slow_{i:03d}.txt'), 'w') as f:
                f.write(f'Slow file {i} content padding\n' * 50)

        # --- Test 1: Create backup rule ---
        print('\n--- Test 1: Create backup rule ---')
        resp = session.post(f'{BASE_URL}/api/backup/create', json={
            'name': 'Test Backup',
            'storage_type': 'webdav',
            'storage_config': _storage_config(),
            'local_path': 'backup_test',
            'encrypted': False,
            'cycle': 'daily',
            'backup_time': {'time': '08:00'},
        })
        data = resp.json()
        assert data['success'] is True
        print('PASSED')

        resp = session.post(f'{BASE_URL}/api/backup/list')
        rules = resp.json()
        rule_id = rules[0]['id']
        assert rules[0]['name'] == 'Test Backup'

        # --- Test 2: Start full backup ---
        print('\n--- Test 2: Start full backup ---')
        mock.state.upload_delay = 1
        resp = session.post(f'{BASE_URL}/api/backup/start', json={
            'rule_id': rule_id,
            'mode': 'full',
        })
        data = resp.json()
        task_id = data['task_id']
        assert task_id is not None
        print(f'Task ID: {task_id}')

        deadline = time.time() + 10
        while time.time() < deadline:
            resp = session.post(f'{BASE_URL}/api/backup/progress', json={'rule_id': rule_id})
            if resp.json().get('is_running', False):
                break
            time.sleep(0.1)

        # --- Bug 17: update while backup running should return BACKUP_RUNNING ---
        print('\n--- Bug 17: update while backup running ---')
        resp = session.post(f'{BASE_URL}/api/backup/update', json={
            'id': rule_id,
            'name': 'Should Fail',
            'storage_type': 'webdav',
            'storage_config': _storage_config(),
            'local_path': 'backup_test',
            'encrypted': False,
            'cycle': 'daily',
            'backup_time': {'time': '08:00'},
        })
        data = resp.json()
        assert data['success'] is False, \
            'BUG17: Update should fail while backup is running'
        assert data.get('fail_code') == 'BACKUP_RUNNING', \
            f'BUG17: Expected BACKUP_RUNNING, got {data.get("fail_code")}'
        print('PASSED')

        # --- Bug 18: delete while backup running should return BACKUP_RUNNING ---
        print('\n--- Bug 18: delete while backup running ---')
        resp = session.post(f'{BASE_URL}/api/backup/delete', json={'id': rule_id})
        data = resp.json()
        assert data['success'] is False, \
            'BUG18: Delete should fail while backup is running'
        assert data.get('fail_code') == 'BACKUP_RUNNING', \
            f'BUG18: Expected BACKUP_RUNNING, got {data.get("fail_code")}'
        print('PASSED')

        # --- Test 3: Monitor backup progress ---
        print('\n--- Test 3: Wait for backup completion ---')
        _wait_backup_done(session, rule_id)
        mock.state.upload_delay = 0
        print('PASSED')

        # --- Test 4: Check backup logs ---
        print('\n--- Test 4: Check backup logs ---')
        resp = session.post(f'{BASE_URL}/api/backup/logs', json={
            'rule_id': rule_id,
            'page': 1,
            'page_size': 10,
        })
        data = resp.json()
        assert data['total'] >= 1
        log_item = data['items'][0]
        assert log_item['status'] in ('completed', 'partial')
        assert log_item['backup_success_count'] >= 2
        print(f'Log: status={log_item["status"]}, success={log_item["backup_success_count"]}, fail={log_item["backup_fail_count"]}')
        print('PASSED')

        # --- Test 5: Verify mock server has files ---
        print('\n--- Test 5: Verify mock server has backup files ---')
        auth = 'Basic ' + __import__('base64').b64encode(b'admin:admin123').decode()
        xml_body = '<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:"><d:prop><d:resourcetype/></d:prop></d:propfind>'
        resp = requests.request('PROPFIND', f'{MOCK_URL}/{MOCK_PATH.lstrip("/")}',
            headers={'Authorization': auth}, data=xml_body)
        assert resp.status_code == 207
        import xml.etree.ElementTree as ET
        root = ET.fromstring(resp.text)
        ns = {'d': 'DAV:'}
        file_names = []
        for r in root.findall('d:response', ns):
            if r.find('.//d:collection', ns) is None:
                href = r.find('d:href', ns)
                if href is not None and href.text:
                    name = href.text.rstrip('/').split('/')[-1]
                    if name:
                        file_names.append(name)
        print(f'Files: {file_names}')
        assert len(file_names) > 0
        print('PASSED')

        # --- Test 6: Verify backup status is idle ---
        print('\n--- Test 6: Verify backup status is idle ---')
        resp = session.post(f'{BASE_URL}/api/backup/progress', json={'rule_id': rule_id})
        data = resp.json()
        assert data['is_running'] is False
        print('PASSED')

        # --- Test 7: Check restore target ---
        print('\n--- Test 7: Check restore target ---')
        restore_dir = os.path.join(root_path, 'restored')
        os.makedirs(restore_dir, exist_ok=True)
        resp = session.post(f'{BASE_URL}/api/restore/check', json={'local_path': 'restored'})
        data = resp.json()
        assert data['is_empty'] is False or data['file_count'] == 0
        print('PASSED')

        # --- Test 8: Start restore ---
        print('\n--- Test 8: Start restore ---')
        resp = session.post(f'{BASE_URL}/api/restore/start', json={
            'storage_type': 'webdav',
            'storage_config': _storage_config(),
            'encrypted': False,
            'local_path': 'restored',
        })
        data = resp.json()
        assert data['success'] is True
        restore_task_id = data['task_id']
        assert restore_task_id is not None

        # --- Test 9: Monitor restore progress ---
        print('\n--- Test 9: Wait for restore completion ---')
        restore_progress = _wait_restore_done(session, restore_task_id)
        print(f'success={restore_progress.get("success_count")}, failed={len(restore_progress.get("failed_items", []))}')
        assert restore_progress.get('success_count', 0) >= 2

        # --- Bug 25: restore progress should use pending_count not pending_items ---
        assert 'pending_count' in restore_progress, \
            f'BUG25: Expected pending_count field, got keys: {list(restore_progress.keys())}'
        assert isinstance(restore_progress['pending_count'], (int, float)), \
            f'BUG25: pending_count should be a number, got {type(restore_progress["pending_count"])}'
        assert 'pending_items' not in restore_progress, \
            'BUG25: Should use pending_count, not pending_items'
        print('PASSED (Bug 25 check)')
        print('PASSED')

        # --- Test 10: Verify restored files ---
        print('\n--- Test 10: Verify restored files ---')
        restored_path = os.path.join(root_path, 'restored')
        assert os.path.isfile(os.path.join(restored_path, 'hello.txt'))
        with open(os.path.join(restored_path, 'hello.txt'), 'r') as f:
            assert f.read() == 'Hello World\n'
        assert os.path.isdir(os.path.join(restored_path, 'subdir'))
        assert os.path.isfile(os.path.join(restored_path, 'subdir', 'nested.txt'))
        with open(os.path.join(restored_path, 'subdir', 'nested.txt'), 'r') as f:
            assert f.read() == 'Nested file content\n'
        print('PASSED')

        # --- Test 11: Second full backup (with new file) ---
        print('\n--- Test 11: Second full backup (with new file) ---')
        with open(os.path.join(test_dir, 'new_file.txt'), 'w') as f:
            f.write('New content after first backup\n')

        resp = session.post(f'{BASE_URL}/api/backup/start', json={
            'rule_id': rule_id,
            'mode': 'full',
        })
        assert resp.json().get('task_id') is not None
        _wait_backup_done(session, rule_id)
        print('PASSED')

        # --- Test 12: Restore second backup ---
        print('\n--- Test 12: Restore second backup ---')
        restore_dir2 = os.path.join(root_path, 'restored2')
        os.makedirs(restore_dir2, exist_ok=True)

        resp = session.post(f'{BASE_URL}/api/restore/start', json={
            'storage_type': 'webdav',
            'storage_config': _storage_config(),
            'encrypted': False,
            'local_path': 'restored2',
        })
        data = resp.json()
        assert data['success'] is True
        _wait_restore_done(session, data['task_id'])

        restored2_path = os.path.join(root_path, 'restored2')
        assert os.path.isfile(os.path.join(restored2_path, 'hello.txt'))
        assert os.path.isfile(os.path.join(restored2_path, 'new_file.txt'))
        with open(os.path.join(restored2_path, 'new_file.txt'), 'r') as f:
            assert f.read() == 'New content after first backup\n'
        print('PASSED')

        # --- Test 13: Encrypted backup ---
        print('\n--- Test 13: Encrypted backup ---')
        enc_test_dir = os.path.join(root_path, 'encrypt_test')
        os.makedirs(enc_test_dir, exist_ok=True)
        with open(os.path.join(enc_test_dir, 'secret.txt'), 'w') as f:
            f.write('Secret data\n')

        resp = session.post(f'{BASE_URL}/api/backup/create', json={
            'name': 'Encrypted Backup',
            'storage_type': 'webdav',
            'storage_config': _storage_config('/testbackup_enc'),
            'local_path': 'encrypt_test',
            'encrypted': True,
            'backup_password': 'enc_pass_123',
            'cycle': 'daily',
            'backup_time': {'time': '12:00'},
        })
        assert resp.json()['success'] is True

        resp = session.post(f'{BASE_URL}/api/backup/list')
        rules = resp.json()
        enc_rule_id = next(r['id'] for r in rules if r['name'] == 'Encrypted Backup')

        resp = session.post(f'{BASE_URL}/api/backup/start', json={
            'rule_id': enc_rule_id,
            'mode': 'full',
        })
        assert resp.json().get('task_id') is not None
        _wait_backup_done(session, enc_rule_id)
        print('PASSED')

        # --- Test 14: Restore encrypted backup ---
        print('\n--- Test 14: Restore encrypted backup ---')
        enc_restore_dir = os.path.join(root_path, 'enc_restored')
        os.makedirs(enc_restore_dir, exist_ok=True)

        resp = session.post(f'{BASE_URL}/api/restore/start', json={
            'storage_type': 'webdav',
            'storage_config': _storage_config('/testbackup_enc'),
            'encrypted': True,
            'backup_password': 'enc_pass_123',
            'local_path': 'enc_restored',
        })
        data = resp.json()
        assert data['success'] is True
        _wait_restore_done(session, data['task_id'])

        enc_restored_path = os.path.join(root_path, 'enc_restored')
        assert os.path.isfile(os.path.join(enc_restored_path, 'secret.txt'))
        with open(os.path.join(enc_restored_path, 'secret.txt'), 'r') as f:
            assert f.read() == 'Secret data\n'
        print('PASSED')

        # --- Test 15: Wrong password for encrypted restore ---
        print('\n--- Test 15: Wrong password for encrypted restore ---')
        resp = session.post(f'{BASE_URL}/api/restore/start', json={
            'storage_type': 'webdav',
            'storage_config': _storage_config('/testbackup_enc'),
            'encrypted': True,
            'backup_password': 'wrong_password',
            'local_path': 'enc_restored2',
        })
        data = resp.json()
        assert data['success'] is False
        print('PASSED')

        # --- Test 16: Delete backup rules ---
        print('\n--- Test 16: Delete backup rules ---')
        resp = session.post(f'{BASE_URL}/api/backup/delete', json={'id': rule_id})
        assert resp.json()['success'] is True
        resp = session.post(f'{BASE_URL}/api/backup/delete', json={'id': enc_rule_id})
        assert resp.json()['success'] is True
        resp = session.post(f'{BASE_URL}/api/backup/list')
        assert len(resp.json()) == 0
        print('PASSED')

        # --- Test 17: H6 - Restore should detect corrupted files (SHA-256 verification) ---
        print('\n--- Test 17: H6 - Restore SHA-256 verification ---')
        h6_test_dir = os.path.join(root_path, 'h6_test')
        os.makedirs(h6_test_dir, exist_ok=True)
        h6_original = 'H6 original content for SHA-256 check\n'
        with open(os.path.join(h6_test_dir, 'check.txt'), 'w') as f:
            f.write(h6_original)

        resp = session.post(f'{BASE_URL}/api/backup/create', json={
            'name': 'H6 Backup',
            'storage_type': 'webdav',
            'storage_config': _storage_config('/testbackup_h6'),
            'local_path': 'h6_test',
            'encrypted': False,
            'cycle': 'daily',
            'backup_time': {'time': '10:00'},
        })
        assert resp.json()['success'] is True

        resp = session.post(f'{BASE_URL}/api/backup/list')
        h6_rules = resp.json()
        h6_rule_id = next(r['id'] for r in h6_rules if r['name'] == 'H6 Backup')

        resp = session.post(f'{BASE_URL}/api/backup/start', json={
            'rule_id': h6_rule_id, 'mode': 'full',
        })
        assert resp.json().get('task_id') is not None
        _wait_backup_done(session, h6_rule_id)
        print('  Backup completed')

        corrupted = b'CORRUPTED DATA - should be detected'
        corrupted_any = False
        for key in list(mock.state.files.keys()):
            if key.endswith('check.txt') or '/check.txt' in key:
                mock.state.files[key] = corrupted
                corrupted_any = True
                print(f'  Corrupted: {key}')
        if not corrupted_any:
            for key in list(mock.state.files.keys()):
                if key != '.index' and '/.index' not in key:
                    mock.state.files[key] = corrupted
                    corrupted_any = True
                    print(f'  Corrupted (fallback): {key}')
                    break
        assert corrupted_any, 'No file found to corrupt on mock WebDAV'

        h6_restore_dir = os.path.join(root_path, 'h6_restored')
        os.makedirs(h6_restore_dir, exist_ok=True)
        resp = session.post(f'{BASE_URL}/api/restore/start', json={
            'storage_type': 'webdav',
            'storage_config': _storage_config('/testbackup_h6'),
            'encrypted': False,
            'local_path': 'h6_restored',
        })
        data = resp.json()
        assert data['success'] is True
        h6_progress = _wait_restore_done(session, data['task_id'])
        h6_failed = h6_progress.get('failed_items', [])
        h6_success = h6_progress.get('success_count', 0)

        h6_restored_file = os.path.join(root_path, 'h6_restored', 'check.txt')
        if os.path.exists(h6_restored_file):
            with open(h6_restored_file, 'r') as f:
                h6_content = f.read()
            if h6_content != h6_original and len(h6_failed) == 0:
                assert False, \
                    'H6 BUG: Restored file is corrupted but restore reported no failures! ' \
                    f'Expected: "{h6_original[:30]}...", Got: "{h6_content[:30]}..."'
            elif h6_content != h6_original and len(h6_failed) > 0:
                print(f'  SHA-256 check worked: corrupted file detected ({len(h6_failed)} failed)')
            else:
                print('  Content matches original (no corruption in data)')
        else:
            if len(h6_failed) > 0:
                print(f'  SHA-256 check may have worked: {len(h6_failed)} failed items')
            else:
                assert False, 'H6: Restored file missing and no failures reported'

        resp = session.post(f'{BASE_URL}/api/backup/delete', json={'id': h6_rule_id})
        assert resp.json()['success'] is True
        print('PASSED')

        # --- Test 18: BUG7 - restore/start should return STORAGE_CONNECTION_ERROR ---
        print('\n--- Test 18: BUG7 - STORAGE_CONNECTION_ERROR for unreachable storage ---')
        resp = session.post(f'{BASE_URL}/api/restore/start', json={
            'storage_type': 'webdav',
            'storage_config': {
                'address': 'http://127.0.0.1:19999',
                'username': 'test',
                'password': 'test',
                'path': '/nonexistent',
            },
            'encrypted': False,
            'local_path': 'h6_restored',
        })
        data = resp.json()
        assert data['success'] is False, \
            'BUG7: restore/start should fail for unreachable storage'
        assert data['fail_code'] == 'STORAGE_CONNECTION_ERROR', \
            f'BUG7: Expected STORAGE_CONNECTION_ERROR, got {data.get("fail_code")}'
        assert data.get('message') is not None and len(data.get('message', '')) > 0, \
            'BUG7: message field should contain specific error info'
        print('PASSED')

        print('\n=== 所有测试通过 ===')

    except Exception:
        print('\n=== 测试失败 ===')
        raise
    finally:
        print_error_log(workdir, 'test_backup_restore_api')
        if backend_proc:
            backend_proc.terminate()
            try:
                backend_proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                backend_proc.kill()
                backend_proc.wait()
        mock.stop()
        shutil.rmtree(workdir, ignore_errors=True)


if __name__ == '__main__':
    test_backup_restore_api()

import requests
from test_utils import run_tests, BASE_URL


def test_backup_api(session, root_path):
    # --- list: empty ---
    resp = session.post(f'{BASE_URL}/api/backup/list')
    rules = resp.json()
    assert isinstance(rules, list)
    assert len(rules) == 0

    # --- create: success ---
    resp = session.post(f'{BASE_URL}/api/backup/create', json={
        'name': 'TestBackup',
        'storage_type': 'webdav',
        'storage_config': {
            'address': 'http://localhost:5244',
            'username': 'test',
            'password': 'test123',
            'path': '/backup'
        },
        'local_path': 'data',
        'cycle': 'daily',
        'backup_time': {'time': '08:00'}
    })
    data = resp.json()
    assert data['success'] is True

    # --- create: weekly with encryption ---
    resp = session.post(f'{BASE_URL}/api/backup/create', json={
        'name': 'Weekly Backup',
        'storage_type': 'webdav',
        'storage_config': {
            'address': 'http://127.0.0.1:15245',
            'username': 'testuser2',
            'password': 'testpass456',
            'path': '/backup/weekly'
        },
        'local_path': '/data/backup2',
        'encrypted': True,
        'backup_password': 'encrypt123',
        'cycle': 'weekly',
        'backup_time': {'week_day': 1, 'time': '10:00'}
    })
    assert resp.json()['success'] is True

    # --- create: monthly ---
    resp = session.post(f'{BASE_URL}/api/backup/create', json={
        'name': 'Monthly Backup',
        'storage_type': 'webdav',
        'storage_config': {
            'address': 'http://127.0.0.1:15246',
            'username': 'testuser3',
            'password': 'testpass789',
            'path': '/backup/monthly'
        },
        'local_path': '/data/backup3',
        'cycle': 'monthly',
        'backup_time': {'month_day': 15, 'time': '12:00'}
    })
    assert resp.json()['success'] is True

    # --- create: yearly ---
    resp = session.post(f'{BASE_URL}/api/backup/create', json={
        'name': 'Yearly Backup',
        'storage_type': 'webdav',
        'storage_config': {
            'address': 'http://127.0.0.1:15247',
            'username': 'testuser4',
            'password': 'testpass000',
            'path': '/backup/yearly'
        },
        'local_path': '/data/backup4',
        'cycle': 'yearly',
        'backup_time': {'year_date': '01-01', 'time': '00:00'}
    })
    assert resp.json()['success'] is True

    # --- list: 4 items, verify fields ---
    resp = session.post(f'{BASE_URL}/api/backup/list')
    rules = resp.json()
    assert len(rules) == 4

    daily_rule = next(r for r in rules if r['name'] == 'TestBackup')
    assert daily_rule['storage_type'] == 'webdav'
    assert daily_rule['cycle'] == 'daily'
    assert daily_rule['backup_time']['time'] == '08:00'
    assert daily_rule['status'] == 'idle'
    rule_id = daily_rule['id']

    weekly_rule = next(r for r in rules if r['name'] == 'Weekly Backup')
    assert weekly_rule['cycle'] == 'weekly'
    assert weekly_rule['backup_time']['week_day'] == 1
    assert weekly_rule['backup_time']['time'] == '10:00'

    # --- get ---
    resp = session.post(f'{BASE_URL}/api/backup/get', json={'id': rule_id})
    rule = resp.json()
    assert rule['name'] == 'TestBackup'
    assert rule['storage_type'] == 'webdav'
    assert rule['storage_config']['address'] == 'http://localhost:5244'
    assert rule['storage_config']['username'] == 'test'
    assert rule['storage_config']['path'] == '/backup'
    assert 'password' not in rule['storage_config']
    assert rule['local_path'] == 'data'
    assert rule['cycle'] == 'daily'

    # --- get encrypted rule ---
    resp = session.post(f'{BASE_URL}/api/backup/get', json={'id': weekly_rule['id']})
    rule = resp.json()
    assert rule['encrypted'] is True
    assert 'password' not in rule['storage_config']

    # --- get: not found ---
    resp = session.post(f'{BASE_URL}/api/backup/get', json={'id': 'nonexistent'})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'BACKUP_RULE_NOT_FOUND'

    # --- create: invalid storage_type ---
    resp = session.post(f'{BASE_URL}/api/backup/create', json={
        'name': 'Invalid',
        'storage_type': 'invalid',
        'storage_config': {
            'address': 'https://test.com',
            'username': 'test',
            'password': 'test',
            'path': '/test'
        },
        'local_path': '/test',
        'cycle': 'daily',
        'backup_time': {'time': '08:00'}
    })
    assert resp.json()['fail_code'] == 'INVALID_STORAGE_TYPE'

    # --- create: invalid cycle ---
    resp = session.post(f'{BASE_URL}/api/backup/create', json={
        'name': 'Invalid Cycle',
        'storage_type': 'webdav',
        'storage_config': {
            'address': 'https://test.com',
            'username': 'test',
            'password': 'test',
            'path': '/test'
        },
        'local_path': '/test',
        'cycle': 'invalid',
        'backup_time': {'time': '08:00'}
    })
    assert resp.json()['fail_code'] == 'INVALID_CYCLE'

    # --- create: empty name ---
    resp = session.post(f'{BASE_URL}/api/backup/create', json={
        'name': '',
        'storage_type': 'webdav',
        'storage_config': {
            'address': 'https://test.com',
            'username': 'test',
            'password': 'test',
            'path': '/test'
        },
        'local_path': '/test',
        'cycle': 'daily',
        'backup_time': {'time': '08:00'}
    })
    assert resp.json()['fail_code'] == 'NAME_EMPTY'

    # --- update ---
    resp = session.post(f'{BASE_URL}/api/backup/update', json={
        'id': rule_id,
        'name': 'UpdatedBackup',
        'storage_type': 'webdav',
        'storage_config': {
            'address': 'http://localhost:5244',
            'username': 'test',
            'password': 'newpass',
            'path': '/backup2'
        },
        'local_path': 'data',
        'cycle': 'weekly',
        'backup_time': {'week_day': 1, 'time': '10:00'}
    })
    assert resp.json()['success'] is True

    # --- verify update ---
    resp = session.post(f'{BASE_URL}/api/backup/get', json={'id': rule_id})
    rule = resp.json()
    assert rule['name'] == 'UpdatedBackup'
    assert rule['cycle'] == 'weekly'
    assert rule['storage_config']['path'] == '/backup2'

    # --- update: not found ---
    resp = session.post(f'{BASE_URL}/api/backup/update', json={
        'id': 'nonexistent',
        'name': 'x',
        'storage_type': 'webdav',
        'storage_config': {
            'address': 'http://localhost:5244',
            'username': 'test',
            'password': 'pass',
            'path': '/backup'
        },
        'local_path': 'data',
        'cycle': 'daily',
        'backup_time': {'time': '08:00'}
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'BACKUP_RULE_NOT_FOUND'

    # --- progress: not running ---
    resp = session.post(f'{BASE_URL}/api/backup/progress', json={'rule_id': rule_id})
    data = resp.json()
    assert data['is_running'] is False

    # --- BUG19-21: backup progress not-running response field names ---
    assert data.get('phase') == 'backup', \
        f'BUG21: Expected phase="backup" when not running, got phase={data.get("phase")}'
    assert 'total_count' in data, \
        'BUG20: Missing total_count field in not-running progress response'
    assert 'scanned_bytes' in data, \
        f'BUG19: Expected scanned_bytes field, got keys: {list(data.keys())}'
    assert 'scanned_count' not in data, \
        'BUG19: Should use scanned_bytes, not scanned_count'

    # --- progress: not found ---
    resp = session.post(f'{BASE_URL}/api/backup/progress', json={'rule_id': 'nonexistent'})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'BACKUP_RULE_NOT_FOUND'

    # --- cancel: not running ---
    resp = session.post(f'{BASE_URL}/api/backup/cancel', json={'rule_id': rule_id})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'TASK_NOT_RUNNING'

    # --- logs: empty ---
    resp = session.post(f'{BASE_URL}/api/backup/logs', json={
        'rule_id': rule_id,
        'page': 1,
        'page_size': 10
    })
    data = resp.json()
    assert data['total'] == 0
    assert data['items'] == []

    # --- logs: not found ---
    resp = session.post(f'{BASE_URL}/api/backup/logs', json={
        'rule_id': 'nonexistent',
        'page': 1,
        'page_size': 10
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'BACKUP_RULE_NOT_FOUND'

    # --- delete ---
    resp = session.post(f'{BASE_URL}/api/backup/delete', json={'id': rule_id})
    assert resp.json()['success'] is True

    # --- verify deletion ---
    resp = session.post(f'{BASE_URL}/api/backup/list')
    rules = resp.json()
    assert len(rules) == 3
    assert not any(r['id'] == rule_id for r in rules)

    # --- delete: not found ---
    resp = session.post(f'{BASE_URL}/api/backup/delete', json={'id': rule_id})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'BACKUP_RULE_NOT_FOUND'

    # --- not logged in ---
    anon = requests.Session()
    resp = anon.post(f'{BASE_URL}/api/backup/list')
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_LOGGED_IN'

    # --- BUG6: backup/create swallows specific error codes ---
    # Model returns NAME_EMPTY/LOCAL_PATH_EMPTY/INVALID_STORAGE_TYPE/INVALID_CYCLE
    # but handler maps everything except INVALID_PARAM to INTERNAL_ERROR.
    # Per API doc these should be returned as specific fail_codes.

    resp = session.post(f'{BASE_URL}/api/backup/create', json={
        'name': '',
        'storage_type': 'webdav',
        'storage_config': {
            'address': 'https://test.com',
            'username': 'test',
            'password': 'test',
            'path': '/test'
        },
        'local_path': '/test',
        'cycle': 'daily',
        'backup_time': {'time': '08:00'}
    })
    assert resp.json()['fail_code'] == 'NAME_EMPTY', \
        f'BUG6: Expected NAME_EMPTY for empty name, got {resp.json().get("fail_code")}'

    resp = session.post(f'{BASE_URL}/api/backup/create', json={
        'name': 'Test',
        'storage_type': 'webdav',
        'storage_config': {
            'address': 'https://test.com',
            'username': 'test',
            'password': 'test',
            'path': '/test'
        },
        'local_path': '',
        'cycle': 'daily',
        'backup_time': {'time': '08:00'}
    })
    assert resp.json()['fail_code'] == 'LOCAL_PATH_EMPTY', \
        f'BUG6: Expected LOCAL_PATH_EMPTY for empty local_path, got {resp.json().get("fail_code")}'

    resp = session.post(f'{BASE_URL}/api/backup/create', json={
        'name': 'Test',
        'storage_type': 'invalid',
        'storage_config': {
            'address': 'https://test.com',
            'username': 'test',
            'password': 'test',
            'path': '/test'
        },
        'local_path': '/test',
        'cycle': 'daily',
        'backup_time': {'time': '08:00'}
    })
    assert resp.json()['fail_code'] == 'INVALID_STORAGE_TYPE', \
        f'BUG6: Expected INVALID_STORAGE_TYPE, got {resp.json().get("fail_code")}'

    resp = session.post(f'{BASE_URL}/api/backup/create', json={
        'name': 'Test',
        'storage_type': 'webdav',
        'storage_config': {
            'address': 'https://test.com',
            'username': 'test',
            'password': 'test',
            'path': '/test'
        },
        'local_path': '/test',
        'cycle': 'invalid_cycle',
        'backup_time': {'time': '08:00'}
    })
    assert resp.json()['fail_code'] == 'INVALID_CYCLE', \
        f'BUG6: Expected INVALID_CYCLE, got {resp.json().get("fail_code")}'

    # cleanup
    resp = session.post(f'{BASE_URL}/api/backup/list')
    for r in resp.json():
        session.post(f'{BASE_URL}/api/backup/delete', json={'id': r['id']})


if __name__ == '__main__':
    run_tests(test_backup_api)

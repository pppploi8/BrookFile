import hmac
import hashlib
import io
import os
import zipfile
import requests
from datetime import datetime, timedelta, timezone
from test_utils import run_tests, BASE_URL


def _upload_file(session, path, content):
    resp = session.post(f'{BASE_URL}/api/file/upload_start', json={'files': [path]})
    uid = resp.json()['uploads'][0]['id']
    session.post(f'{BASE_URL}/api/file/upload_chunk', files={
        'upload_id': (None, uid),
        'offset': (None, '0'),
        'chunk': ('chunk', content, 'application/octet-stream')
    })
    session.post(f'{BASE_URL}/api/file/upload_complete', json={'upload_id': uid})


def test_share_api(session, root_path):
    _upload_file(session, 'share_test.txt', b'share content')

    # --- create share: not found file ---
    resp = session.post(f'{BASE_URL}/api/share/create', json={
        'file_path': 'nonexist.txt',
        'expire_type': 'permanent',
        'share_mode': 'page'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'FILE_NOT_FOUND'

    # --- create share: page mode, no password ---
    resp = session.post(f'{BASE_URL}/api/share/create', json={
        'file_path': 'share_test.txt',
        'expire_type': 'permanent',
        'share_mode': 'page'
    })
    data = resp.json()
    assert data['success'] is True
    share_code = data['share_code']
    assert len(share_code) == 8
    assert data['share_url'] == f'/s/{share_code}'
    assert data['direct_url'] == f'/api/share/file/{share_code}'

    # --- create share: same file → SHARE_ALREADY_EXISTS ---
    resp = session.post(f'{BASE_URL}/api/share/create', json={
        'file_path': 'share_test.txt',
        'expire_type': 'permanent',
        'share_mode': 'page'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'SHARE_ALREADY_EXISTS'

    # --- create time-limited share ---
    _upload_file(session, 'share_time.txt', b'time limited')
    expire_time = (datetime.now(timezone.utc) + timedelta(hours=1)).strftime('%Y-%m-%d %H:%M:%S')
    resp = session.post(f'{BASE_URL}/api/share/create', json={
        'file_path': 'share_time.txt',
        'expire_type': 'time',
        'expire_at': expire_time,
        'share_mode': 'direct'
    })
    data = resp.json()
    assert data['success'] is True
    time_code = data['share_code']

    # --- create count-limited share ---
    _upload_file(session, 'share_count.txt', b'count limited')
    resp = session.post(f'{BASE_URL}/api/share/create', json={
        'file_path': 'share_count.txt',
        'expire_type': 'count',
        'max_downloads': 3,
        'share_mode': 'page'
    })
    data = resp.json()
    assert data['success'] is True
    count_code = data['share_code']

    # --- direct + password → SHARE_DIRECT_NO_PASSWORD ---
    _upload_file(session, 'direct_pwd.txt', b'direct with password')
    resp = session.post(f'{BASE_URL}/api/share/create', json={
        'file_path': 'direct_pwd.txt',
        'expire_type': 'permanent',
        'share_mode': 'direct',
        'password': 'secretpwd'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'SHARE_DIRECT_NO_PASSWORD'

    anon = requests.Session()

    # --- share info ---
    resp = anon.post(f'{BASE_URL}/api/share/info', json={'share_code': share_code})
    data = resp.json()
    assert data['success'] is True
    assert data['file_name'] == 'share_test.txt'
    assert data['is_directory'] is False
    assert data['share_mode'] == 'page'
    assert data['need_password'] is False
    assert data['expire_type'] == 'permanent'

    # --- share info: not found ---
    resp = anon.post(f'{BASE_URL}/api/share/info', json={'share_code': 'AAAAAAAA'})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'SHARE_NOT_FOUND'

    # --- download nonexistent share → SHARE_NOT_FOUND ---
    resp = anon.get(f'{BASE_URL}/api/share/file/ZZZZZZZZ')
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'SHARE_NOT_FOUND'

    # --- get download token ---
    resp = anon.post(f'{BASE_URL}/api/share/get_download_token', json={
        'share_code': share_code
    })
    data = resp.json()
    assert data['success'] is True
    token = data['download_token']

    # --- download share file ---
    resp = anon.get(f'{BASE_URL}/api/share/file/{share_code}?token={token}')
    assert resp.status_code == 200
    assert resp.content == b'share content'
    assert 'share_test.txt' in resp.headers.get('Content-Disposition', '')

    # --- download without token: page mode → denied ---
    resp = anon.get(f'{BASE_URL}/api/share/file/{share_code}')
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'SHARE_DOWNLOAD_DENIED'

    # --- download with invalid token → denied ---
    resp = anon.get(f'{BASE_URL}/api/share/file/{share_code}?token=invalid_token')
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'SHARE_DOWNLOAD_DENIED'

    # --- directory share ---
    os.makedirs(os.path.join(root_path, 'share_dir'), exist_ok=True)
    with open(os.path.join(root_path, 'share_dir', 'file1.txt'), 'wb') as f:
        f.write(b'file1 content')
    with open(os.path.join(root_path, 'share_dir', 'file2.txt'), 'wb') as f:
        f.write(b'file2 content')

    resp = session.post(f'{BASE_URL}/api/share/create', json={
        'file_path': 'share_dir',
        'expire_type': 'permanent',
        'share_mode': 'page'
    })
    data = resp.json()
    assert data['success'] is True
    dir_code = data['share_code']

    resp = anon.post(f'{BASE_URL}/api/share/get_download_token', json={'share_code': dir_code})
    dir_token = resp.json()['download_token']
    resp = anon.get(f'{BASE_URL}/api/share/file/{dir_code}?token={dir_token}')
    assert resp.status_code == 200
    assert resp.content[:2] == b'PK'
    zf = zipfile.ZipFile(io.BytesIO(resp.content))
    names = zf.namelist()
    assert any('file1.txt' in n for n in names)
    assert any('file2.txt' in n for n in names)

    # --- get by path ---
    resp = session.post(f'{BASE_URL}/api/share/get_by_path', json={
        'file_path': 'share_test.txt'
    })
    data = resp.json()
    assert data['success'] is True
    assert data['share'] is not None
    assert data['share']['share_code'] == share_code
    assert data['share']['has_password'] is False
    assert data['share']['status'] == 'active'

    # --- get by path: not shared ---
    resp = session.post(f'{BASE_URL}/api/share/get_by_path', json={
        'file_path': 'noexist.txt'
    })
    data = resp.json()
    assert data['success'] is True
    assert data['share'] is None

    # --- list shares ---
    resp = session.post(f'{BASE_URL}/api/share/list')
    data = resp.json()
    assert data['success'] is True
    assert len(data['shares']) >= 2
    for s in data['shares']:
        assert 'id' in s
        assert 'share_code' in s
        assert 'file_name' in s
        assert 'status' in s
        assert 'expire_type' in s
        assert 'share_mode' in s
        assert 'created_at' in s

    # --- share code uniqueness ---
    codes = []
    for i in range(5):
        _upload_file(session, f'unique_test_{i}.txt', f'unique {i}'.encode())
        resp = session.post(f'{BASE_URL}/api/share/create', json={
            'file_path': f'unique_test_{i}.txt',
            'expire_type': 'permanent',
            'share_mode': 'page'
        })
        codes.append(resp.json()['share_code'])
    assert len(set(codes)) == len(codes)

    # --- create share with password ---
    _upload_file(session, 'secret.txt', b'secret content')
    resp = session.post(f'{BASE_URL}/api/share/create', json={
        'file_path': 'secret.txt',
        'expire_type': 'permanent',
        'share_mode': 'page',
        'password': 'mypass123'
    })
    data = resp.json()
    assert data['success'] is True
    pw_share_code = data['share_code']

    # --- share info with password ---
    resp = anon.post(f'{BASE_URL}/api/share/info', json={'share_code': pw_share_code})
    data = resp.json()
    assert data['success'] is True
    assert data['need_password'] is True
    assert data['password_salt'] is not None
    salt = data['password_salt']

    # --- get download token: no password → SHARE_PASSWORD_REQUIRED ---
    resp = anon.post(f'{BASE_URL}/api/share/get_download_token', json={
        'share_code': pw_share_code
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'SHARE_PASSWORD_REQUIRED'

    # --- get download token: wrong password ---
    wrong_hash = hmac.new(salt.encode(), b'wrongpass', hashlib.sha256).hexdigest()
    resp = anon.post(f'{BASE_URL}/api/share/get_download_token', json={
        'share_code': pw_share_code,
        'password_hash': wrong_hash
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'SHARE_PASSWORD_WRONG'

    # --- get download token: correct password ---
    correct_hash = hmac.new(salt.encode(), b'mypass123', hashlib.sha256).hexdigest()
    resp = anon.post(f'{BASE_URL}/api/share/get_download_token', json={
        'share_code': pw_share_code,
        'password_hash': correct_hash
    })
    data = resp.json()
    assert data['success'] is True
    pw_token = data['download_token']

    # --- download with token ---
    resp = anon.get(f'{BASE_URL}/api/share/file/{pw_share_code}?token={pw_token}')
    assert resp.status_code == 200
    assert resp.content == b'secret content'

    # --- download without token → denied ---
    resp = anon.get(f'{BASE_URL}/api/share/file/{pw_share_code}')
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'SHARE_DOWNLOAD_DENIED'

    # --- time expiry ---
    _upload_file(session, 'expire_test.txt', b'will expire')
    past_time = (datetime.now(timezone.utc) - timedelta(hours=1)).strftime('%Y-%m-%d %H:%M:%S')
    resp = session.post(f'{BASE_URL}/api/share/create', json={
        'file_path': 'expire_test.txt',
        'expire_type': 'time',
        'expire_at': past_time,
        'share_mode': 'page'
    })
    expired_code = resp.json()['share_code']
    resp = anon.post(f'{BASE_URL}/api/share/info', json={'share_code': expired_code})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'SHARE_EXPIRED'
    resp = anon.get(f'{BASE_URL}/api/share/file/{expired_code}')
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'SHARE_EXPIRED'

    # --- download count limit ---
    _upload_file(session, 'limit_test.txt', b'download limit test')
    resp = session.post(f'{BASE_URL}/api/share/create', json={
        'file_path': 'limit_test.txt',
        'expire_type': 'count',
        'max_downloads': 2,
        'share_mode': 'page'
    })
    limit_code = resp.json()['share_code']
    resp = anon.post(f'{BASE_URL}/api/share/get_download_token', json={'share_code': limit_code})
    limit_token = resp.json()['download_token']

    resp = anon.get(f'{BASE_URL}/api/share/file/{limit_code}?token={limit_token}')
    assert resp.status_code == 200

    resp = anon.post(f'{BASE_URL}/api/share/get_download_token', json={'share_code': limit_code})
    limit_token = resp.json()['download_token']
    resp = anon.get(f'{BASE_URL}/api/share/file/{limit_code}?token={limit_token}')
    assert resp.status_code == 200

    resp = anon.post(f'{BASE_URL}/api/share/get_download_token', json={'share_code': limit_code})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'SHARE_OVER_LIMIT'

    resp = anon.post(f'{BASE_URL}/api/share/info', json={'share_code': limit_code})
    info_data = resp.json()
    assert info_data['success'] is False
    assert info_data['fail_code'] == 'SHARE_OVER_LIMIT'

    # --- file missing detection ---
    _upload_file(session, 'missing_test.txt', b'will be deleted')
    resp = session.post(f'{BASE_URL}/api/share/create', json={
        'file_path': 'missing_test.txt',
        'expire_type': 'permanent',
        'share_mode': 'page'
    })
    missing_code = resp.json()['share_code']
    os.remove(os.path.join(root_path, 'missing_test.txt'))
    resp = anon.post(f'{BASE_URL}/api/share/info', json={'share_code': missing_code})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'SHARE_FILE_MISSING'

    # --- Bug 52: download count should only increment after successful file send ---
    _upload_file(session, 'bug52_count.txt', b'bug52 count check')
    resp = session.post(f'{BASE_URL}/api/share/create', json={
        'file_path': 'bug52_count.txt',
        'expire_type': 'count',
        'max_downloads': 2,
        'share_mode': 'page'
    })
    bug52_code = resp.json()['share_code']
    bug52_anon = requests.Session()
    bug52_info_before = bug52_anon.post(f'{BASE_URL}/api/share/info', json={'share_code': bug52_code}).json()
    assert bug52_info_before['download_count'] == 0, 'download_count should be 0 before download'

    resp = bug52_anon.post(f'{BASE_URL}/api/share/get_download_token', json={'share_code': bug52_code})
    bug52_token = resp.json()['download_token']
    resp = bug52_anon.get(f'{BASE_URL}/api/share/file/{bug52_code}?token={bug52_token}')
    assert resp.status_code == 200
    assert resp.content == b'bug52 count check'

    bug52_info_after = bug52_anon.post(f'{BASE_URL}/api/share/info', json={'share_code': bug52_code}).json()
    assert bug52_info_after['download_count'] == 1, \
        f'BUG52: download_count should be 1 after successful download, got {bug52_info_after["download_count"]}'

    resp = session.post(f'{BASE_URL}/api/share/list')
    bug52_shares = resp.json()['shares']
    bug52_ids = [s['id'] for s in bug52_shares if s['share_code'] == bug52_code]
    if bug52_ids:
        session.post(f'{BASE_URL}/api/share/delete', json={'ids': bug52_ids})

    # --- not logged in ---
    no_auth = requests.Session()
    resp = no_auth.post(f'{BASE_URL}/api/share/create', json={
        'file_path': 'test.txt',
        'expire_type': 'permanent',
        'share_mode': 'page'
    })
    assert resp.json()['fail_code'] == 'NOT_LOGGED_IN'
    no_auth2 = requests.Session()
    resp = no_auth2.post(f'{BASE_URL}/api/share/get_by_path', json={'file_path': 'test.txt'})
    assert resp.json()['fail_code'] == 'NOT_LOGGED_IN'
    no_auth3 = requests.Session()
    resp = no_auth3.post(f'{BASE_URL}/api/share/list')
    assert resp.json()['fail_code'] == 'NOT_LOGGED_IN'
    no_auth4 = requests.Session()
    resp = no_auth4.post(f'{BASE_URL}/api/share/delete', json={'ids': ['fake-id']})
    assert resp.json()['fail_code'] == 'NOT_LOGGED_IN'

    # --- delete single share by id ---
    resp = session.post(f'{BASE_URL}/api/share/list')
    shares = resp.json()['shares']
    single_share = next(s for s in shares if s['share_code'] == share_code)
    resp = session.post(f'{BASE_URL}/api/share/delete', json={'ids': [single_share['id']]})
    assert resp.json()['success'] is True
    resp = anon.post(f'{BASE_URL}/api/share/info', json={'share_code': share_code})
    assert resp.json()['fail_code'] == 'SHARE_NOT_FOUND'

    # --- delete batch shares by ids ---
    resp = session.post(f'{BASE_URL}/api/share/list')
    all_ids = [s['id'] for s in resp.json()['shares']]
    resp = session.post(f'{BASE_URL}/api/share/delete', json={'ids': all_ids})
    assert resp.json()['success'] is True

    # --- verify all deleted ---
    resp = session.post(f'{BASE_URL}/api/share/list')
    data = resp.json()
    assert data['success'] is True
    assert len(data['shares']) == 0

    # --- create direct mode share ---
    _upload_file(session, 'direct.txt', b'direct content')
    resp = session.post(f'{BASE_URL}/api/share/create', json={
        'file_path': 'direct.txt',
        'expire_type': 'permanent',
        'share_mode': 'direct'
    })
    data = resp.json()
    assert data['success'] is True
    direct_code = data['share_code']

    # --- info: direct mode ---
    resp = anon.post(f'{BASE_URL}/api/share/info', json={'share_code': direct_code})
    data = resp.json()
    assert data['success'] is True
    assert data['share_mode'] == 'direct'
    assert data['need_password'] is False

    # --- download direct mode without token ---
    resp = anon.get(f'{BASE_URL}/api/share/file/{direct_code}')
    assert resp.status_code == 200
    assert resp.content == b'direct content'

    # cleanup
    resp = session.post(f'{BASE_URL}/api/share/list')
    all_ids = [s['id'] for s in resp.json()['shares']]
    session.post(f'{BASE_URL}/api/share/delete', json={'ids': all_ids})

    # --- BUG: get_download_token uses share_code as HashMap key ---
    # When two different users request download tokens for the same share,
    # the second token overwrites the first, causing the first user's download to fail.
    _upload_file(session, 'token_bug_test.txt', b'token bug content')
    resp = session.post(f'{BASE_URL}/api/share/create', json={
        'file_path': 'token_bug_test.txt',
        'expire_type': 'permanent',
        'share_mode': 'page'
    })
    token_bug_code = resp.json()['share_code']

    user1 = requests.Session()
    user2 = requests.Session()

    resp1 = user1.post(f'{BASE_URL}/api/share/get_download_token', json={
        'share_code': token_bug_code
    })
    token1 = resp1.json()['download_token']

    resp2 = user2.post(f'{BASE_URL}/api/share/get_download_token', json={
        'share_code': token_bug_code
    })
    token2 = resp2.json()['download_token']

    assert token1 != token2, \
        'Each download request should get a unique token'

    # BUG: token1 no longer works because token2 overwrote it in the HashMap.
    # The download endpoint returns HTTP 200 with JSON {success:false, fail_code:"SHARE_DOWNLOAD_DENIED"}
    # when the token is invalid, rather than the file content.
    resp = user1.get(f'{BASE_URL}/api/share/file/{token_bug_code}?token={token1}')
    download_ok = resp.status_code == 200 and (
        resp.headers.get('Content-Type', '').startswith('application/json') is False
        or resp.json().get('success') is not False
    )
    assert download_ok, \
        'BUG: First user token should still work after second user gets a token, ' \
        'but share_code-based HashMap key causes token2 to overwrite token1.'

    # token2 should also work
    resp = user2.get(f'{BASE_URL}/api/share/file/{token_bug_code}?token={token2}')
    assert resp.status_code == 200
    assert resp.content == b'token bug content'

    # cleanup
    resp = session.post(f'{BASE_URL}/api/share/list')
    all_ids = [s['id'] for s in resp.json()['shares']]
    session.post(f'{BASE_URL}/api/share/delete', json={'ids': all_ids})

    # --- BUG: get_share_by_path should return expired/file_missing shares with status ---
    _upload_file(session, 'bug_status_test.txt', b'status check')
    resp = session.post(f'{BASE_URL}/api/share/create', json={
        'file_path': 'bug_status_test.txt',
        'expire_type': 'permanent',
        'share_mode': 'page'
    })
    bug_status_code = resp.json()['share_code']

    resp = session.post(f'{BASE_URL}/api/share/get_by_path', json={
        'file_path': 'bug_status_test.txt'
    })
    data = resp.json()
    assert data['success'] is True
    assert data['share'] is not None
    assert data['share']['status'] == 'active'

    os.remove(os.path.join(root_path, 'bug_status_test.txt'))
    resp = session.post(f'{BASE_URL}/api/share/get_by_path', json={
        'file_path': 'bug_status_test.txt'
    })
    data = resp.json()
    assert data['success'] is True
    assert data['share'] is not None, \
        'BUG: get_share_by_path should return share with file_missing status, not None'
    assert data['share']['status'] == 'file_missing', \
        f'BUG: Expected file_missing status, got {data["share"]["status"]}'

    _upload_file(session, 'bug_expired_status.txt', b'will expire')
    past_time = (datetime.now(timezone.utc) - timedelta(hours=1)).strftime('%Y-%m-%d %H:%M:%S')
    resp = session.post(f'{BASE_URL}/api/share/create', json={
        'file_path': 'bug_expired_status.txt',
        'expire_type': 'time',
        'expire_at': past_time,
        'share_mode': 'page'
    })
    expired_status_code = resp.json()['share_code']

    resp = session.post(f'{BASE_URL}/api/share/get_by_path', json={
        'file_path': 'bug_expired_status.txt'
    })
    data = resp.json()
    assert data['success'] is True
    assert data['share'] is not None, \
        'BUG: get_share_by_path should return expired share with expired status, not None'
    assert data['share']['status'] == 'expired', \
        f'BUG: Expected expired status, got {data["share"]["status"]}'

    resp = session.post(f'{BASE_URL}/api/share/list')
    all_ids = [s['id'] for s in resp.json()['shares']]
    session.post(f'{BASE_URL}/api/share/delete', json={'ids': all_ids})


if __name__ == '__main__':
    run_tests(test_share_api)

from test_utils import run_tests, BASE_URL


def test_auth_api(session, root_path):
    session.post(f'{BASE_URL}/api/auth/logout')

    # --- login: wrong password ---
    resp = session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'admin',
        'password': 'wrong'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'INVALID_USERNAME_OR_PASSWORD'

    # --- login: success ---
    resp = session.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'admin',
        'password': 'password123'
    })
    assert resp.json()['success'] is True

    # --- info: logged in ---
    resp = session.post(f'{BASE_URL}/api/system/info')
    data = resp.json()
    assert data['logged_in'] is True
    assert data['user']['username'] == 'admin'
    assert data['user']['is_admin'] is True

    # --- logout ---
    resp = session.post(f'{BASE_URL}/api/auth/logout')
    assert resp.json()['success'] is True

    # --- logout: not logged in ---
    resp = session.post(f'{BASE_URL}/api/auth/logout')
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_LOGGED_IN'

    # --- info: logged out ---
    resp = session.post(f'{BASE_URL}/api/system/info')
    data = resp.json()
    assert data['initialized'] is True
    assert data['logged_in'] is False


if __name__ == '__main__':
    run_tests(test_auth_api)

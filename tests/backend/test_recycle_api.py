import os
import requests
from test_utils import run_tests, BASE_URL


def test_recycle_api(session, root_path):
    recycle_bin_path = os.path.join(os.path.dirname(root_path), 'recycle')

    # --- 1. system/info has recycle_bin_enabled=True for admin ---
    resp = session.post(f'{BASE_URL}/api/system/info')
    data = resp.json()
    assert data['user']['recycle_bin_enabled'] is True

    # --- 2. Create user2 without recycle_bin_path ---
    workdir = os.path.dirname(root_path)
    resp = session.post(f'{BASE_URL}/api/user/create', json={
        'username': 'user2',
        'password': 'password123',
        'root_path': root_path
    })
    assert resp.json()['success'] is True
    user_list = session.post(f'{BASE_URL}/api/user/list').json()
    user2 = next(u for u in user_list if u['username'] == 'user2')
    user2_id = user2['id']
    assert user2['recycle_bin_path'] is None

    # --- 3. Login as user2 → recycle_bin_enabled=False ---
    session2 = requests.Session()
    resp = session2.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'user2',
        'password': 'password123'
    })
    assert resp.json()['success'] is True
    resp = session2.post(f'{BASE_URL}/api/system/info')
    assert resp.json()['user']['recycle_bin_enabled'] is False

    # --- 4. Create test1.txt, delete → success, file gone ---
    test_file = os.path.join(root_path, 'test1.txt')
    with open(test_file, 'w') as f:
        f.write('A' * 100)
    resp = session.post(f'{BASE_URL}/api/file/delete', json={'path': 'test1.txt'})
    assert resp.json()['success'] is True
    assert not os.path.exists(test_file)

    # --- 5. recycle/list → total=1, check fields ---
    resp = session.post(f'{BASE_URL}/api/recycle/list', json={})
    data = resp.json()
    assert data['success'] is True
    assert data['data']['total'] == 1
    item = data['data']['items'][0]
    assert item['original_path'] == 'test1.txt'
    assert item['original_name'] == 'test1.txt'
    assert item['is_directory'] is False
    assert item['file_size'] == 100
    record_id = item['id']

    # --- 6. File physically in recycle_bin_path/{record_id}/test1.txt ---
    recycle_file = os.path.join(recycle_bin_path, record_id, 'test1.txt')
    assert os.path.exists(recycle_file)

    # --- 7. Create test_folder, delete → is_directory=True, file_size=300 ---
    folder_path = os.path.join(root_path, 'test_folder')
    sub_path = os.path.join(folder_path, 'sub')
    os.makedirs(sub_path)
    with open(os.path.join(sub_path, 'file_a.txt'), 'wb') as f:
        f.write(b'A' * 100)
    with open(os.path.join(sub_path, 'file_b.txt'), 'wb') as f:
        f.write(b'B' * 200)

    resp = session.post(f'{BASE_URL}/api/file/delete', json={'path': 'test_folder'})
    assert resp.json()['success'] is True
    assert not os.path.exists(folder_path)

    resp = session.post(f'{BASE_URL}/api/recycle/list', json={})
    data = resp.json()
    folder_item = next(i for i in data['data']['items'] if i['original_name'] == 'test_folder')
    assert folder_item['is_directory'] is True
    assert folder_item['file_size'] == 300

    # --- 8. Directory structure preserved in recycle bin ---
    recycle_folder_dir = os.path.join(recycle_bin_path, folder_item['id'], 'test_folder')
    assert os.path.exists(os.path.join(recycle_folder_dir, 'sub', 'file_a.txt'))
    assert os.path.exists(os.path.join(recycle_folder_dir, 'sub', 'file_b.txt'))

    # --- 9. Batch delete → recycle/list total=5 ---
    for i in range(3):
        with open(os.path.join(root_path, f'batch_{i}.txt'), 'w') as f:
            f.write(f'content {i}')
    resp = session.post(f'{BASE_URL}/api/file/batch_delete', json={
        'files': ['batch_0.txt', 'batch_1.txt', 'batch_2.txt']
    })
    assert resp.json()['success'] is True
    resp = session.post(f'{BASE_URL}/api/recycle/list', json={})
    assert resp.json()['data']['total'] == 5

    # --- 10. Restore test1.txt ---
    resp = session.post(f'{BASE_URL}/api/recycle/list', json={})
    data = resp.json()
    test1_item = next(i for i in data['data']['items'] if i['original_path'] == 'test1.txt')
    test1_id = test1_item['id']

    resp = session.post(f'{BASE_URL}/api/recycle/restore', json={'id': test1_id})
    assert resp.json()['success'] is True
    assert os.path.exists(test_file)
    with open(test_file, 'r') as f:
        assert f.read() == 'A' * 100

    resp = session.post(f'{BASE_URL}/api/recycle/list', json={})
    data = resp.json()
    assert not any(i['id'] == test1_id for i in data['data']['items'])
    assert not os.path.exists(os.path.join(recycle_bin_path, test1_id))

    # --- 11. Restore path conflict ---
    with open(os.path.join(root_path, 'conflict_file.txt'), 'w') as f:
        f.write('original content')
    resp = session.post(f'{BASE_URL}/api/file/delete', json={'path': 'conflict_file.txt'})
    assert resp.json()['success'] is True
    with open(os.path.join(root_path, 'conflict_file.txt'), 'w') as f:
        f.write('new content')

    resp = session.post(f'{BASE_URL}/api/recycle/list', json={})
    data = resp.json()
    conflict_item = next(i for i in data['data']['items'] if i['original_path'] == 'conflict_file.txt')

    resp = session.post(f'{BASE_URL}/api/recycle/restore', json={'id': conflict_item['id']})
    assert resp.json()['success'] is False
    assert resp.json()['fail_code'] == 'RESTORE_PATH_OCCUPIED'
    with open(os.path.join(root_path, 'conflict_file.txt'), 'r') as f:
        assert f.read() == 'new content'

    # --- 12. Restore with missing parent ---
    os.makedirs(os.path.join(root_path, 'dir'))
    with open(os.path.join(root_path, 'dir', 'file.txt'), 'w') as f:
        f.write('dir file content')
    resp = session.post(f'{BASE_URL}/api/file/delete', json={'path': 'dir/file.txt'})
    assert resp.json()['success'] is True
    resp = session.post(f'{BASE_URL}/api/file/delete', json={'path': 'dir'})
    assert resp.json()['success'] is True
    assert not os.path.exists(os.path.join(root_path, 'dir'))

    resp = session.post(f'{BASE_URL}/api/recycle/list', json={})
    data = resp.json()
    dir_file_item = next(i for i in data['data']['items'] if i['original_path'] == 'dir/file.txt')

    resp = session.post(f'{BASE_URL}/api/recycle/restore', json={'id': dir_file_item['id']})
    assert resp.json()['success'] is True
    assert os.path.exists(os.path.join(root_path, 'dir', 'file.txt'))

    # --- 13. Batch restore ---
    for i in range(3):
        with open(os.path.join(root_path, f'batch_restore_{i}.txt'), 'w') as f:
            f.write(f'restore content {i}')
    resp = session.post(f'{BASE_URL}/api/file/batch_delete', json={
        'files': [f'batch_restore_{i}.txt' for i in range(3)]
    })
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/recycle/list', json={})
    data = resp.json()
    restore_ids = [i['id'] for i in data['data']['items'] if i['original_name'].startswith('batch_restore_')]

    resp = session.post(f'{BASE_URL}/api/recycle/batch_restore', json={'ids': restore_ids})
    assert resp.json()['success'] is True
    for i in range(3):
        assert os.path.exists(os.path.join(root_path, f'batch_restore_{i}.txt'))

    # --- 14. Batch restore with conflict ---
    with open(os.path.join(root_path, 'conflict_batch.txt'), 'w') as f:
        f.write('will be deleted')
    resp = session.post(f'{BASE_URL}/api/file/delete', json={'path': 'conflict_batch.txt'})
    assert resp.json()['success'] is True
    with open(os.path.join(root_path, 'conflict_batch.txt'), 'w') as f:
        f.write('recreated')

    with open(os.path.join(root_path, 'normal_batch.txt'), 'w') as f:
        f.write('normal')
    resp = session.post(f'{BASE_URL}/api/file/delete', json={'path': 'normal_batch.txt'})
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/recycle/list', json={})
    data = resp.json()
    conflict_batch_item = next(i for i in data['data']['items'] if i['original_path'] == 'conflict_batch.txt')
    normal_batch_item = next(i for i in data['data']['items'] if i['original_path'] == 'normal_batch.txt')

    resp = session.post(f'{BASE_URL}/api/recycle/batch_restore', json={
        'ids': [conflict_batch_item['id'], normal_batch_item['id']]
    })
    assert resp.json()['success'] is False
    assert resp.json()['fail_code'] == 'RESTORE_PATH_OCCUPIED'
    conflict_items = resp.json()['data']['conflict_items']
    assert len(conflict_items) == 1
    assert conflict_items[0]['id'] == conflict_batch_item['id']

    # --- 15. Permanent delete single record ---
    with open(os.path.join(root_path, 'perm_delete.txt'), 'w') as f:
        f.write('to be permanently deleted')
    resp = session.post(f'{BASE_URL}/api/file/delete', json={'path': 'perm_delete.txt'})
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/recycle/list', json={})
    data = resp.json()
    perm_item = next(i for i in data['data']['items'] if i['original_path'] == 'perm_delete.txt')
    perm_id = perm_item['id']
    perm_dir = os.path.join(recycle_bin_path, perm_id)
    assert os.path.exists(perm_dir)

    resp = session.post(f'{BASE_URL}/api/recycle/delete', json={'id': perm_id})
    assert resp.json()['success'] is True
    assert not os.path.exists(perm_dir)

    resp = session.post(f'{BASE_URL}/api/recycle/list', json={})
    data = resp.json()
    assert not any(i['id'] == perm_id for i in data['data']['items'])

    # --- 16. Batch permanent delete ---
    for i in range(3):
        with open(os.path.join(root_path, f'batch_perm_{i}.txt'), 'w') as f:
            f.write(f'batch perm {i}')
    resp = session.post(f'{BASE_URL}/api/file/batch_delete', json={
        'files': [f'batch_perm_{i}.txt' for i in range(3)]
    })
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/recycle/list', json={})
    data = resp.json()
    batch_perm_ids = [i['id'] for i in data['data']['items'] if i['original_name'].startswith('batch_perm_')]

    resp = session.post(f'{BASE_URL}/api/recycle/batch_delete', json={'ids': batch_perm_ids})
    assert resp.json()['success'] is True
    for bid in batch_perm_ids:
        assert not os.path.exists(os.path.join(recycle_bin_path, bid))

    resp = session.post(f'{BASE_URL}/api/recycle/list', json={})
    data = resp.json()
    assert not any(i['id'] in batch_perm_ids for i in data['data']['items'])

    # --- 17. Empty recycle bin ---
    for i in range(3):
        with open(os.path.join(root_path, f'empty_test_{i}.txt'), 'w') as f:
            f.write(f'empty {i}')
    resp = session.post(f'{BASE_URL}/api/file/batch_delete', json={
        'files': [f'empty_test_{i}.txt' for i in range(3)]
    })
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/recycle/empty', json={})
    assert resp.json()['success'] is True
    recycle_entries = os.listdir(recycle_bin_path) if os.path.exists(recycle_bin_path) else []
    assert len(recycle_entries) == 0

    resp = session.post(f'{BASE_URL}/api/recycle/list', json={})
    assert resp.json()['data']['total'] == 0

    # --- 18. user2 without recycle → permanent delete ---
    with open(os.path.join(root_path, 'user2_file.txt'), 'w') as f:
        f.write('user2 file')
    resp = session2.post(f'{BASE_URL}/api/file/delete', json={'path': 'user2_file.txt'})
    assert resp.json()['success'] is True
    assert not os.path.exists(os.path.join(root_path, 'user2_file.txt'))

    resp = session2.post(f'{BASE_URL}/api/recycle/list', json={})
    assert resp.json()['success'] is False
    assert resp.json()['fail_code'] == 'RECYCLE_NOT_ENABLED'

    resp = session2.post(f'{BASE_URL}/api/recycle/restore', json={'id': 'fake-id'})
    assert resp.json()['success'] is False
    assert resp.json()['fail_code'] == 'RECYCLE_NOT_ENABLED'

    # --- 19-20. Pagination ---
    for i in range(25):
        with open(os.path.join(root_path, f'page_{i:02d}.txt'), 'w') as f:
            f.write(f'page content {i}')
    resp = session.post(f'{BASE_URL}/api/file/batch_delete', json={
        'files': [f'page_{i:02d}.txt' for i in range(25)]
    })
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/recycle/list', json={'page': 1, 'page_size': 20})
    data = resp.json()['data']
    assert len(data['items']) == 20
    assert data['total'] == 25

    resp = session.post(f'{BASE_URL}/api/recycle/list', json={'page': 2, 'page_size': 20})
    data = resp.json()['data']
    assert len(data['items']) == 5

    # --- 21. page_size=10 ---
    resp = session.post(f'{BASE_URL}/api/recycle/list', json={'page': 1, 'page_size': 10})
    data = resp.json()['data']
    assert len(data['items']) == 10
    assert data['total'] == 25

    # --- 22. Restore nonexistent ---
    resp = session.post(f'{BASE_URL}/api/recycle/restore', json={'id': 'nonexistent-id'})
    assert resp.json()['success'] is False
    assert resp.json()['fail_code'] == 'RECYCLE_ITEM_NOT_FOUND'

    # --- 23. Delete nonexistent ---
    resp = session.post(f'{BASE_URL}/api/recycle/delete', json={'id': 'nonexistent-id'})
    assert resp.json()['success'] is False
    assert resp.json()['fail_code'] == 'RECYCLE_ITEM_NOT_FOUND'

    # --- 24. Cross-user isolation ---
    session.post(f'{BASE_URL}/api/recycle/empty', json={})

    user3_recycle = os.path.join(workdir, 'user3_recycle_cross')
    resp = session.post(f'{BASE_URL}/api/user/create', json={
        'username': 'user3_cross',
        'password': 'password123',
        'root_path': root_path,
        'recycle_bin_path': user3_recycle
    })
    assert resp.json()['success'] is True

    session3 = requests.Session()
    resp = session3.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'user3_cross',
        'password': 'password123'
    })
    assert resp.json()['success'] is True

    with open(os.path.join(root_path, 'admin_only.txt'), 'w') as f:
        f.write('admin file')
    resp = session.post(f'{BASE_URL}/api/file/delete', json={'path': 'admin_only.txt'})
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/recycle/list', json={})
    admin_item = resp.json()['data']['items'][0]

    resp = session3.post(f'{BASE_URL}/api/recycle/restore', json={'id': admin_item['id']})
    assert resp.json()['success'] is False
    assert resp.json()['fail_code'] == 'RECYCLE_ITEM_NOT_FOUND'

    resp = session3.post(f'{BASE_URL}/api/recycle/delete', json={'id': admin_item['id']})
    assert resp.json()['success'] is False
    assert resp.json()['fail_code'] == 'RECYCLE_ITEM_NOT_FOUND'

    # --- 25. Create user3 with recycle_bin_path ---
    user3_recycle_path = os.path.join(workdir, 'user3_recycle')
    resp = session.post(f'{BASE_URL}/api/user/create', json={
        'username': 'user3',
        'password': 'password123',
        'root_path': root_path,
        'recycle_bin_path': user3_recycle_path
    })
    assert resp.json()['success'] is True

    user_list = session.post(f'{BASE_URL}/api/user/list').json()
    user3 = next(u for u in user_list if u['username'] == 'user3')
    assert user3['recycle_bin_path'] == user3_recycle_path

    # --- 26. Update user3 to clear recycle_bin_path ---
    resp = session.post(f'{BASE_URL}/api/user/update', json={
        'id': user3['id'],
        'recycle_bin_path': None
    })
    assert resp.json()['success'] is True

    user_list = session.post(f'{BASE_URL}/api/user/list').json()
    user3 = next(u for u in user_list if u['username'] == 'user3')
    assert user3['recycle_bin_path'] is None

    session3b = requests.Session()
    resp = session3b.post(f'{BASE_URL}/api/auth/login', json={
        'username': 'user3',
        'password': 'password123'
    })
    assert resp.json()['success'] is True
    resp = session3b.post(f'{BASE_URL}/api/system/info')
    assert resp.json()['user']['recycle_bin_enabled'] is False


if __name__ == '__main__':
    run_tests(test_recycle_api)

import os
import time
from test_utils import run_frontend_test, FRONTEND_URL, _ref


def _get_files_comp(agent):
    comps = agent.find_components_by_name('Files')
    assert len(comps) > 0, 'Files component not found'
    return comps[0]


def _wait_upload_done(agent, uid, timeout=15):
    deadline = time.time() + timeout
    while time.time() < deadline:
        state = agent.get_component_state(uid)
        tasks = _ref(state['setupState']['uploadTasks'])
        all_done = all(t['status'] in ('completed', 'failed', 'cancelled') for t in tasks)
        if all_done and len(tasks) > 0:
            return tasks
        time.sleep(0.3)
    return None


def test_file_manager(page, agent, root_path, workdir):
    page.wait_for_url('**/files', timeout=5000)
    agent.wait_ready(timeout=15000)

    comp = _get_files_comp(agent)
    uid = comp['id']

    # --- 1. Initial file list: empty root ---
    state = agent.get_component_state(uid)
    file_list = _ref(state['setupState']['fileList'])
    assert isinstance(file_list, list)

    # --- 2. Create folder ---
    agent.call_method(uid, 'handleNewFolder')
    time.sleep(0.3)
    state = agent.get_component_state(uid)
    assert _ref(state['setupState']['newFolderDialogVisible']) is True

    agent.set_ref(uid, 'newFolderName', 'test_folder')
    agent.clear_messages()
    agent.call_method(uid, 'confirmCreateFolder')
    time.sleep(1.0)
    msgs = agent.get_messages()
    assert any(m['key'] == 'files.createFolderSuccess' for m in msgs if m['type'] == 'success')
    time.sleep(0.5)

    state = agent.get_component_state(uid)
    file_list = _ref(state['setupState']['fileList'])
    names = [f['name'] for f in file_list]
    assert 'test_folder' in names

    # --- 3. Create a subfolder inside test_folder ---
    folder_row = next(f for f in file_list if f['name'] == 'test_folder')
    agent.call_method(uid, 'handleFileClick', [folder_row])
    time.sleep(0.5)
    state = agent.get_component_state(uid)
    assert _ref(state['setupState']['currentPath']) == 'test_folder'

    agent.call_method(uid, 'handleNewFolder')
    time.sleep(0.3)
    agent.set_ref(uid, 'newFolderName', 'sub_folder')
    agent.clear_messages()
    agent.call_method(uid, 'confirmCreateFolder')
    agent.wait_for_message('success', timeout=5000)
    time.sleep(0.5)

    # --- 4. Create folder with empty name → warning ---
    agent.call_method(uid, 'handleNewFolder')
    time.sleep(0.3)
    agent.set_ref(uid, 'newFolderName', '')
    agent.clear_messages()
    agent.call_method(uid, 'confirmCreateFolder')
    msg = agent.wait_for_message('warning', timeout=5000)
    assert msg is not None
    assert msg['key'] == 'files.pleaseEnterFolderName'

    # --- 5. Navigate via breadcrumb to root ---
    agent.call_method(uid, 'handlePathClick', [-1])
    time.sleep(0.5)
    state = agent.get_component_state(uid)
    assert _ref(state['setupState']['currentPath']) == ''

    # --- 6. Upload file via API (simulate by calling loadFiles after API upload) ---
    # The file upload uses HTML file input which is hard to trigger from agent,
    # so we upload via backend API directly and verify the file list refreshes
    test_file_content = b'Hello, this is a test file for upload!'
    test_file_path = os.path.join(root_path, 'test_file.txt')
    with open(test_file_path, 'wb') as f:
        f.write(test_file_content)

    agent.call_method(uid, 'loadFiles')
    time.sleep(0.5)
    state = agent.get_component_state(uid)
    file_list = _ref(state['setupState']['fileList'])
    names = [f['name'] for f in file_list]
    assert 'test_file.txt' in names
    test_file_row = next(f for f in file_list if f['name'] == 'test_file.txt')
    assert test_file_row['file_type'] == 'file'
    assert test_file_row['size'] == len(test_file_content)

    # --- 7. Download file: trigger handleDownload and check dialog opens ---
    agent.call_method(uid, 'handleDownload', [test_file_row])
    time.sleep(1.0)
    state = agent.get_component_state(uid)
    dl_task = _ref(state['setupState']['downloadTask'])
    assert dl_task is not None
    assert dl_task['name'] == 'test_file.txt'

    # wait for download to complete
    deadline = time.time() + 10
    while time.time() < deadline:
        state = agent.get_component_state(uid)
        dl_task = _ref(state['setupState']['downloadTask'])
        if dl_task and dl_task['status'] == 'completed':
            break
        time.sleep(0.3)

    assert dl_task['status'] == 'completed'
    assert dl_task['progress'] == 100
    agent.call_method(uid, 'closeDownloadDialog')

    # --- 8. Move file into test_folder ---
    state = agent.get_component_state(uid)
    file_list = _ref(state['setupState']['fileList'])
    test_file_row = next(f for f in file_list if f['name'] == 'test_file.txt')
    agent.call_method(uid, 'handleSelectionChange', [[test_file_row]])
    time.sleep(0.3)

    agent.call_method(uid, 'openMoveDialog')
    time.sleep(0.5)
    state = agent.get_component_state(uid)
    assert _ref(state['setupState']['moveDialogVisible']) is True

    browse_folders = _ref(state['setupState']['moveBrowserFolders'])
    test_folder_entry = next(f for f in browse_folders if f['name'] == 'test_folder')
    agent.call_method(uid, 'handleMoveFolderClick', [test_folder_entry])
    time.sleep(0.5)

    agent.clear_messages()
    agent.call_method(uid, 'confirmMove')
    msg = agent.wait_for_message('success', timeout=5000)
    assert msg is not None
    assert msg['key'] == 'files.moveSuccess'
    time.sleep(0.5)

    state = agent.get_component_state(uid)
    file_list = _ref(state['setupState']['fileList'])
    names = [f['name'] for f in file_list]
    assert 'test_file.txt' not in names

    # verify file is inside test_folder
    folder_row = next(f for f in file_list if f['name'] == 'test_folder')
    agent.call_method(uid, 'handleFileClick', [folder_row])
    time.sleep(0.5)
    state = agent.get_component_state(uid)
    file_list = _ref(state['setupState']['fileList'])
    names = [f['name'] for f in file_list]
    assert 'test_file.txt' in names

    # --- 9. Move to same folder → rejected ---
    test_file_row = next(f for f in file_list if f['name'] == 'test_file.txt')
    agent.call_method(uid, 'handleSelectionChange', [[test_file_row]])
    time.sleep(0.3)
    agent.call_method(uid, 'openMoveDialog')
    time.sleep(0.5)

    # set target to current path (same folder)
    agent.set_ref(uid, 'moveTargetPath', 'test_folder')
    agent.clear_messages()
    agent.call_method(uid, 'confirmMove')
    msg = agent.wait_for_message('error', timeout=5000)
    assert msg is not None
    assert msg['key'] == 'files.cannotMoveToSameFolder'

    # close move dialog
    agent.set_ref(uid, 'moveDialogVisible', False)

    # --- 10. Go back to root ---
    agent.call_method(uid, 'handlePathClick', [-1])
    time.sleep(0.5)

    # --- 11. Delete file from root (the sub_folder) ---
    state = agent.get_component_state(uid)
    file_list = _ref(state['setupState']['fileList'])

    # Create another file to delete
    del_file_path = os.path.join(root_path, 'to_delete.txt')
    with open(del_file_path, 'wb') as f:
        f.write(b'delete me')
    agent.call_method(uid, 'loadFiles')
    time.sleep(0.5)

    state = agent.get_component_state(uid)
    file_list = _ref(state['setupState']['fileList'])
    del_row = next(f for f in file_list if f['name'] == 'to_delete.txt')

    # handleDelete uses ElMessageBox.confirm which we can't interact with via agent,
    # so we test via API-level delete
    import requests
    session = requests.Session()
    session.post(f'{FRONTEND_URL}/api/auth/login', json={'username': 'admin', 'password': 'password123'})
    session.post(f'{FRONTEND_URL}/api/file/delete', json={'path': 'to_delete.txt'})
    assert not os.path.exists(del_file_path)

    agent.call_method(uid, 'loadFiles')
    time.sleep(0.5)
    state = agent.get_component_state(uid)
    file_list = _ref(state['setupState']['fileList'])
    names = [f['name'] for f in file_list]
    assert 'to_delete.txt' not in names

    # --- 12. Share: open share drawer for test_folder ---
    state = agent.get_component_state(uid)
    file_list = _ref(state['setupState']['fileList'])
    folder_row = next(f for f in file_list if f['name'] == 'test_folder')
    agent.call_method(uid, 'handleShare', [folder_row])
    time.sleep(1.0)

    state = agent.get_component_state(uid)
    assert _ref(state['setupState']['shareDrawerVisible']) is True
    assert _ref(state['setupState']['shareFileName']) == 'test_folder'
    assert _ref(state['setupState']['existingShare']) is None

    # --- 13. Create share: permanent, page mode, no password ---
    share_form = _ref(state['setupState']['shareForm'])
    assert share_form['expire_type'] == 'permanent'
    assert share_form['share_mode'] == 'page'
    assert share_form['usePassword'] is False

    agent.clear_messages()
    agent.call_method(uid, 'handleCreateShare')
    msg = agent.wait_for_message('success', timeout=5000)
    assert msg is not None
    assert msg['key'] == 'share.shareCreated'
    time.sleep(0.5)

    state = agent.get_component_state(uid)
    existing = _ref(state['setupState']['existingShare'])
    assert existing is not None
    share_code = existing['share_code']
    assert share_code is not None and len(share_code) > 0

    # --- 14. Cancel share ---
    agent.call_method(uid, 'handleCancelShare')
    time.sleep(0.3)
    # The cancel uses ElMessageBox, so we trigger it via API
    share_id = existing['id']
    session.post(f'{FRONTEND_URL}/api/share/delete', json={'ids': [share_id]})

    # --- 15. Create share with password ---
    agent.call_method(uid, 'handleShare', [folder_row])
    time.sleep(1.0)

    agent.set_reactive_field(uid, 'shareForm', 'usePassword', True)
    agent.set_reactive_field(uid, 'shareForm', 'password', 'secret123')
    agent.clear_messages()
    agent.call_method(uid, 'handleCreateShare')
    msg = agent.wait_for_message('success', timeout=5000)
    assert msg is not None
    assert msg['key'] == 'share.shareCreated'
    time.sleep(0.5)

    state = agent.get_component_state(uid)
    existing = _ref(state['setupState']['existingShare'])
    assert existing is not None
    assert existing['has_password'] is True
    share_with_pw_code = existing['share_code']

    # --- 16. Create share with count limit ---
    session.post(f'{FRONTEND_URL}/api/share/delete', json={'ids': [existing['id']]})

    agent.call_method(uid, 'handleShare', [folder_row])
    time.sleep(1.0)

    agent.set_reactive_field(uid, 'shareForm', 'expire_type', 'count')
    agent.set_reactive_field(uid, 'shareForm', 'max_downloads', 3)
    agent.set_reactive_field(uid, 'shareForm', 'usePassword', False)
    agent.clear_messages()
    agent.call_method(uid, 'handleCreateShare')
    msg = agent.wait_for_message('success', timeout=5000)
    assert msg is not None
    time.sleep(0.5)

    state = agent.get_component_state(uid)
    existing = _ref(state['setupState']['existingShare'])
    assert existing is not None
    assert existing['expire_type'] == 'count'
    assert existing['max_downloads'] == 3

    # --- 17. Create direct link share ---
    session.post(f'{FRONTEND_URL}/api/share/delete', json={'ids': [existing['id']]})

    agent.call_method(uid, 'handleShare', [folder_row])
    time.sleep(1.0)

    agent.set_reactive_field(uid, 'shareForm', 'expire_type', 'permanent')
    agent.set_reactive_field(uid, 'shareForm', 'share_mode', 'direct')
    agent.set_reactive_field(uid, 'shareForm', 'usePassword', False)
    agent.clear_messages()
    agent.call_method(uid, 'handleCreateShare')
    msg = agent.wait_for_message('success', timeout=5000)
    assert msg is not None
    time.sleep(0.5)

    state = agent.get_component_state(uid)
    existing = _ref(state['setupState']['existingShare'])
    assert existing is not None
    assert existing['share_mode'] == 'direct'
    direct_share_code = existing['share_code']

    # Close share drawer
    agent.set_ref(uid, 'shareDrawerVisible', False)

    # --- 18. Navigate to recycle bin ---
    page.evaluate('() => { const app = document.getElementById("app").__vue_app__; const router = app.config.globalProperties.$router; router.push("/recycle-bin"); }')
    time.sleep(1.5)
    page.wait_for_url('**/recycle-bin', timeout=5000)
    assert agent.find_components_by_name('RecycleBin'), 'RecycleBin component not found'


if __name__ == '__main__':
    run_frontend_test(test_file_manager)

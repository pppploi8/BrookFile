import os
import time
from test_utils import run_frontend_test, _ref


def _get_recycle_comp(agent):
    comps = agent.find_components_by_name('RecycleBin')
    assert len(comps) > 0, 'RecycleBin component not found'
    return comps[0]


def _wait_for_load(agent, uid, timeout=10):
    deadline = time.time() + timeout
    while time.time() < deadline:
        state = agent.get_component_state(uid)
        loading = _ref(state['setupState']['loading'])
        if loading is False:
            return state
        time.sleep(0.3)
    return agent.get_component_state(uid)


def _navigate_to_recycle_bin(page, agent):
    page.evaluate('() => { const app = document.getElementById("app").__vue_app__; const router = app.config.globalProperties.$router; router.push("/recycle-bin"); }')
    time.sleep(1.5)
    comp = _get_recycle_comp(agent)
    state = _wait_for_load(agent, comp['id'])
    return comp, state


def _delete_files_via_api(page, file_paths):
    for fp in file_paths:
        page.evaluate("""async (fp) => {
            await fetch('/api/file/delete', {
                method: 'POST',
                headers: {'Content-Type': 'application/json'},
                body: JSON.stringify({path: fp}),
                credentials: 'include'
            });
        }""", fp)
    time.sleep(0.5)


def test_recycle_bin(page, agent, root_path, workdir):
    page.wait_for_url('**/files', timeout=5000)
    agent.wait_ready(timeout=15000)

    # --- Setup: create files and delete them to populate recycle bin ---
    with open(os.path.join(root_path, 'file1.txt'), 'w') as f:
        f.write('content 1')
    with open(os.path.join(root_path, 'file2.txt'), 'w') as f:
        f.write('content 2')
    with open(os.path.join(root_path, 'file3.txt'), 'w') as f:
        f.write('content 3')
    os.makedirs(os.path.join(root_path, 'folder1'))
    with open(os.path.join(root_path, 'folder1', 'nested.txt'), 'w') as f:
        f.write('nested content')

    _delete_files_via_api(page, ['file1.txt', 'file2.txt', 'file3.txt', 'folder1'])

    # --- 1. Navigate to recycle bin, verify items loaded ---
    comp, state = _navigate_to_recycle_bin(page, agent)
    uid = comp['id']

    items = _ref(state['setupState']['items'])
    total = _ref(state['setupState']['total'])
    assert total >= 4, f'Expected at least 4 items, got {total}'
    assert len(items) >= 4, f'Expected at least 4 items in list, got {len(items)}'

    paths = [item['original_path'] for item in items]
    assert 'file1.txt' in paths
    assert 'file2.txt' in paths
    assert 'file3.txt' in paths
    assert 'folder1' in paths

    # --- 2. Verify item structure ---
    file1_item = next(i for i in items if i['original_path'] == 'file1.txt')
    assert file1_item['is_directory'] is False
    assert file1_item['file_size'] == len('content 1')
    assert file1_item['deleted_at'] is not None
    assert file1_item['id'] is not None

    folder1_item = next(i for i in items if i['original_path'] == 'folder1')
    assert folder1_item['is_directory'] is True

    # --- 3. Search/filter items ---
    agent.set_ref(uid, 'searchKeyword', 'file1')
    time.sleep(0.3)
    state = agent.get_component_state(uid)
    display_items = _ref(state['setupState']['displayItems'])
    assert len(display_items) == 1
    assert display_items[0]['original_path'] == 'file1.txt'

    agent.set_ref(uid, 'searchKeyword', '')
    time.sleep(0.3)

    # --- 4. Restore single item ---
    state = agent.get_component_state(uid)
    items = _ref(state['setupState']['items'])
    file1_item = next(i for i in items if i['original_path'] == 'file1.txt')

    agent.clear_messages()
    agent.call_method(uid, 'handleRestore', [file1_item])
    agent.wait_for_message('success', timeout=5000)
    time.sleep(0.5)

    assert os.path.exists(os.path.join(root_path, 'file1.txt'))

    state = _wait_for_load(agent, uid)
    items = _ref(state['setupState']['items'])
    total = _ref(state['setupState']['total'])
    paths = [item['original_path'] for item in items]
    assert 'file1.txt' not in paths
    assert total >= 3

    # --- 5. Batch restore via component ---
    state = agent.get_component_state(uid)
    items = _ref(state['setupState']['items'])
    file2_item = next(i for i in items if i['original_path'] == 'file2.txt')
    file3_item = next(i for i in items if i['original_path'] == 'file3.txt')

    agent.call_method(uid, 'handleSelectionChange', [[file2_item, file3_item]])
    time.sleep(0.3)

    state = agent.get_component_state(uid)
    selected = _ref(state['setupState']['selectedItems'])
    assert len(selected) == 2

    agent.clear_messages()
    agent.call_method(uid, 'handleBatchRestore')
    agent.wait_for_message('success', timeout=5000)
    time.sleep(0.5)

    assert os.path.exists(os.path.join(root_path, 'file2.txt'))
    assert os.path.exists(os.path.join(root_path, 'file3.txt'))

    state = _wait_for_load(agent, uid)
    items = _ref(state['setupState']['items'])
    total = _ref(state['setupState']['total'])
    assert total >= 1
    paths = [item['original_path'] for item in items]
    assert 'folder1' in paths

    # --- 6. Permanent delete single item via API (ElMessageBox bypass) ---
    state = agent.get_component_state(uid)
    items = _ref(state['setupState']['items'])
    folder1_item = next(i for i in items if i['original_path'] == 'folder1')

    page.evaluate("""async (id) => {
        await fetch('/api/recycle/delete', {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({id: id}),
            credentials: 'include'
        });
    }""", folder1_item['id'])

    agent.call_method(uid, 'loadList')
    state = _wait_for_load(agent, uid)
    items = _ref(state['setupState']['items'])
    total = _ref(state['setupState']['total'])
    assert total == 0
    assert len(items) == 0

    # --- 7. Empty recycle bin with items ---
    with open(os.path.join(root_path, 'more1.txt'), 'w') as f:
        f.write('x')
    with open(os.path.join(root_path, 'more2.txt'), 'w') as f:
        f.write('y')
    _delete_files_via_api(page, ['more1.txt', 'more2.txt'])

    agent.call_method(uid, 'loadList')
    state = _wait_for_load(agent, uid)
    total = _ref(state['setupState']['total'])
    assert total >= 2

    page.evaluate("""async () => {
        await fetch('/api/recycle/empty', {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({}),
            credentials: 'include'
        });
    }""")

    agent.call_method(uid, 'loadList')
    state = _wait_for_load(agent, uid)
    total = _ref(state['setupState']['total'])
    assert total == 0

    # --- 8. Selection bar behavior ---
    with open(os.path.join(root_path, 'sel1.txt'), 'w') as f:
        f.write('s1')
    with open(os.path.join(root_path, 'sel2.txt'), 'w') as f:
        f.write('s2')
    _delete_files_via_api(page, ['sel1.txt', 'sel2.txt'])

    agent.call_method(uid, 'loadList')
    state = _wait_for_load(agent, uid)
    items = _ref(state['setupState']['items'])
    assert len(items) >= 2

    sel1 = next(i for i in items if i['original_path'] == 'sel1.txt')
    agent.call_method(uid, 'handleSelectionChange', [[sel1]])
    time.sleep(0.3)
    state = agent.get_component_state(uid)
    selected = _ref(state['setupState']['selectedItems'])
    assert len(selected) == 1
    assert selected[0]['original_path'] == 'sel1.txt'

    # --- 9. Pagination state ---
    state = agent.get_component_state(uid)
    assert _ref(state['setupState']['currentPage']) == 1
    assert _ref(state['setupState']['pageSize']) == 20


if __name__ == '__main__':
    run_frontend_test(test_recycle_bin)

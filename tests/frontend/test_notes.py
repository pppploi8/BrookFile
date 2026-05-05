import time
from test_utils import run_frontend_test, _ref


def _get_comp(agent):
    deadline = time.time() + 10
    while time.time() < deadline:
        comps = agent.find_components_by_name('Notes')
        if len(comps) > 0:
            return comps[0]
        time.sleep(0.5)
    assert False, 'Notes component not found'


def _navigate_to_notes(page, agent):
    page.evaluate('() => { const app = document.getElementById("app").__vue_app__; const router = app.config.globalProperties.$router; router.push("/notes"); }')
    time.sleep(1.0)
    agent.wait_ready(timeout=10000)
    comp = _get_comp(agent)
    return comp


def _wait_store(agent, store_name, timeout=10):
    deadline = time.time() + timeout
    while time.time() < deadline:
        store = agent.get_store(store_name)
        if store is not None:
            return store
        time.sleep(0.3)
    store = agent.get_store(store_name)
    if store is None:
        stores = agent.list_stores()
        raise AssertionError(f'Store "{store_name}" not found. Available: {stores}')
    return store


def _get_file_tree_via_page(page, notebook_id):
    return page.evaluate(f"""() => {{
        const app = document.getElementById('app').__vue_app__;
        const pinia = app.config.globalProperties.$pinia;
        const store = pinia._s.get('notebook');
        if (!store) return [];
        const tree = store.fileTreeMap.get('{notebook_id}');
        if (!tree) return [];
        return JSON.parse(JSON.stringify(tree));
    }}""")


def _sv(store, key):
    val = store.get(key)
    return _ref(val) if val is not None else None


def test_notes(page, agent, root_path, workdir):
    comp = _navigate_to_notes(page, agent)
    uid = comp['id']

    _wait_store(agent, 'notebook')
    nb_store = agent.get_store('notebook')
    assert isinstance(_sv(nb_store, 'notebooks'), list)
    assert len(_sv(nb_store, 'notebooks')) == 0

    # --- 1. Create notebook ---
    res = agent.dispatch_store('notebook', 'createNotebook', [{
        'name': 'TestNotes',
        'description': 'A test notebook',
        'path': 'testnotes',
    }])
    assert res['result']['success'] is True
    notebook_id = res['result']['id']
    time.sleep(1.0)

    nb_store = agent.get_store('notebook')
    notebooks = _sv(nb_store, 'notebooks')
    assert len(notebooks) == 1
    assert notebooks[0]['name'] == 'TestNotes'
    assert notebooks[0]['encrypted'] is False

    # --- 2. Create folder via API ---
    res = page.evaluate("""async ([nbId, path]) => {
        const r = await fetch('/api/notebook/create_folder', {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({ notebook_id: nbId, path: path }),
            credentials: 'include'
        });
        return await r.json();
    }""", [notebook_id, 'NotesFolder'])
    assert res['success'] is True
    time.sleep(0.5)

    # --- 3. Create note via API ---
    res = page.evaluate("""async ([nbId, path, content]) => {
        const r = await fetch('/api/notebook/save_note', {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({ notebook_id: nbId, path: path, content: content }),
            credentials: 'include'
        });
        return await r.json();
    }""", [notebook_id, 'MyFirstNote.md', '# Hello World\n\nThis is my first note.'])
    assert res['success'] is True
    time.sleep(0.5)

    # --- 4. Load file tree and verify ---
    agent.dispatch_store('notebook', 'fetchFileTree', [notebook_id])
    time.sleep(1.0)

    tree = _get_file_tree_via_page(page, notebook_id)
    assert len(tree) > 0

    note_names = [n['name'] for n in tree if not n.get('is_dir')]
    assert any('MyFirstNote' in name for name in note_names)

    folder_names = [n['name'] for n in tree if n.get('is_dir')]
    assert 'NotesFolder' in folder_names

    # --- 5. Create note inside folder via API ---
    res = page.evaluate("""async ([nbId, path, content]) => {
        const r = await fetch('/api/notebook/save_note', {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({ notebook_id: nbId, path: path, content: content }),
            credentials: 'include'
        });
        return await r.json();
    }""", [notebook_id, 'NotesFolder/FolderNote.md', 'Content inside folder'])
    assert res['success'] is True
    time.sleep(0.5)

    # --- 6. Open note via store + set currentNotebook on component ---
    nb_store = agent.get_store('notebook')
    notebooks = _sv(nb_store, 'notebooks')
    agent.set_ref(uid, 'currentNotebook', notebooks[0])
    time.sleep(0.3)

    agent.dispatch_store('note', 'openNote', [notebook_id, 'MyFirstNote.md', False, 'MyFirstNote'])
    time.sleep(1.0)

    note_store = agent.get_store('note')
    current_note = _sv(note_store, 'currentNote')
    assert current_note is not None
    assert current_note['notebookId'] == notebook_id
    assert 'Hello World' in current_note['content']
    assert current_note['isLoading'] is False
    assert current_note['isDirty'] is False

    # --- 7. Update note content via store ---
    agent.dispatch_store('note', 'updateContent', ['# Updated Content\n\nNew stuff here.'])
    time.sleep(0.3)

    note_store = agent.get_store('note')
    current_note = _sv(note_store, 'currentNote')
    assert 'Updated Content' in current_note['content']
    assert current_note['isDirty'] is True

    # --- 8. Save note via component ---
    agent.clear_messages()
    agent.call_method(uid, 'handleSaveNote')
    msg = agent.wait_for_message('success', timeout=15000)
    assert msg is not None
    assert msg['key'] == 'notes.saveSuccess'
    time.sleep(0.5)

    note_store = agent.get_store('note')
    current_note = _sv(note_store, 'currentNote')
    assert current_note['isDirty'] is False

    # --- 9. Rename note via store ---
    res = agent.dispatch_store('notebook', 'renameNotePath', [notebook_id, 'MyFirstNote.md', 'RenamedNote.md'])
    assert res['result']['success'] is True
    time.sleep(0.5)

    # Verify renamed note in tree
    agent.dispatch_store('notebook', 'fetchFileTree', [notebook_id])
    time.sleep(0.5)

    tree = _get_file_tree_via_page(page, notebook_id)
    note_names = [n['name'] for n in tree if not n.get('is_dir')]
    assert any('RenamedNote' in name for name in note_names)
    assert not any('MyFirstNote' in name for name in note_names)

    # --- 10. Search notes via component ---
    agent.set_ref(uid, 'searchKeyword', 'Updated Content')
    agent.call_method(uid, 'executeSearch')
    time.sleep(2.0)

    state = agent.get_component_state(uid)
    assert _ref(state['setupState']['isSearchActive']) is True
    search_tree_data = _ref(state['setupState']['searchTreeData'])
    assert len(search_tree_data) >= 1
    nb_result = search_tree_data[0]
    assert nb_result['notebookId'] == notebook_id
    assert len(nb_result['children']) >= 1

    # --- 11. Clear search ---
    agent.call_method(uid, 'clearSearch')
    time.sleep(0.3)
    state = agent.get_component_state(uid)
    assert _ref(state['setupState']['isSearchActive']) is False
    assert _ref(state['setupState']['searchKeyword']) == ''

    # --- 12. Edit notebook via store ---
    agent.dispatch_store('notebook', 'updateNotebookData', [notebook_id, 'UpdatedNotes', 'Updated description'])
    time.sleep(1.0)

    nb_store = agent.get_store('notebook')
    notebooks = _sv(nb_store, 'notebooks')
    assert notebooks[0]['name'] == 'UpdatedNotes'
    assert notebooks[0]['description'] == 'Updated description'

    # --- 13. Close note ---
    agent.dispatch_store('note', 'closeNote')
    time.sleep(0.3)

    note_store = agent.get_store('note')
    assert _sv(note_store, 'currentNote') is None

    # --- 14. Move note via store ---
    res = agent.dispatch_store('notebook', 'moveNotePath', [notebook_id, 'RenamedNote.md', 'NotesFolder'])
    assert res['result']['success'] is True
    time.sleep(0.5)

    tree = _get_file_tree_via_page(page, notebook_id)
    root_notes = [n for n in tree if not n.get('is_dir')]
    assert len(root_notes) == 0, 'Note should have been moved to folder'

    notes_folder = next((n for n in tree if n.get('is_dir') and n['name'] == 'NotesFolder'), None)
    assert notes_folder is not None
    folder_notes = notes_folder.get('children', [])
    assert any('RenamedNote' in n['name'] for n in folder_notes)

    # --- 15. Delete notes via API ---
    tree = _get_file_tree_via_page(page, notebook_id)
    all_paths = []

    def collect_paths(nodes):
        for node in nodes:
            if not node.get('is_dir'):
                all_paths.append(node['path'])
            if node.get('children'):
                collect_paths(node['children'])

    if tree:
        collect_paths(tree)

    if all_paths:
        page.evaluate("""async ([nbId, paths]) => {
            await fetch('/api/notebook/batch_delete', {
                method: 'POST',
                headers: {'Content-Type': 'application/json'},
                body: JSON.stringify({ notebook_id: nbId, paths: paths }),
                credentials: 'include'
            });
        }""", [notebook_id, all_paths])
    time.sleep(0.5)

    # --- 16. Delete folder via API ---
    page.evaluate("""async ([nbId, folderPath]) => {
        await fetch('/api/notebook/delete_folder', {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({ notebook_id: nbId, path: folderPath }),
            credentials: 'include'
        });
    }""", [notebook_id, 'NotesFolder'])
    time.sleep(0.5)

    # --- 17. Delete notebook via store ---
    res = agent.dispatch_store('notebook', 'removeNotebook', [notebook_id])
    assert res['result']['success'] is True
    time.sleep(0.5)

    nb_store = agent.get_store('notebook')
    notebooks = _sv(nb_store, 'notebooks')
    assert len(notebooks) == 0


if __name__ == '__main__':
    run_frontend_test(test_notes)

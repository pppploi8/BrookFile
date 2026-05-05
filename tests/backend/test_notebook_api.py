import os
import time
import json
import requests
from test_utils import run_tests, BASE_URL


def test_notebook_api(session, root_path):
    # --- list: empty ---
    resp = session.post(f'{BASE_URL}/api/notebook/list')
    data = resp.json()
    assert data['notebooks'] == []

    # --- create: success ---
    os.makedirs(os.path.join(root_path, 'notes'))
    resp = session.post(f'{BASE_URL}/api/notebook/create', json={
        'name': 'WorkNotes',
        'description': 'work stuff',
        'path': 'notes',
        'encrypted': False
    })
    data = resp.json()
    assert data['success'] is True
    nb_id = data['id']

    # --- create: duplicate path ---
    resp = session.post(f'{BASE_URL}/api/notebook/create', json={
        'name': 'Another',
        'path': 'notes',
        'encrypted': False
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'DUPLICATE_NOTEBOOK_PATH'

    # --- create: path not empty ---
    os.makedirs(os.path.join(root_path, 'nonempty'))
    with open(os.path.join(root_path, 'nonempty', 'file.md'), 'w') as f:
        f.write('x')
    resp = session.post(f'{BASE_URL}/api/notebook/create', json={
        'name': 'NonEmpty',
        'path': 'nonempty',
        'encrypted': False
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PATH_NOT_EMPTY'

    # --- list: has one ---
    resp = session.post(f'{BASE_URL}/api/notebook/list')
    data = resp.json()
    assert len(data['notebooks']) == 1
    assert data['notebooks'][0]['name'] == 'WorkNotes'
    assert data['notebooks'][0]['path'] == 'notes'
    assert data['notebooks'][0]['encrypted'] is False

    # --- file_tree: empty ---
    resp = session.post(f'{BASE_URL}/api/notebook/file_tree', json={'notebook_id': nb_id})
    data = resp.json()
    assert data['tree'] == []

    # --- create_folder ---
    resp = session.post(f'{BASE_URL}/api/notebook/create_folder', json={
        'notebook_id': nb_id,
        'path': 'folder1'
    })
    assert resp.json()['success'] is True
    assert os.path.isdir(os.path.join(root_path, 'notes', 'folder1'))

    # --- create_folder: already exists ---
    resp = session.post(f'{BASE_URL}/api/notebook/create_folder', json={
        'notebook_id': nb_id,
        'path': 'folder1'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'FOLDER_ALREADY_EXISTS'

    # --- save_note: new (no hash) ---
    resp = session.post(f'{BASE_URL}/api/notebook/save_note', json={
        'notebook_id': nb_id,
        'path': 'note1.md',
        'content': '# Hello\nThis is note 1.'
    })
    data = resp.json()
    assert data['success'] is True
    hash1 = data['hash']

    # --- save_note: duplicate without hash → FILE_ALREADY_EXISTS ---
    resp = session.post(f'{BASE_URL}/api/notebook/save_note', json={
        'notebook_id': nb_id,
        'path': 'note1.md',
        'content': 'duplicate'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'FILE_ALREADY_EXISTS'

    # --- read_note ---
    resp = session.post(f'{BASE_URL}/api/notebook/read_note', json={
        'notebook_id': nb_id,
        'path': 'note1.md'
    })
    data = resp.json()
    assert data['success'] is True
    assert data['content'] == '# Hello\nThis is note 1.'
    assert data['hash'] == hash1

    # --- read_note: not found ---
    resp = session.post(f'{BASE_URL}/api/notebook/read_note', json={
        'notebook_id': nb_id,
        'path': 'nonexist.md'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'FILE_NOT_FOUND'

    # --- save_note: update with hash ---
    resp = session.post(f'{BASE_URL}/api/notebook/save_note', json={
        'notebook_id': nb_id,
        'path': 'note1.md',
        'content': '# Updated\nNew content.',
        'hash': hash1
    })
    data = resp.json()
    assert data['success'] is True
    hash2 = data['hash']
    assert hash2 != hash1

    # --- save_note: conflict ---
    resp = session.post(f'{BASE_URL}/api/notebook/save_note', json={
        'notebook_id': nb_id,
        'path': 'note1.md',
        'content': 'stale update',
        'hash': hash1
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'CONFLICT_DETECTED'
    assert data['server_content'] == '# Updated\nNew content.'
    assert data['server_hash'] == hash2

    # --- save_conflict ---
    resp = session.post(f'{BASE_URL}/api/notebook/save_conflict', json={
        'notebook_id': nb_id,
        'path': 'note1.md',
        'content': 'conflicting version'
    })
    data = resp.json()
    assert data['success'] is True
    assert 'conflict_path' in data
    assert 'conflict' in data['conflict_path']

    # --- save_note in subfolder ---
    resp = session.post(f'{BASE_URL}/api/notebook/save_note', json={
        'notebook_id': nb_id,
        'path': 'folder1/note2.md',
        'content': 'Subfolder note.'
    })
    assert resp.json()['success'] is True

    # --- file_tree ---
    resp = session.post(f'{BASE_URL}/api/notebook/file_tree', json={'notebook_id': nb_id})
    data = resp.json()
    assert len(data['tree']) >= 2
    folder_node = next(n for n in data['tree'] if n['is_dir'])
    file_node = next(n for n in data['tree'] if not n['is_dir'])
    assert folder_node['name'] == 'folder1'
    assert file_node['name'] == 'note1.md'
    assert len(folder_node['children']) == 1
    assert folder_node['children'][0]['name'] == 'note2.md'

    # --- rename ---
    resp = session.post(f'{BASE_URL}/api/notebook/rename', json={
        'notebook_id': nb_id,
        'old_path': 'note1.md',
        'new_name': 'renamed.md'
    })
    data = resp.json()
    assert data['success'] is True
    assert data['new_path'] == 'renamed.md'

    # --- move ---
    resp = session.post(f'{BASE_URL}/api/notebook/move', json={
        'notebook_id': nb_id,
        'source_path': 'renamed.md',
        'target_folder': 'folder1'
    })
    data = resp.json()
    assert data['success'] is True
    assert data['new_path'] == 'folder1/renamed.md'

    # --- rename: already exists ---
    resp = session.post(f'{BASE_URL}/api/notebook/rename', json={
        'notebook_id': nb_id,
        'old_path': 'folder1/note2.md',
        'new_name': 'renamed.md'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'FILE_ALREADY_EXISTS'

    # --- search ---
    time.sleep(1)
    resp = session.post(f'{BASE_URL}/api/notebook/search', json={
        'keyword': 'Subfolder',
        'notebook_id': nb_id
    })
    data = resp.json()
    assert data['success'] is True
    assert len(data['results']) >= 1
    result = data['results'][0]
    assert result['note_path'] == 'folder1/note2.md'
    assert result['match_count'] >= 1

    # --- search: across all notebooks ---
    resp = session.post(f'{BASE_URL}/api/notebook/search', json={
        'keyword': 'Subfolder'
    })
    data = resp.json()
    assert data['success'] is True
    assert len(data['results']) >= 1

    # --- batch_delete ---
    resp = session.post(f'{BASE_URL}/api/notebook/batch_delete', json={
        'notebook_id': nb_id,
        'paths': ['folder1/note2.md', 'folder1/renamed.md']
    })
    assert resp.json()['success'] is True
    assert not os.path.exists(os.path.join(root_path, 'notes', 'folder1', 'note2.md'))
    assert not os.path.exists(os.path.join(root_path, 'notes', 'folder1', 'renamed.md'))

    # --- delete_folder: empty ---
    resp = session.post(f'{BASE_URL}/api/notebook/delete_folder', json={
        'notebook_id': nb_id,
        'path': 'folder1'
    })
    assert resp.json()['success'] is True
    assert not os.path.exists(os.path.join(root_path, 'notes', 'folder1'))

    # --- delete_folder: not empty ---
    os.makedirs(os.path.join(root_path, 'notes', 'notempty'))
    with open(os.path.join(root_path, 'notes', 'notempty', 'x.md'), 'w') as f:
        f.write('x')
    resp = session.post(f'{BASE_URL}/api/notebook/delete_folder', json={
        'notebook_id': nb_id,
        'path': 'notempty'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'FOLDER_NOT_EMPTY'

    # cleanup notempty
    os.remove(os.path.join(root_path, 'notes', 'notempty', 'x.md'))
    os.rmdir(os.path.join(root_path, 'notes', 'notempty'))

    # --- update notebook ---
    resp = session.post(f'{BASE_URL}/api/notebook/update', json={
        'id': nb_id,
        'name': 'PersonalNotes',
        'description': 'personal'
    })
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/notebook/list')
    assert resp.json()['notebooks'][0]['name'] == 'PersonalNotes'

    # --- update: not found ---
    resp = session.post(f'{BASE_URL}/api/notebook/update', json={
        'id': 'nonexistent',
        'name': 'x'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOTEBOOK_NOT_FOUND'

    # --- delete notebook ---
    resp = session.post(f'{BASE_URL}/api/notebook/delete', json={'id': nb_id})
    assert resp.json()['success'] is True

    # --- delete: not found ---
    resp = session.post(f'{BASE_URL}/api/notebook/delete', json={'id': nb_id})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOTEBOOK_NOT_FOUND'

    # --- open: from existing directory ---
    os.makedirs(os.path.join(root_path, 'existing'))
    with open(os.path.join(root_path, 'existing', 'readme.md'), 'w') as f:
        f.write('existing content')
    resp = session.post(f'{BASE_URL}/api/notebook/open', json={
        'name': 'Existing',
        'description': 'from dir',
        'path': 'existing',
        'encrypted': False
    })
    data = resp.json()
    assert data['success'] is True
    open_id = data['id']

    # verify file_tree has content
    resp = session.post(f'{BASE_URL}/api/notebook/file_tree', json={'notebook_id': open_id})
    data = resp.json()
    assert len(data['tree']) == 1
    assert data['tree'][0]['name'] == 'readme.md'

    # cleanup
    session.post(f'{BASE_URL}/api/notebook/delete', json={'id': open_id})

    # --- notebook not found for operations ---
    for endpoint, payload in [
        ('file_tree', {'notebook_id': 'nonexistent'}),
        ('create_folder', {'notebook_id': 'nonexistent', 'path': 'x'}),
        ('save_note', {'notebook_id': 'nonexistent', 'path': 'x.md', 'content': 'x'}),
        ('read_note', {'notebook_id': 'nonexistent', 'path': 'x.md'}),
        ('rename', {'notebook_id': 'nonexistent', 'old_path': 'x.md', 'new_name': 'y.md'}),
        ('move', {'notebook_id': 'nonexistent', 'source_path': 'x.md', 'target_folder': ''}),
        ('delete_folder', {'notebook_id': 'nonexistent', 'path': 'x'}),
        ('batch_delete', {'notebook_id': 'nonexistent', 'paths': ['x.md']}),
    ]:
        resp = session.post(f'{BASE_URL}/api/notebook/{endpoint}', json=payload)
        data = resp.json()
        assert data['success'] is False
        assert data['fail_code'] == 'NOTEBOOK_NOT_FOUND'

    # --- import notebook rebuilds search index ---
    imported_dir = os.path.join(root_path, 'imported_notes')
    os.makedirs(imported_dir)
    with open(os.path.join(imported_dir, 'note1.md'), 'w', encoding='utf-8') as f:
        f.write('# Hello World\nThis is a test note about machine learning.\n')
    with open(os.path.join(imported_dir, 'note2.md'), 'w', encoding='utf-8') as f:
        f.write('# Another note\nDeep learning is a subset of machine learning.\n')
    resp = session.post(f'{BASE_URL}/api/notebook/open', json={
        'name': 'ImportedNotes',
        'description': 'test imported notebook',
        'path': 'imported_notes',
        'encrypted': False
    })
    data = resp.json()
    assert data['success'] is True
    time.sleep(2)
    resp = session.post(f'{BASE_URL}/api/notebook/search', json={'keyword': 'machine learning'})
    paths = [r['note_path'] for r in resp.json()['results']]
    assert 'note1.md' in paths
    assert 'note2.md' in paths
    resp = session.post(f'{BASE_URL}/api/notebook/search', json={'keyword': 'deep learning'})
    paths2 = [r['note_path'] for r in resp.json()['results']]
    assert 'note2.md' in paths2
    session.post(f'{BASE_URL}/api/notebook/delete', json={'id': data['id']})

    # --- import notebook with subfolders ---
    folder_dir = os.path.join(root_path, 'folder_notes')
    sub_dir = os.path.join(folder_dir, 'sub')
    os.makedirs(sub_dir)
    with open(os.path.join(folder_dir, 'root_note.md'), 'w', encoding='utf-8') as f:
        f.write('Quantum computing basics\n')
    with open(os.path.join(sub_dir, 'sub_note.md'), 'w', encoding='utf-8') as f:
        f.write('Advanced quantum algorithms\n')
    resp = session.post(f'{BASE_URL}/api/notebook/open', json={
        'name': 'FolderNotes',
        'description': '',
        'path': 'folder_notes',
        'encrypted': False
    })
    data = resp.json()
    assert data['success'] is True
    time.sleep(2)
    resp = session.post(f'{BASE_URL}/api/notebook/search', json={'keyword': 'quantum'})
    results = resp.json()['results']
    assert len(results) >= 2
    paths = [r['note_path'] for r in results]
    assert 'root_note.md' in paths
    assert any('sub_note.md' in p for p in paths)
    session.post(f'{BASE_URL}/api/notebook/delete', json={'id': data['id']})

    # --- rename note updates index ---
    os.makedirs(os.path.join(root_path, 'rename_nb'))
    resp = session.post(f'{BASE_URL}/api/notebook/create', json={
        'name': 'RenameTest',
        'description': '',
        'path': 'rename_nb',
        'encrypted': False
    })
    data = resp.json()
    assert data['success'] is True
    rename_nb_id = data['id']
    resp = session.post(f'{BASE_URL}/api/notebook/save_note', json={
        'notebook_id': rename_nb_id,
        'path': 'original_note.md',
        'content': 'UniqueContentAlphaBetaGamma\n',
    })
    assert resp.json()['success'] is True
    time.sleep(2)
    resp = session.post(f'{BASE_URL}/api/notebook/search', json={'keyword': 'UniqueContentAlphaBetaGamma'})
    results = resp.json()['results']
    assert len(results) == 1
    assert results[0]['note_path'] == 'original_note.md'
    resp = session.post(f'{BASE_URL}/api/notebook/rename', json={
        'notebook_id': rename_nb_id,
        'old_path': 'original_note.md',
        'new_name': 'renamed_note.md',
    })
    assert resp.json()['success'] is True
    time.sleep(2)
    resp = session.post(f'{BASE_URL}/api/notebook/search', json={'keyword': 'UniqueContentAlphaBetaGamma'})
    results = resp.json()['results']
    assert len(results) == 1
    assert results[0]['note_path'] == 'renamed_note.md'
    session.post(f'{BASE_URL}/api/notebook/delete', json={'id': rename_nb_id})

    # --- move note updates index ---
    os.makedirs(os.path.join(root_path, 'move_nb'))
    resp = session.post(f'{BASE_URL}/api/notebook/create', json={
        'name': 'MoveTest',
        'description': '',
        'path': 'move_nb',
        'encrypted': False
    })
    data = resp.json()
    assert data['success'] is True
    move_nb_id = data['id']
    resp = session.post(f'{BASE_URL}/api/notebook/create_folder', json={
        'notebook_id': move_nb_id,
        'path': 'source_dir',
    })
    assert resp.json()['success'] is True
    resp = session.post(f'{BASE_URL}/api/notebook/create_folder', json={
        'notebook_id': move_nb_id,
        'path': 'target_dir',
    })
    assert resp.json()['success'] is True
    resp = session.post(f'{BASE_URL}/api/notebook/save_note', json={
        'notebook_id': move_nb_id,
        'path': 'source_dir/movable_note.md',
        'content': 'MoveTestContentGolfHotelIndia\n',
    })
    assert resp.json()['success'] is True
    time.sleep(2)
    resp = session.post(f'{BASE_URL}/api/notebook/search', json={'keyword': 'MoveTestContentGolfHotelIndia'})
    results = resp.json()['results']
    assert len(results) == 1
    assert results[0]['note_path'] == 'source_dir/movable_note.md'
    resp = session.post(f'{BASE_URL}/api/notebook/move', json={
        'notebook_id': move_nb_id,
        'source_path': 'source_dir/movable_note.md',
        'target_folder': 'target_dir',
    })
    assert resp.json()['success'] is True
    time.sleep(2)
    resp = session.post(f'{BASE_URL}/api/notebook/search', json={'keyword': 'MoveTestContentGolfHotelIndia'})
    results = resp.json()['results']
    assert len(results) == 1
    assert results[0]['note_path'] == 'target_dir/movable_note.md'
    session.post(f'{BASE_URL}/api/notebook/delete', json={'id': move_nb_id})

    # --- search match format ---
    os.makedirs(os.path.join(root_path, 'match_fmt_nb'))
    resp = session.post(f'{BASE_URL}/api/notebook/create', json={
        'name': 'MatchFormatTest',
        'description': '',
        'path': 'match_fmt_nb',
        'encrypted': False
    })
    data = resp.json()
    assert data['success'] is True
    fmt_nb_id = data['id']
    resp = session.post(f'{BASE_URL}/api/notebook/save_note', json={
        'notebook_id': fmt_nb_id,
        'path': 'fmt_note.md',
        'content': 'ZUniquePrefixX abc test123 def test456 ghi\nanother line ZUniquePrefixX test789\n',
    })
    assert resp.json()['success'] is True
    time.sleep(2)
    resp = session.post(f'{BASE_URL}/api/notebook/search', json={'keyword': 'ZUniquePrefixX'})
    results = resp.json()['results']
    assert len(results) >= 1
    r = results[0]
    assert r['note_path'] == 'fmt_note.md'
    assert len(r['matches']) >= 1
    m = r['matches'][0]
    assert 'line_number' in m
    assert 'content' in m
    assert '<match>' in m['content']
    assert '</match>' in m['content']
    assert 'before' not in m
    assert 'match_text' not in m
    session.post(f'{BASE_URL}/api/notebook/delete', json={'id': fmt_nb_id})

    # --- create encrypted notebook ---
    sig_content = json.dumps({
        'salt': 'dGVzdHNhbHQ=',
        'iv': 'dGVzdGl2MTIzNDU2',
        'rounds': 100000,
        'signature': 'dGVzdHNpZ25hdHVyZQ==',
    })
    resp = session.post(f'{BASE_URL}/api/notebook/create', json={
        'name': 'EncryptedNB',
        'description': '',
        'path': 'enc_nb',
        'encrypted': True,
        'signature': sig_content,
    })
    data = resp.json()
    assert data['success'] is True
    enc_nb_id = data['id']
    resp = session.post(f'{BASE_URL}/api/notebook/list')
    enc_nb = next((n for n in resp.json()['notebooks'] if n['name'] == 'EncryptedNB'), None)
    assert enc_nb is not None
    assert enc_nb['encrypted'] is True
    sig_path = os.path.join(root_path, 'enc_nb', '.notebook.sig')
    assert os.path.exists(sig_path)
    with open(sig_path, 'r') as f:
        saved_sig = json.loads(f.read())
    assert saved_sig['salt'] == 'dGVzdHNhbHQ='
    assert saved_sig['rounds'] == 100000

    # --- encrypted without signature rejected ---
    resp = session.post(f'{BASE_URL}/api/notebook/create', json={
        'name': 'EncNoSig',
        'description': '',
        'path': 'enc_no_sig',
        'encrypted': True,
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'INVALID_PARAM'

    # --- encrypted invalid signature rejected ---
    resp = session.post(f'{BASE_URL}/api/notebook/create', json={
        'name': 'EncBadSig',
        'description': '',
        'path': 'enc_bad_sig',
        'encrypted': True,
        'signature': 'not-valid-json',
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'INVALID_PARAM'
    resp = session.post(f'{BASE_URL}/api/notebook/create', json={
        'name': 'EncBadSig2',
        'description': '',
        'path': 'enc_bad_sig2',
        'encrypted': True,
        'signature': json.dumps({'salt': 'abc'}),
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'INVALID_PARAM'

    # --- encrypted nested rejected ---
    resp = session.post(f'{BASE_URL}/api/notebook/create', json={
        'name': 'ChildEnc',
        'description': '',
        'path': 'enc_nb/child_enc',
        'encrypted': True,
        'signature': sig_content,
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NESTED_ENCRYPTED_NOT_ALLOWED'

    # --- encrypted on non-empty path rejected ---
    non_empty_enc_dir = os.path.join(root_path, 'non_empty_enc')
    os.makedirs(non_empty_enc_dir)
    with open(os.path.join(non_empty_enc_dir, 'existing.md'), 'w') as f:
        f.write('some content')
    resp = session.post(f'{BASE_URL}/api/notebook/create', json={
        'name': 'NonEmptyEnc',
        'description': '',
        'path': 'non_empty_enc',
        'encrypted': True,
        'signature': sig_content,
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PATH_NOT_EMPTY'

    # --- encrypted notebook no search index ---
    resp = session.post(f'{BASE_URL}/api/notebook/create', json={
        'name': 'EncNoIndex',
        'description': '',
        'path': 'enc_no_index',
        'encrypted': True,
        'signature': sig_content,
    })
    data = resp.json()
    assert data['success'] is True
    time.sleep(2)
    resp = session.post(f'{BASE_URL}/api/notebook/search', json={'keyword': 'anything'})
    enc_results = [r for r in resp.json()['results'] if r['notebook_name'] == 'EncNoIndex']
    assert len(enc_results) == 0

    # --- BUG9: notebook/create should return PATH_NOT_FOUND when path is a file ---
    with open(os.path.join(root_path, 'file_as_path.txt'), 'w') as f:
        f.write('this is a file, not a directory')
    resp = session.post(f'{BASE_URL}/api/notebook/create', json={
        'name': 'FileAsPath',
        'path': 'file_as_path.txt',
        'encrypted': False
    })
    data = resp.json()
    assert data['success'] is False, \
        'BUG9: create notebook on a file path should fail'
    assert data['fail_code'] == 'PATH_NOT_FOUND', \
        f'BUG9: Expected PATH_NOT_FOUND for file path, got {data.get("fail_code")}'

    # --- BUG10: notebook/update should return INVALID_PARAM for empty name ---
    os.makedirs(os.path.join(root_path, 'bug10_nb'))
    resp = session.post(f'{BASE_URL}/api/notebook/create', json={
        'name': 'Bug10Test',
        'path': 'bug10_nb',
        'encrypted': False
    })
    bug10_id = resp.json()['id']
    resp = session.post(f'{BASE_URL}/api/notebook/update', json={
        'id': bug10_id,
        'name': ''
    })
    data = resp.json()
    assert data['success'] is False, \
        'BUG10: update notebook with empty name should fail'
    assert data['fail_code'] == 'INVALID_PARAM', \
        f'BUG10: Expected INVALID_PARAM for empty name, got {data.get("fail_code")}'
    session.post(f'{BASE_URL}/api/notebook/delete', json={'id': bug10_id})

    # --- BUG: /api/notebook/attachment should work without login (token-based auth) ---
    os.makedirs(os.path.join(root_path, 'bug_att_nb'), exist_ok=True)
    resp = session.post(f'{BASE_URL}/api/notebook/create', json={
        'name': 'BugAttTest',
        'path': 'bug_att_nb',
        'encrypted': False
    })
    bug_att_id = resp.json()['id']

    att_dir = os.path.join(root_path, 'bug_att_nb', 'attachment')
    os.makedirs(att_dir, exist_ok=True)
    with open(os.path.join(att_dir, 'test.txt'), 'wb') as f:
        f.write(b'attachment content for bug test')

    resp = session.post(f'{BASE_URL}/api/notebook/attachment_token', json={
        'notebook_id': bug_att_id
    })
    data = resp.json()
    assert data['success'] is True
    att_token = data['token']

    no_auth = requests.Session()
    resp = no_auth.get(f'{BASE_URL}/api/notebook/attachment', params={
        'notebook_id': bug_att_id,
        'path': 'attachment/test.txt',
        'token': att_token
    })
    assert resp.status_code == 200, \
        f'BUG: attachment should be accessible without login via token, got status {resp.status_code}'
    assert resp.content == b'attachment content for bug test'

    resp = no_auth.get(f'{BASE_URL}/api/notebook/attachment', params={
        'notebook_id': bug_att_id,
        'path': 'attachment/test.txt',
        'token': 'invalid_token'
    })
    assert resp.json()['success'] is False

    session.post(f'{BASE_URL}/api/notebook/delete', json={'id': bug_att_id})

    # --- attachment upload: non-encrypted keeps original filename ---
    os.makedirs(os.path.join(root_path, 'att_upload_nb'), exist_ok=True)
    resp = session.post(f'{BASE_URL}/api/notebook/create', json={
        'name': 'AttUploadTest',
        'path': 'att_upload_nb',
        'encrypted': False
    })
    att_up_id = resp.json()['id']

    resp = session.post(f'{BASE_URL}/api/notebook/upload_attachment', files={
        'notebook_id': (None, att_up_id),
        'path': (None, 'photo.jpg'),
        'file': ('photo.jpg', b'\xff\xd8\xff\xe0fake_jpg_data', 'image/jpeg'),
    })
    data = resp.json()
    assert data['success'] is True
    assert data['path'] == 'photo.jpg'
    assert os.path.isfile(os.path.join(root_path, 'att_upload_nb', 'attachment', 'photo.jpg'))

    # --- attachment upload: non-encrypted dedup (Windows-style) ---
    resp = session.post(f'{BASE_URL}/api/notebook/upload_attachment', files={
        'notebook_id': (None, att_up_id),
        'path': (None, 'photo.jpg'),
        'file': ('photo.jpg', b'\xff\xd8\xff\xe0another_jpg', 'image/jpeg'),
    })
    data = resp.json()
    assert data['success'] is True
    assert data['path'] == 'photo(1).jpg'
    assert os.path.isfile(os.path.join(root_path, 'att_upload_nb', 'attachment', 'photo(1).jpg'))

    # --- attachment upload: non-encrypted dedup increments ---
    resp = session.post(f'{BASE_URL}/api/notebook/upload_attachment', files={
        'notebook_id': (None, att_up_id),
        'path': (None, 'photo.jpg'),
        'file': ('photo.jpg', b'\xff\xd8\xff\xe0third_jpg', 'image/jpeg'),
    })
    data = resp.json()
    assert data['success'] is True
    assert data['path'] == 'photo(2).jpg'
    assert os.path.isfile(os.path.join(root_path, 'att_upload_nb', 'attachment', 'photo(2).jpg'))

    # --- attachment upload: non-encrypted different name no dedup ---
    resp = session.post(f'{BASE_URL}/api/notebook/upload_attachment', files={
        'notebook_id': (None, att_up_id),
        'path': (None, 'document.pdf'),
        'file': ('document.pdf', b'%PDF-1.4 fake pdf', 'application/pdf'),
    })
    data = resp.json()
    assert data['success'] is True
    assert data['path'] == 'document.pdf'
    assert os.path.isfile(os.path.join(root_path, 'att_upload_nb', 'attachment', 'document.pdf'))

    # --- attachment download: non-encrypted has Content-Disposition ---
    resp_tok = session.post(f'{BASE_URL}/api/notebook/attachment_token', json={
        'notebook_id': att_up_id
    })
    att_token = resp_tok.json()['token']
    resp = requests.get(f'{BASE_URL}/api/notebook/attachment', params={
        'notebook_id': att_up_id,
        'path': 'attachment/photo.jpg',
        'token': att_token,
    })
    assert resp.status_code == 200
    assert 'Content-Disposition' in resp.headers
    assert 'photo.jpg' in resp.headers['Content-Disposition']

    # --- attachment download: non-encrypted dedup file also has Content-Disposition ---
    resp = requests.get(f'{BASE_URL}/api/notebook/attachment', params={
        'notebook_id': att_up_id,
        'path': 'attachment/photo(1).jpg',
        'token': att_token,
    })
    assert resp.status_code == 200
    assert 'Content-Disposition' in resp.headers
    assert 'photo(1).jpg' in resp.headers['Content-Disposition']

    # --- attachment upload: non-encrypted no extension dedup ---
    resp = session.post(f'{BASE_URL}/api/notebook/upload_attachment', files={
        'notebook_id': (None, att_up_id),
        'path': (None, 'README'),
        'file': ('README', b'readme content', 'text/plain'),
    })
    data = resp.json()
    assert data['success'] is True
    assert data['path'] == 'README'

    resp = session.post(f'{BASE_URL}/api/notebook/upload_attachment', files={
        'notebook_id': (None, att_up_id),
        'path': (None, 'README'),
        'file': ('README', b'readme content 2', 'text/plain'),
    })
    data = resp.json()
    assert data['success'] is True
    assert data['path'] == 'README(1)'

    session.post(f'{BASE_URL}/api/notebook/delete', json={'id': att_up_id})


if __name__ == '__main__':
    run_tests(test_notebook_api)

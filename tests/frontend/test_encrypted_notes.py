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


def _sv(store, key):
    val = store.get(key)
    return _ref(val) if val is not None else None


def test_encrypted_notes(page, agent, root_path, workdir):
    comp = _navigate_to_notes(page, agent)
    uid = comp['id']

    _wait_store(agent, 'notebook')
    _wait_store(agent, 'crypto')

    password = 'test_encrypt_pass_123'

    # --- 1. Create encrypted notebook via store ---
    res = page.evaluate("""async ([name, path, encrypted, password]) => {
        const app = document.getElementById('app').__vue_app__;
        const pinia = app.config.globalProperties.$pinia;
        const cryptoStore = pinia._s.get('crypto');
        const nbStore = pinia._s.get('notebook');

        const sigContent = await cryptoStore.generateSignatureFile(password);
        const signature = JSON.stringify(sigContent);

        const createRes = await nbStore.createNotebook({
            name: name,
            description: 'encrypted test',
            path: path,
            encrypted: true,
            signature: signature,
        });

        if (createRes.success && createRes.id) {
            const unlocked = await cryptoStore.unlockNotebook(createRes.id, password, signature);
            return { success: true, id: createRes.id, unlocked: unlocked, signature: signature };
        }
        return { success: false, fail_code: createRes.fail_code };
    }""", ['EncNotebook', 'encnotes', True, password])
    assert res['success'] is True, f'Create failed: {res}'
    notebook_id = res['id']
    assert res['unlocked'] is True, 'Notebook should be unlocked after creation'
    time.sleep(1.0)

    # --- 2. Create a note via API (encrypted) ---
    encrypted_content = page.evaluate("""async ([nbId, path, content]) => {
        const app = document.getElementById('app').__vue_app__;
        const pinia = app.config.globalProperties.$pinia;
        const cryptoStore = pinia._s.get('crypto');

        const encrypted = await cryptoStore.encryptContent(nbId, path, content);
        const r = await fetch('/api/notebook/save_note', {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({ notebook_id: nbId, path: path, content: encrypted }),
            credentials: 'include'
        });
        return await r.json();
    }""", [notebook_id, 'SecretNote.md', '# Secret Content\n\nThis is encrypted.'])
    assert encrypted_content['success'] is True, f'Save note failed: {encrypted_content}'
    time.sleep(0.5)

    # --- 3. Read and decrypt the note (verify it works while unlocked) ---
    read_result = page.evaluate("""async ([nbId, path]) => {
        const app = document.getElementById('app').__vue_app__;
        const pinia = app.config.globalProperties.$pinia;
        const cryptoStore = pinia._s.get('crypto');

        const r = await fetch('/api/notebook/read_note', {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({ notebook_id: nbId, path: path }),
            credentials: 'include'
        });
        const res = await r.json();
        if (!res.success) return { success: false, fail_code: res.fail_code };
        const decrypted = await cryptoStore.decryptContent(nbId, path, res.content);
        return { success: true, content: decrypted };
    }""", [notebook_id, 'SecretNote.md'])
    assert read_result['success'] is True, f'Read note failed: {read_result}'
    assert 'Secret Content' in read_result['content'], f'Content mismatch: {read_result["content"]}'
    print('  Step 3: Read+decrypt while unlocked OK')

    # --- 4. Lock the notebook (simulate closing it) ---
    page.evaluate("""(nbId) => {
        const app = document.getElementById('app').__vue_app__;
        const pinia = app.config.globalProperties.$pinia;
        const cryptoStore = pinia._s.get('crypto');
        cryptoStore.lockNotebook(nbId);
    }""", notebook_id)
    time.sleep(0.3)

    is_unlocked = page.evaluate("""(nbId) => {
        const app = document.getElementById('app').__vue_app__;
        const pinia = app.config.globalProperties.$pinia;
        const cryptoStore = pinia._s.get('crypto');
        return cryptoStore.isUnlocked(nbId);
    }""", notebook_id)
    assert is_unlocked is False, 'Notebook should be locked after lockNotebook'
    print('  Step 4: Lock notebook OK')

    # --- 5. Try to unlock again by reading .notebook.sig (this is what the UI does) ---
    unlock_result = page.evaluate("""async ([nbId, pwd]) => {
        const app = document.getElementById('app').__vue_app__;
        const pinia = app.config.globalProperties.$pinia;
        const cryptoStore = pinia._s.get('crypto');

        // This is what Notes.vue submitUnlock does:
        // 1. Read .notebook.sig via readNote API
        const r = await fetch('/api/notebook/read_note', {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({ notebook_id: nbId, path: '.notebook.sig' }),
            credentials: 'include'
        });
        const sigRes = await r.json();

        if (!sigRes.success) {
            return { success: false, step: 'read_sig', fail_code: sigRes.fail_code };
        }

        // 2. Unlock with password
        const valid = await cryptoStore.unlockNotebook(nbId, pwd, sigRes.content);
        return { success: true, unlocked: valid };
    }""", [notebook_id, password])

    if not unlock_result.get('success'):
        print(f'  Step 5: REPRODUCED BUG - read .notebook.sig failed with: {unlock_result["fail_code"]}')
        print('  BUG: Backend blocks reading .notebook.sig via read_note API (INVALID_FILE_PATH)')
        print('  This prevents unlocking an encrypted notebook after closing it')
        assert False, f'BUG REPRODUCED: Cannot read .notebook.sig, got fail_code={unlock_result["fail_code"]}'
    else:
        print(f'  Step 5: Unlock result: {unlock_result}')
        assert unlock_result.get('unlocked') is True, 'Unlock should succeed with correct password'

        # Verify decryption works after re-unlock
        read_result2 = page.evaluate("""async ([nbId, path]) => {
            const app = document.getElementById('app').__vue_app__;
            const pinia = app.config.globalProperties.$pinia;
            const cryptoStore = pinia._s.get('crypto');

            const r = await fetch('/api/notebook/read_note', {
                method: 'POST',
                headers: {'Content-Type': 'application/json'},
                body: JSON.stringify({ notebook_id: nbId, path: path }),
                credentials: 'include'
            });
            const res = await r.json();
            if (!res.success) return { success: false };
            const decrypted = await cryptoStore.decryptContent(nbId, path, res.content);
            return { success: true, content: decrypted };
        }""", [notebook_id, 'SecretNote.md'])
        assert read_result2['success'] is True
        assert 'Secret Content' in read_result2['content']

    # --- Cleanup: delete notebook ---
    agent.dispatch_store('notebook', 'removeNotebook', [notebook_id])
    time.sleep(0.5)


if __name__ == '__main__':
    run_frontend_test(test_encrypted_notes)

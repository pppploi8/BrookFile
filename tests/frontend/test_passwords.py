import os
import time
from test_utils import run_frontend_test, _ref


def _get_comp(agent):
    comps = agent.find_components_by_name('Passwords')
    assert len(comps) > 0, 'Passwords component not found'
    return comps[0]


def _wait_vaults_loaded(agent, uid, timeout=10):
    deadline = time.time() + timeout
    while time.time() < deadline:
        state = agent.get_component_state(uid)
        vaults = _ref(state['setupState']['vaults'])
        if vaults is not None:
            return state
        time.sleep(0.3)
    return agent.get_component_state(uid)


def _navigate_to_passwords(page, agent):
    page.evaluate('() => { const app = document.getElementById("app").__vue_app__; const router = app.config.globalProperties.$router; router.push("/passwords"); }')
    time.sleep(1.5)
    agent.wait_ready(timeout=10000)
    comp = _get_comp(agent)
    state = _wait_vaults_loaded(agent, comp['id'])
    return comp, state


def test_passwords(page, agent, root_path, workdir):
    comp, state = _navigate_to_passwords(page, agent)
    uid = comp['id']

    vaults = _ref(state['setupState']['vaults'])
    assert isinstance(vaults, list)
    assert len(vaults) == 0

    # --- 1. Create vault ---
    # Open create dialog via context menu simulation
    agent.set_reactive_field(uid, 'contextMenu', 'isRoot', True)
    agent.set_reactive_field(uid, 'contextMenu', 'visible', True)
    time.sleep(0.3)
    agent.call_method(uid, 'handleCreateVault')
    time.sleep(0.5)

    state = agent.get_component_state(uid)
    assert _ref(state['setupState']['createVaultVisible']) is True

    agent.set_reactive_field(uid, 'createVaultForm', 'name', 'TestVault')
    agent.set_reactive_field(uid, 'createVaultForm', 'description', 'A test vault')
    agent.set_reactive_field(uid, 'createVaultForm', 'path', '/')
    agent.set_reactive_field(uid, 'createVaultForm', 'filename', 'test_vault.dat')
    agent.set_reactive_field(uid, 'createVaultForm', 'password', 'master123')
    agent.set_reactive_field(uid, 'createVaultForm', 'rounds', 100)

    agent.clear_messages()
    agent.call_method(uid, 'submitCreateVault')
    msg = agent.wait_for_message('success', timeout=15000)
    assert msg is not None
    assert msg['key'] == 'passwords.createVaultSuccess'
    time.sleep(0.5)

    state = agent.get_component_state(uid)
    vaults = _ref(state['setupState']['vaults'])
    assert len(vaults) == 1
    assert vaults[0]['name'] == 'TestVault'
    vault_id = vaults[0]['id']

    # Verify vault appears in tree and is unlocked (has children key, even if empty)
    tree_data = _ref(state['setupState']['treeData'])
    root_node = tree_data[0]
    vault_node = next((c for c in (root_node.get('children') or []) if c.get('id') == vault_id), None)
    assert vault_node is not None
    assert vault_node.get('children') is not None  # unlocked vaults show children

    # --- 2. Select vault and verify it's accessible ---
    agent.call_method(uid, 'handleNodeClick', [vault_node])
    time.sleep(0.5)

    state = agent.get_component_state(uid)
    current_vault = _ref(state['setupState']['currentVault'])
    assert current_vault is not None
    assert current_vault['name'] == 'TestVault'
    # No unlock dialog should appear since vault was auto-unlocked on creation
    assert _ref(state['setupState']['unlockDialogVisible']) is False

    filtered_passwords = _ref(state['setupState']['filteredPasswords'])
    assert len(filtered_passwords) == 0

    # --- 3. Add password ---
    agent.call_method(uid, 'handleAddPassword')
    time.sleep(1.0)

    state = agent.get_component_state(uid)
    assert _ref(state['setupState']['passwordDialogVisible']) is True
    assert _ref(state['setupState']['isEditPassword']) is False

    agent.set_reactive_field(uid, 'passwordForm', 'title', 'GitHub Account')
    agent.set_reactive_field(uid, 'passwordForm', 'username', 'user@example.com')
    agent.set_reactive_field(uid, 'passwordForm', 'password', 'gh_secret_pass')
    agent.set_reactive_field(uid, 'passwordForm', 'remark', 'Personal GitHub')
    time.sleep(0.3)

    agent.clear_messages()
    agent.call_method(uid, 'submitPasswordForm')
    msg = agent.wait_for_message('success', timeout=15000)
    assert msg is not None
    assert msg['key'] == 'passwords.addPasswordSuccess'
    time.sleep(0.5)

    state = agent.get_component_state(uid)
    filtered_passwords = _ref(state['setupState']['filteredPasswords'])
    assert len(filtered_passwords) == 1
    assert filtered_passwords[0]['title'] == 'GitHub Account'
    assert filtered_passwords[0]['username'] == 'user@example.com'
    assert filtered_passwords[0]['password'] == 'gh_secret_pass'
    assert filtered_passwords[0]['remark'] == 'Personal GitHub'

    state = agent.get_component_state(uid)
    filtered_passwords = _ref(state['setupState']['filteredPasswords'])
    assert len(filtered_passwords) == 1
    assert filtered_passwords[0]['title'] == 'GitHub Account'
    assert filtered_passwords[0]['username'] == 'user@example.com'
    assert filtered_passwords[0]['password'] == 'gh_secret_pass'
    assert filtered_passwords[0]['remark'] == 'Personal GitHub'

    # --- 4. Add a second password ---
    agent.call_method(uid, 'handleAddPassword')
    time.sleep(0.3)
    agent.set_reactive_field(uid, 'passwordForm', 'title', 'AWS Console')
    agent.set_reactive_field(uid, 'passwordForm', 'username', 'admin@aws')
    agent.set_reactive_field(uid, 'passwordForm', 'password', 'aws_key_123')
    agent.set_reactive_field(uid, 'passwordForm', 'remark', '')
    agent.clear_messages()
    agent.call_method(uid, 'submitPasswordForm')
    agent.wait_for_message('success', timeout=10000)
    time.sleep(0.3)

    state = agent.get_component_state(uid)
    filtered_passwords = _ref(state['setupState']['filteredPasswords'])
    assert len(filtered_passwords) == 2

    # --- 5. Search/filter passwords ---
    agent.set_ref(uid, 'searchKeyword', 'github')
    time.sleep(0.3)
    state = agent.get_component_state(uid)
    filtered_passwords = _ref(state['setupState']['filteredPasswords'])
    assert len(filtered_passwords) == 1
    assert filtered_passwords[0]['title'] == 'GitHub Account'

    agent.set_ref(uid, 'searchKeyword', '')
    time.sleep(0.3)

    # --- 6. Edit password ---
    state = agent.get_component_state(uid)
    passwords = _ref(state['setupState']['filteredPasswords'])
    gh_item = next(p for p in passwords if p['title'] == 'GitHub Account')

    agent.call_method(uid, 'handleEditPassword', [gh_item])
    time.sleep(0.3)

    state = agent.get_component_state(uid)
    assert _ref(state['setupState']['passwordDialogVisible']) is True
    assert _ref(state['setupState']['isEditPassword']) is True
    password_form = _ref(state['setupState']['passwordForm'])
    assert password_form['title'] == 'GitHub Account'
    assert password_form['username'] == 'user@example.com'

    agent.set_reactive_field(uid, 'passwordForm', 'title', 'GitHub Pro')
    agent.set_reactive_field(uid, 'passwordForm', 'remark', 'Pro account')
    agent.clear_messages()
    agent.call_method(uid, 'submitPasswordForm')
    agent.wait_for_message('success', timeout=10000)
    time.sleep(0.3)

    state = agent.get_component_state(uid)
    filtered_passwords = _ref(state['setupState']['filteredPasswords'])
    gh_item = next(p for p in filtered_passwords if p['id'] == gh_item['id'])
    assert gh_item['title'] == 'GitHub Pro'
    assert gh_item['remark'] == 'Pro account'

    # --- 7. Password generator ---
    agent.call_method(uid, 'handleAddPassword')
    time.sleep(0.3)
    agent.set_reactive_field(uid, 'generatorConfig', 'length', 16)
    agent.set_reactive_field(uid, 'generatorConfig', 'numbers', True)
    agent.set_reactive_field(uid, 'generatorConfig', 'lowercase', True)
    agent.set_reactive_field(uid, 'generatorConfig', 'uppercase', True)
    agent.set_reactive_field(uid, 'generatorConfig', 'special', False)
    agent.call_method(uid, 'generatePassword')
    time.sleep(0.3)

    state = agent.get_component_state(uid)
    pw_form = _ref(state['setupState']['passwordForm'])
    generated_pw = pw_form['password']
    assert len(generated_pw) == 16
    valid_chars = set('0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ')
    assert all(c in valid_chars for c in generated_pw)

    # Close the add password dialog
    agent.set_ref(uid, 'passwordDialogVisible', False)
    time.sleep(0.3)

    # --- 8. Create folder ---
    agent.set_reactive_field(uid, 'contextMenu', 'vaultId', vault_id)
    agent.set_reactive_field(uid, 'contextMenu', 'isFolder', False)
    agent.set_reactive_field(uid, 'contextMenu', 'isRoot', False)
    agent.call_method(uid, 'handleCreateFolder')
    time.sleep(0.3)

    state = agent.get_component_state(uid)
    assert _ref(state['setupState']['folderDialogVisible']) is True
    assert _ref(state['setupState']['isEditingFolder']) is False

    agent.set_ref(uid, 'folderFormName', 'Work')
    agent.clear_messages()
    agent.call_method(uid, 'submitFolderForm')
    agent.wait_for_message('success', timeout=10000)
    time.sleep(0.3)

    state = agent.get_component_state(uid)
    vault_data_map = _ref(state['setupState']['vaultDataMap'])
    vault_data = vault_data_map.get(vault_id) if isinstance(vault_data_map, dict) else None
    if vault_data is None:
        # vaultDataMap is a Map, agent returns it as object entries
        vault_data_raw = state['setupState']['vaultDataMap']
        if isinstance(vault_data_raw, dict) and '__type' in vault_data_raw:
            map_val = vault_data_raw['value']
            vault_data = map_val.get(vault_id) if isinstance(map_val, dict) else None

    # Verify folder exists by checking tree data
    tree_data = _ref(state['setupState']['treeData'])
    assert tree_data is not None
    root_node = tree_data[0] if tree_data else None
    assert root_node is not None
    vault_node = next((c for c in (root_node.get('children') or []) if c.get('id') == vault_id), None)
    assert vault_node is not None
    folder_children = vault_node.get('children') or []
    assert len(folder_children) >= 1
    assert any(f.get('label') == 'Work' for f in folder_children)

    # --- 9. Rename folder ---
    work_folder = next(f for f in folder_children if f.get('label') == 'Work')
    folder_id = work_folder['id'].replace('folder-', '')

    agent.set_reactive_field(uid, 'contextMenu', 'vaultId', vault_id)
    agent.set_reactive_field(uid, 'contextMenu', 'folderId', folder_id)
    agent.set_reactive_field(uid, 'contextMenu', 'isFolder', True)
    agent.call_method(uid, 'handleRenameFolder')
    time.sleep(0.3)

    state = agent.get_component_state(uid)
    assert _ref(state['setupState']['folderDialogVisible']) is True
    assert _ref(state['setupState']['isEditingFolder']) is True
    assert _ref(state['setupState']['folderFormName']) == 'Work'

    agent.set_ref(uid, 'folderFormName', 'Work Accounts')
    agent.clear_messages()
    agent.call_method(uid, 'submitFolderForm')
    agent.wait_for_message('success', timeout=10000)
    time.sleep(0.3)

    # --- 10. Lock vault ---
    agent.set_reactive_field(uid, 'contextMenu', 'vaultId', vault_id)
    agent.call_method(uid, 'handleLockVault')
    time.sleep(0.5)

    state = agent.get_component_state(uid)
    # Vault locked: currentVault should be null
    current_vault = _ref(state['setupState']['currentVault'])
    assert current_vault is None
    # Tree node should not have children (locked)
    tree_data = _ref(state['setupState']['treeData'])
    root_node = tree_data[0]
    vault_node = next((c for c in (root_node.get('children') or []) if c.get('id') == vault_id), None)
    assert vault_node is not None
    assert vault_node.get('children') is None

    # --- 11. Unlock vault with wrong password ---
    state = agent.get_component_state(uid)
    vaults = _ref(state['setupState']['vaults'])
    vault_obj = vaults[0]
    agent.call_method(uid, 'selectVault', [vault_obj])
    time.sleep(0.5)

    state = agent.get_component_state(uid)
    assert _ref(state['setupState']['unlockDialogVisible']) is True

    agent.set_ref(uid, 'unlockPassword', 'wrongpass')
    agent.clear_messages()
    agent.call_method(uid, 'submitUnlock')
    msg = agent.wait_for_message('error', timeout=10000)
    assert msg is not None
    assert msg['key'] == 'passwords.wrongPassword'

    # --- 12. Unlock vault with correct password ---
    agent.set_ref(uid, 'unlockPassword', 'master123')
    agent.clear_messages()
    agent.call_method(uid, 'submitUnlock')
    msg = agent.wait_for_message('success', timeout=10000)
    assert msg is not None
    assert msg['key'] == 'passwords.openVaultSuccess'
    time.sleep(0.5)

    state = agent.get_component_state(uid)
    current_vault = _ref(state['setupState']['currentVault'])
    assert current_vault is not None
    assert current_vault['name'] == 'TestVault'

    # Verify passwords still present after unlock
    filtered_passwords = _ref(state['setupState']['filteredPasswords'])
    assert len(filtered_passwords) == 2

    # --- 13. Delete vault via API (ElMessageBox bypass) ---
    page.evaluate("""async (id) => {
        await fetch('/api/vault/delete', {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({id: id}),
            credentials: 'include'
        });
    }""", vault_id)

    agent.call_method(uid, 'loadVaults')
    time.sleep(1)

    state = agent.get_component_state(uid)
    vaults = _ref(state['setupState']['vaults'])
    assert len(vaults) == 0

    # --- 14. H12: vaultMasterPasswords should not store plaintext passwords ---
    comp, state = _navigate_to_passwords(page, agent)
    uid = comp['id']

    agent.set_reactive_field(uid, 'contextMenu', 'isRoot', True)
    agent.set_reactive_field(uid, 'contextMenu', 'visible', True)
    time.sleep(0.3)
    agent.call_method(uid, 'handleCreateVault')
    time.sleep(0.5)

    agent.set_reactive_field(uid, 'createVaultForm', 'name', 'H12Vault')
    agent.set_reactive_field(uid, 'createVaultForm', 'description', 'H12 test')
    agent.set_reactive_field(uid, 'createVaultForm', 'path', '/')
    agent.set_reactive_field(uid, 'createVaultForm', 'filename', 'h12_vault.dat')
    agent.set_reactive_field(uid, 'createVaultForm', 'password', 'plaintext_master_pw')
    agent.set_reactive_field(uid, 'createVaultForm', 'rounds', 100)

    agent.clear_messages()
    agent.call_method(uid, 'submitCreateVault')
    agent.wait_for_message('success', timeout=15000)
    time.sleep(0.5)

    plaintext_found = page.evaluate("""() => {
        const app = document.getElementById('app').__vue_app__;
        const allComps = [];
        function walk(vnode) {
            if (vnode && vnode.component) {
                const name = vnode.type?.name || vnode.type?.__name;
                if (name === 'Passwords') allComps.push(vnode.component);
            }
            if (vnode && vnode.children) {
                if (Array.isArray(vnode.children)) vnode.children.forEach(walk);
                else if (typeof vnode.children === 'object') Object.values(vnode.children).forEach(walk);
            }
            if (vnode && vnode.component && vnode.component.subTree) walk(vnode.component.subTree);
        }
        walk(app._instance.subTree);
        if (allComps.length === 0) return false;
        const setupState = allComps[0].setupState;
        if (!setupState || !setupState.vaultMasterPasswords) return false;
        const mp = setupState.vaultMasterPasswords;
        const mapVal = mp.__v_isRef ? mp.value : mp;
        if (mapVal instanceof Map) {
            for (const [key, value] of mapVal) {
                if (value === 'plaintext_master_pw') return true;
            }
        }
        return false;
    }""")

    assert not plaintext_found, \
        'H12 BUG: vaultMasterPasswords stores plaintext master password instead of derived key!'

    page.evaluate("""async () => {
        const comps = window.__vue_agent__.findComponentsByName('Passwords');
        if (comps.length > 0) {
            const state = window.__vue_agent__.getComponentState(comps[0].id);
            const vaults = state?.setupState?.vaults;
            if (vaults) {
                const v = vaults.__type ? vaults.value : vaults;
                if (Array.isArray(v) && v.length > 0) {
                    await fetch('/api/vault/delete', {
                        method: 'POST',
                        headers: {'Content-Type': 'application/json'},
                        body: JSON.stringify({id: v[0].id}),
                        credentials: 'include'
                    });
                }
            }
        }
    }""")
    time.sleep(0.5)


if __name__ == '__main__':
    run_frontend_test(test_passwords)

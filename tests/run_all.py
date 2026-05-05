import os
import sys
import subprocess
import time

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
BACKEND_TESTS_DIR = os.path.join(SCRIPT_DIR, 'backend')
FRONTEND_TESTS_DIR = os.path.join(SCRIPT_DIR, 'frontend')

BACKEND_TESTS = [
    'test_system_api.py',
    'test_auth_api.py',
    'test_user_api.py',
    'test_file_api.py',
    'test_compress_api.py',
    'test_recycle_api.py',
    'test_vault_api.py',
    'test_notebook_api.py',
    'test_share_api.py',
    'test_backup_api.py',
    'test_backup_restore_api.py',
    'test_backup_restore_error.py',
    'test_webdav_api.py',
    'test_webdav_protocol.py',
]

FRONTEND_TESTS = [
    'test_system_init.py',
    'test_login.py',
    'test_file_manager.py',
    'test_notes.py',
    'test_passwords.py',
    'test_recycle_bin.py',
    'test_share_page.py',
    'test_webdav.py',
    'test_backup_restore.py',
    'test_account_management.py',
    'test_profile_center.py',
    'test_i18n.py',
]


def run_test(script_path, label):
    print(f'\n{"=" * 60}')
    print(f'  运行: {label}')
    print(f'{"=" * 60}')
    start = time.time()
    result = subprocess.run(
        [sys.executable, script_path],
        cwd=os.path.dirname(script_path),
    )
    elapsed = time.time() - start
    status = 'PASS' if result.returncode == 0 else 'FAIL'
    print(f'\n  [{status}] {label} ({elapsed:.1f}s)')
    return result.returncode == 0


def main():
    passed = []
    failed = []
    skipped = []

    if len(sys.argv) > 1:
        target = sys.argv[1].lower()
        run_backend = target in ('backend', 'b', 'all')
        run_frontend = target in ('frontend', 'f', 'all')
        if not run_backend and not run_frontend:
            print(f'用法: python run_all.py [backend|b|frontend|f|all]')
            print(f'  无参数时运行全部测试')
            sys.exit(1)
    else:
        run_backend = True
        run_frontend = True

    if run_backend:
        print(f'\n{"#" * 60}')
        print(f'  后端测试 ({len(BACKEND_TESTS)} 个)')
        print(f'{"#" * 60}')
        for test_file in BACKEND_TESTS:
            path = os.path.join(BACKEND_TESTS_DIR, test_file)
            if not os.path.exists(path):
                print(f'  [SKIP] {test_file} (文件不存在)')
                skipped.append(test_file)
                continue
            label = f'backend/{test_file}'
            if run_test(path, label):
                passed.append(label)
            else:
                failed.append(label)

    if run_frontend:
        print(f'\n{"#" * 60}')
        print(f'  前端测试 ({len(FRONTEND_TESTS)} 个)')
        print(f'{"#" * 60}')
        for test_file in FRONTEND_TESTS:
            path = os.path.join(FRONTEND_TESTS_DIR, test_file)
            if not os.path.exists(path):
                print(f'  [SKIP] {test_file} (文件不存在)')
                skipped.append(test_file)
                continue
            label = f'frontend/{test_file}'
            if run_test(path, label):
                passed.append(label)
            else:
                failed.append(label)

    print(f'\n{"=" * 60}')
    print(f'  测试结果汇总')
    print(f'{"=" * 60}')
    print(f'  通过: {len(passed)}')
    print(f'  失败: {len(failed)}')
    print(f'  跳过: {len(skipped)}')
    print(f'  总计: {len(passed) + len(failed) + len(skipped)}')

    if failed:
        print(f'\n  失败列表:')
        for name in failed:
            print(f'    - {name}')

    sys.exit(1 if failed else 0)


if __name__ == '__main__':
    main()

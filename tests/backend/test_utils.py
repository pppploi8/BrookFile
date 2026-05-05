import os
import sys
import tempfile
import requests

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
from common import (
    build_backend, start_backend, init_system, login,
    print_error_log, stop_backend,
    BASE_URL, DEFAULT_USERNAME, DEFAULT_PASSWORD, DEFAULT_SYSTEM_NAME,
)


def run_tests(test_func, init=True):
    test_name = test_func.__name__
    workdir = tempfile.mkdtemp(prefix='brookfile_test_')
    root_path = os.path.join(workdir, 'data')
    recycle_bin_path = os.path.join(workdir, 'recycle')

    build_backend()

    try:
        process = start_backend(workdir)
        session = requests.Session()

        if init:
            os.makedirs(root_path)
            os.makedirs(recycle_bin_path)
            init_system(session, root_path, recycle_bin_path)
            login(session)

        print(f'\n=== [{test_name}] 运行测试 ===')
        test_func(session, root_path)
        print(f'\n=== [{test_name}] 所有测试通过 ===')
    except Exception:
        print(f'\n=== [{test_name}] 测试失败 ===')
        raise
    finally:
        print_error_log(workdir, test_name)
        if 'process' in locals():
            stop_backend(process, workdir, test_name)

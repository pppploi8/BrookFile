import os
import requests
from test_utils import run_tests, BASE_URL


def test_compress_api(session, root_path):
    folder_path = os.path.join(root_path, 'test_compress_folder')
    subfolder_path = os.path.join(folder_path, 'subfolder')
    os.makedirs(subfolder_path)
    with open(os.path.join(folder_path, 'file1.txt'), 'w') as f:
        f.write('Hello, this is file 1!')
    with open(os.path.join(folder_path, 'file2.txt'), 'w') as f:
        f.write('This is file 2 content.')
    with open(os.path.join(subfolder_path, 'file3.txt'), 'w') as f:
        f.write('File in subfolder.')

    resp = session.post(f'{BASE_URL}/api/file/download_folder', json={'path': 'nonexistent_folder'})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PATH_NOT_FOUND'

    with open(os.path.join(root_path, 'test_file.txt'), 'w') as f:
        f.write('not a directory')
    resp = session.post(f'{BASE_URL}/api/file/download_folder', json={'path': 'test_file.txt'})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_A_DIRECTORY'

    resp = session.post(f'{BASE_URL}/api/file/download_folder', json={'path': 'test_compress_folder'})
    assert resp.status_code == 200
    content_type = resp.headers.get('Content-Type', '')
    assert 'zip' in content_type or 'octet-stream' in content_type
    assert resp.content[:4] == b'PK\x03\x04'

    import zipfile
    import io
    z = zipfile.ZipFile(io.BytesIO(resp.content))
    names = z.namelist()
    assert 'file1.txt' in names
    assert 'file2.txt' in names
    assert 'subfolder/file3.txt' in names or 'subfolder\\file3.txt' in names
    assert z.read('file1.txt') == b'Hello, this is file 1!'
    assert z.read('file2.txt') == b'This is file 2 content.'
    assert z.read('subfolder/file3.txt') == b'File in subfolder.'
    z.close()

    cn_folder_path = os.path.join(root_path, '中文文件夹')
    cn_sub_path = os.path.join(cn_folder_path, '子目录')
    os.makedirs(cn_sub_path)
    with open(os.path.join(cn_folder_path, '文件一.txt'), 'w', encoding='utf-8') as f:
        f.write('中文内容测试')
    with open(os.path.join(cn_sub_path, '文件二.txt'), 'w', encoding='utf-8') as f:
        f.write('子目录中文文件')

    resp = session.post(f'{BASE_URL}/api/file/download_folder', json={'path': '中文文件夹'})
    assert resp.status_code == 200
    assert resp.content[:4] == b'PK\x03\x04'

    z = zipfile.ZipFile(io.BytesIO(resp.content))
    names = z.namelist()
    assert any('文件一.txt' in n for n in names), f'文件一.txt not found in {names}'
    assert any('文件二.txt' in n for n in names), f'文件二.txt not found in {names}'
    assert any(n.endswith('文件一.txt') for n in names)
    assert z.read([n for n in names if n.endswith('文件一.txt')][0]) == '中文内容测试'.encode('utf-8')
    assert z.read([n for n in names if n.endswith('文件二.txt')][0]) == '子目录中文文件'.encode('utf-8')
    z.close()


if __name__ == '__main__':
    run_tests(test_compress_api)

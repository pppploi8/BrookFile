import os
import sqlite3
import requests
from test_utils import run_tests, BASE_URL


def _set_ebook_path(session, path):
    return session.post(f'{BASE_URL}/api/user/set_ebook_path', json={'ebook_path': path})


def _info(session):
    return session.post(f'{BASE_URL}/api/system/info').json()


def _make_test_png_base64():
    """生成一个最小合法 1x1 PNG 的 base64，用于模拟前端上传的封面。"""
    import struct
    import zlib
    import base64

    w = h = 1
    raw = b'\x00\xff\x00\x00'  # 过滤字节 0 + 红像素 RGB
    def chunk(typ, data):
        c = typ + data
        return struct.pack('>I', len(data)) + c + struct.pack('>I', zlib.crc32(c) & 0xffffffff)
    sig = b'\x89PNG\r\n\x1a\n'
    ihdr = struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0)
    idat = zlib.compress(raw)
    png = sig + chunk(b'IHDR', ihdr) + chunk(b'IDAT', idat) + chunk(b'IEND', b'')
    return base64.b64encode(png).decode()


def test_ebook_api(session, root_path):
    # --- 0. 初始状态：未启用电子书 ---
    info = _info(session)
    assert info['user']['ebook_enabled'] is False
    assert info['user'].get('ebook_path') is None
    assert info['user']['ebook_db_status'] == 'missing'

    # --- 1. 设置为空串：关闭（幂等，保持未启用） ---
    resp = _set_ebook_path(session, '')
    assert resp.json()['success'] is True
    info = _info(session)
    assert info['user']['ebook_enabled'] is False
    assert info['user'].get('ebook_path') is None

    # --- 2. 设置为不存在的相对目录：PATH_INVALID ---
    resp = _set_ebook_path(session, 'does_not_exist_xyz')
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PATH_INVALID'

    # --- 3. 设置为不安全路径（父目录引用）：PATH_INVALID ---
    resp = _set_ebook_path(session, '..')
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PATH_INVALID'

    # --- 4. 设置空目录：自动初始化元数据库，启用成功，状态 ok ---
    ebook_dir = os.path.join(root_path, 'ebook_data')
    os.makedirs(ebook_dir, exist_ok=True)
    resp = _set_ebook_path(session, 'ebook_data')
    assert resp.json()['success'] is True
    info = _info(session)
    assert info['user']['ebook_path'] == 'ebook_data'
    assert info['user']['ebook_enabled'] is True
    assert info['user']['ebook_db_status'] == 'ok'
    # 元数据库文件已创建
    meta_db = os.path.join(ebook_dir, '.brookfile', 'metadata.db')
    assert os.path.exists(meta_db), '元数据库未自动创建'
    # 三张表都已建立
    conn = sqlite3.connect(meta_db)
    tables = [r[0] for r in conn.execute("SELECT name FROM sqlite_master WHERE type='table'")]
    conn.close()
    for t in ('meta', 'books', 'reading_progress'):
        assert t in tables, f'缺少数据表: {t}'

    # --- 5. 重复打开同一合法目录：复用元数据库，成功 ---
    resp = _set_ebook_path(session, 'ebook_data')
    assert resp.json()['success'] is True
    info = _info(session)
    assert info['user']['ebook_db_status'] == 'ok'
    assert info['user']['ebook_enabled'] is True

    # --- 5.5 PDF 封面：后端不生成，统一由前端渲染后上传 ---
    # 在 ebook_data 下放一个最小 PDF 文件（后端只做元数据提取，不渲染封面）
    pdf_path = os.path.join(ebook_dir, 'sample.pdf')
    with open(pdf_path, 'wb') as f:
        f.write(b"%PDF-1.1\n"
                b"1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n"
                b"2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n"
                b"3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 300 300]>>endobj\n"
                b"trailer<</Root 1 0 R>>\n")
    resp = session.post(f'{BASE_URL}/api/ebook/shelf/add',
                        json={'items': [{'path': 'ebook_data/sample.pdf'}]})
    add_data = resp.json()
    assert add_data['success'] is True
    assert len(add_data['added']) == 1, 'PDF 添加失败'
    book_id = add_data['added'][0]['id']
    batch_id = add_data['batch_id']
    # 等待后台解析完成
    if batch_id:
        for _ in range(60):
            prog = session.post(f'{BASE_URL}/api/ebook/scan/progress',
                                json={'batch_id': batch_id}).json()
            if not prog.get('is_running'):
                break
    # 解析后 PDF 不应有服务端封面
    shelf = session.post(f'{BASE_URL}/api/ebook/shelf/list').json()
    the_book = next(b for b in shelf['books'] if b['id'] == book_id)
    assert the_book['format'] == 'pdf'
    assert the_book['has_preview'] is False, 'PDF 不应由后端生成封面'
    # 前端渲染首页后上传封面（这里用一张最小 PNG 模拟前端结果）
    png_b64 = _make_test_png_base64()
    resp = session.post(f'{BASE_URL}/api/ebook/shelf/cover',
                        json={'book_id': book_id, 'image': png_b64})
    data = resp.json()
    assert data['success'] is True, f'封面上传失败: {data}'
    # 上传后 has_preview 应置真
    shelf = session.post(f'{BASE_URL}/api/ebook/shelf/list').json()
    the_book = next(b for b in shelf['books'] if b['id'] == book_id)
    assert the_book['has_preview'] is True, '封面上传后 has_preview 未置真'
    # 预览文件应已落盘
    preview_file = os.path.join(ebook_dir, '.brookfile', 'previews', f'{book_id}.png')
    assert os.path.exists(preview_file), '预览文件未写入磁盘'

    # --- 5.6 TXT 封面：天然无图，不生成占位封面（has_preview=False），
    #        前端以「背景色 + 书名」模式显示 ---
    txt_path = os.path.join(ebook_dir, 'sample.txt')
    with open(txt_path, 'w', encoding='utf-8') as f:
        f.write('第一章\nhello world\n第二章\nmore text\n')
    resp = session.post(f'{BASE_URL}/api/ebook/shelf/add',
                        json={'items': [{'path': 'ebook_data/sample.txt'}]})
    add_data = resp.json()
    assert add_data['success'] is True
    assert len(add_data['added']) == 1, 'TXT 添加失败'
    txt_book_id = add_data['added'][0]['id']
    txt_batch = add_data['batch_id']
    if txt_batch:
        for _ in range(60):
            prog = session.post(f'{BASE_URL}/api/ebook/scan/progress',
                                json={'batch_id': txt_batch}).json()
            if not prog.get('is_running'):
                break
    shelf = session.post(f'{BASE_URL}/api/ebook/shelf/list').json()
    txt_book = next(b for b in shelf['books'] if b['id'] == txt_book_id)
    assert txt_book['format'] == 'txt'
    assert txt_book['has_preview'] is False, 'TXT 不应生成占位封面'

    # --- 6. 嵌套子目录：自动初始化在更深层级，启用成功 ---
    nested_dir = os.path.join(root_path, 'ebook_nested', 'sub')
    os.makedirs(nested_dir, exist_ok=True)
    resp = _set_ebook_path(session, 'ebook_nested/sub')
    assert resp.json()['success'] is True
    info = _info(session)
    assert info['user']['ebook_path'] == 'ebook_nested/sub'
    assert info['user']['ebook_enabled'] is True
    assert info['user']['ebook_db_status'] == 'ok'
    nested_meta_db = os.path.join(nested_dir, '.brookfile', 'metadata.db')
    assert os.path.exists(nested_meta_db), '嵌套目录元数据库未创建'

    # --- 7. 损坏的元数据库：EBOOK_DB_INVALID，且拒绝启用 ---
    corrupt_dir = os.path.join(root_path, 'ebook_corrupt')
    os.makedirs(os.path.join(corrupt_dir, '.brookfile'), exist_ok=True)
    with open(os.path.join(corrupt_dir, '.brookfile', 'metadata.db'), 'wb') as f:
        f.write(b'this is definitely not a sqlite database')
    resp = _set_ebook_path(session, 'ebook_corrupt')
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'EBOOK_DB_INVALID'
    # 之前已启用的合法目录不受影响，仍处于启用状态
    info = _info(session)
    assert info['user']['ebook_enabled'] is True
    assert info['user']['ebook_path'] == 'ebook_nested/sub'

    # --- 8. schema 不匹配（合法 sqlite 但无预期表/版本不符）：EBOOK_DB_INVALID ---
    mismatch_dir = os.path.join(root_path, 'ebook_mismatch')
    os.makedirs(os.path.join(mismatch_dir, '.brookfile'), exist_ok=True)
    # 创建一个 user_version=0 的空 sqlite 库（缺少预期表结构）
    conn = sqlite3.connect(os.path.join(mismatch_dir, '.brookfile', 'metadata.db'))
    conn.close()
    resp = _set_ebook_path(session, 'ebook_mismatch')
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'EBOOK_DB_INVALID'

    # --- 9. 清空路径：关闭电子书，状态回到 missing ---
    resp = _set_ebook_path(session, '')
    assert resp.json()['success'] is True
    info = _info(session)
    assert info['user']['ebook_enabled'] is False
    assert info['user'].get('ebook_path') is None
    assert info['user']['ebook_db_status'] == 'missing'

    # --- 10. 未登录：NOT_LOGGED_IN ---
    anon = requests.Session()
    resp = anon.post(f'{BASE_URL}/api/user/set_ebook_path', json={'ebook_path': 'x'})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_LOGGED_IN'


if __name__ == '__main__':
    run_tests(test_ebook_api)

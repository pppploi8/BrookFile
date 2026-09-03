import os
import time
import sqlite3
import struct
import zlib
import zipfile
import requests
from test_utils import run_tests, BASE_URL


def _make_txt_file(path, content, encoding='utf-8'):
    with open(path, 'w', encoding=encoding) as f:
        f.write(content)


def _make_simple_epub(path, title='Test EPUB'):
    import io
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, 'w', zipfile.ZIP_DEFLATED) as zf:
        zf.writestr('META-INF/container.xml', '''<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>''')
        zf.writestr('content.opf', f'''<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>{title}</dc:title>
    <dc:creator>Test Author</dc:creator>
  </metadata>
  <manifest>
    <item id="ch1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
    <item id="ch2" href="chapter2.xhtml" media-type="application/xhtml+xml"/>
    <item id="cover" href="cover.png" media-type="image/png" properties="cover-image"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
    <itemref idref="ch2"/>
  </spine>
</package>''')
        zf.writestr('chapter1.xhtml', '<html><body><h1>Chapter 1</h1><p>Content 1</p></body></html>')
        zf.writestr('chapter2.xhtml', '<html><body><h1>Chapter 2</h1><p>Content 2</p></body></html>')
        cover_data = b'\x89PNG\r\n\x1a\n' + b'\x00' * 100
        zf.writestr('cover.png', cover_data)
    with open(path, 'wb') as f:
        f.write(buf.getvalue())


def _make_simple_pdf(path, num_pages=5):
    objects = []
    offsets = []
    pdf = b'%PDF-1.4\n'

    objects.append(b'<< /Type /Catalog /Pages 2 0 R >>')
    objects.append(f'<< /Type /Pages /Kids [{" ".join([f"{3+2*i} 0 R" for i in range(num_pages)])}] /Count {num_pages} >>'.encode())
    for i in range(num_pages):
        objects.append(b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>')
        objects.append(b'<< /Length 44 >>\nstream\nBT /F1 12 Tf 100 700 Td (Page content) Tj ET\nendstream')

    for i, obj in enumerate(objects):
        offsets.append(len(pdf))
        pdf += f'{i+1} 0 obj\n'.encode() + obj + b'\nendobj\n'

    xref_offset = len(pdf)
    pdf += b'xref\n'
    pdf += f'0 {len(objects)+1}\n'.encode()
    pdf += b'0000000000 65535 f \n'
    for off in offsets:
        pdf += f'{off:010d} 00000 n \n'.encode()
    pdf += b'trailer\n'
    pdf += f'<< /Size {len(objects)+1} /Root 1 0 R >>\n'.encode()
    pdf += b'startxref\n'
    pdf += f'{xref_offset}\n'.encode()
    pdf += b'%%EOF\n'

    with open(path, 'wb') as f:
        f.write(pdf)


def _wait_for_scan(session, batch_id, timeout=30):
    for _ in range(timeout * 2):
        resp = session.post(f'{BASE_URL}/api/ebook/scan/progress', json={'batch_id': batch_id})
        data = resp.json()
        if not data['is_running']:
            return data
        time.sleep(0.5)
    return data


def test_ebook_reader_api(session, root_path):
    # --- 0. 初始状态 ---
    ebook_dir = os.path.join(root_path, 'ebook_data')
    os.makedirs(ebook_dir, exist_ok=True)

    resp = session.post(f'{BASE_URL}/api/ebook/config/set', json={'ebook_path': 'ebook_data'})
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/ebook/config/get')
    data = resp.json()
    assert data['success'] is True
    assert data['ebook_enabled'] is True
    assert data['ebook_path'] == 'ebook_data'
    assert data['ebook_db_status'] == 'ok'

    # --- 1. 空书架 ---
    resp = session.post(f'{BASE_URL}/api/ebook/shelf/list')
    data = resp.json()
    assert data['success'] is True
    assert data['books'] == []
    assert data['categories'] == []

    # --- 2. 添加 TXT 书籍（带章节标记） ---
    txt_dir = os.path.join(root_path, 'books')
    os.makedirs(txt_dir, exist_ok=True)
    txt_content = (
        '第一章 开始\n\n这是第一章的内容，有一些文字。\n\n'
        '第二章 发展\n\n这是第二章的内容。\n\n'
        '第三章 结局\n\n这是第三章的内容。\n\n'
    )
    txt_path = os.path.join(txt_dir, 'test_chapter.txt')
    _make_txt_file(txt_path, txt_content)

    resp = session.post(f'{BASE_URL}/api/ebook/shelf/add', json={
        'items': [{'path': 'books/test_chapter.txt'}]
    })
    data = resp.json()
    assert data['success'] is True
    assert len(data['added']) == 1
    assert data['added'][0]['title'] == 'test_chapter'
    assert data['added'][0]['format'] == 'txt'
    assert data['duplicates'] == []
    book_id = data['added'][0]['id']
    batch_id = data['batch_id']
    assert batch_id is not None

    # --- 3. 等待元数据扫描完成 ---
    progress = _wait_for_scan(session, batch_id)
    assert progress['total'] == 1
    assert progress['completed'] == 1
    assert progress['failed'] == 0
    assert progress['is_running'] is False

    # --- 4. 验证书架列表 ---
    resp = session.post(f'{BASE_URL}/api/ebook/shelf/list')
    data = resp.json()
    assert len(data['books']) == 1
    book = data['books'][0]
    assert book['id'] == book_id
    assert book['title'] == 'test_chapter'
    assert book['format'] == 'txt'
    assert book['scan_status'] == 'ready'
    assert book['has_preview'] is True

    # --- 5. 重复添加去重 ---
    resp = session.post(f'{BASE_URL}/api/ebook/shelf/add', json={
        'items': [{'path': 'books/test_chapter.txt'}]
    })
    data = resp.json()
    assert data['success'] is True
    assert len(data['added']) == 0
    assert len(data['duplicates']) == 1
    assert data['duplicates'][0]['path'] == 'books/test_chapter.txt'

    # --- 6. 获取书籍元数据 ---
    resp = session.post(f'{BASE_URL}/api/ebook/book/meta', json={'book_id': book_id})
    data = resp.json()
    assert data['success'] is True
    assert data['book']['id'] == book_id
    assert data['source_status'] == 'ok'
    chapters = data['chapters']
    assert len(chapters) >= 3
    assert chapters[0]['title'] is not None
    assert '第一章' in chapters[0]['title'] or '第二章' in chapters[0]['title'] or '第三章' in chapters[0]['title']

    # --- 7. 下载书籍 ---
    resp = session.post(f'{BASE_URL}/api/ebook/book/download', json={'book_id': book_id})
    assert resp.status_code == 200
    assert 'text/plain' in resp.headers.get('content-type', '')
    downloaded = resp.content.decode('utf-8')
    assert '第一章' in downloaded

    # --- 8. 获取预览图 ---
    resp = session.post(f'{BASE_URL}/api/ebook/preview/get', json={'book_id': book_id})
    assert resp.status_code == 200
    assert 'image/png' in resp.headers.get('content-type', '')
    assert len(resp.content) > 0

    # --- 9. 分类管理 ---
    resp = session.post(f'{BASE_URL}/api/ebook/category/add', json={'name': '小说'})
    data = resp.json()
    assert data['success'] is True
    cat_id = data['id']
    assert cat_id is not None

    resp = session.post(f'{BASE_URL}/api/ebook/category/list')
    data = resp.json()
    assert data['success'] is True
    assert len(data['categories']) == 1
    assert data['categories'][0]['name'] == '小说'

    resp = session.post(f'{BASE_URL}/api/ebook/category/rename', json={'id': cat_id, 'name': '文学'})
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/ebook/category/list')
    assert resp.json()['categories'][0]['name'] == '文学'

    resp = session.post(f'{BASE_URL}/api/ebook/category/move-book', json={'book_id': book_id, 'category_id': cat_id})
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/ebook/shelf/list')
    data = resp.json()
    assert data['books'][0]['category_id'] == cat_id

    # --- 10. 进度保存与获取（3 槽系统） ---
    resp = session.post(f'{BASE_URL}/api/ebook/progress/save', json={
        'book_id': book_id, 'slot': 1, 'content_coord': '0:100', 'summary': '开头'
    })
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/ebook/progress/save', json={
        'book_id': book_id, 'slot': 2, 'content_coord': '1:200', 'summary': '中间'
    })
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/ebook/progress/save', json={
        'book_id': book_id, 'slot': 3, 'content_coord': '2:300', 'summary': '结尾'
    })
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/ebook/progress/get', json={'book_id': book_id})
    data = resp.json()
    assert data['success'] is True
    assert len(data['slots']) == 3

    resp = session.post(f'{BASE_URL}/api/ebook/progress/save', json={
        'book_id': book_id, 'slot': 1, 'content_coord': '0:150', 'summary': '更新进度'
    })
    resp = session.post(f'{BASE_URL}/api/ebook/progress/get', json={'book_id': book_id})
    data = resp.json()
    slot1 = [s for s in data['slots'] if s['slot'] == 1][0]
    assert slot1['content_coord'] == '0:150'

    resp = session.post(f'{BASE_URL}/api/ebook/progress/save', json={
        'book_id': book_id, 'slot': 0, 'content_coord': 'x', 'summary': None
    })
    assert resp.json()['success'] is False
    assert resp.json()['fail_code'] == 'INVALID_PARAM'

    resp = session.post(f'{BASE_URL}/api/ebook/progress/save', json={
        'book_id': book_id, 'slot': 4, 'content_coord': 'x', 'summary': None
    })
    assert resp.json()['success'] is False
    assert resp.json()['fail_code'] == 'INVALID_PARAM'

    # --- 11. 书签管理 ---
    resp = session.post(f'{BASE_URL}/api/ebook/bookmark/add', json={
        'book_id': book_id, 'content_coord': '0:50', 'summary': '重要段落'
    })
    data = resp.json()
    assert data['success'] is True
    bm_id = data['id']

    resp = session.post(f'{BASE_URL}/api/ebook/bookmark/list', json={'book_id': book_id})
    data = resp.json()
    assert data['success'] is True
    assert len(data['bookmarks']) == 1
    assert data['bookmarks'][0]['id'] == bm_id
    assert data['bookmarks'][0]['summary'] == '重要段落'

    resp = session.post(f'{BASE_URL}/api/ebook/bookmark/remove', json={'id': bm_id})
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/ebook/bookmark/list', json={'book_id': book_id})
    assert len(resp.json()['bookmarks']) == 0

    # --- 12. GBK 编码 TXT ---
    gbk_path = os.path.join(txt_dir, 'gbk_book.txt')
    gbk_content = '第一章 GBK测试\n\n这是GBK编码的内容。\n\n第二章 继续\n\n更多内容。\n\n'
    _make_txt_file(gbk_path, gbk_content, encoding='gbk')

    resp = session.post(f'{BASE_URL}/api/ebook/shelf/add', json={
        'items': [{'path': 'books/gbk_book.txt'}]
    })
    data = resp.json()
    assert data['success'] is True
    assert len(data['added']) == 1
    gbk_book_id = data['added'][0]['id']
    gbk_batch_id = data['batch_id']

    progress = _wait_for_scan(session, gbk_batch_id)
    assert progress['completed'] == 1

    resp = session.post(f'{BASE_URL}/api/ebook/book/download', json={'book_id': gbk_book_id})
    downloaded = resp.content.decode('utf-8')
    assert 'GBK测试' in downloaded
    assert 'GBK编码' in downloaded

    # --- 13. 无章节标记的 TXT（600 字硬切） ---
    no_chapter_path = os.path.join(txt_dir, 'no_chapters.txt')
    no_chapter_content = '这是没有章节标记的内容。' * 100
    _make_txt_file(no_chapter_path, no_chapter_content)

    resp = session.post(f'{BASE_URL}/api/ebook/shelf/add', json={
        'items': [{'path': 'books/no_chapters.txt'}]
    })
    data = resp.json()
    assert data['success'] is True
    no_ch_book_id = data['added'][0]['id']
    no_ch_batch_id = data['batch_id']

    _wait_for_scan(session, no_ch_batch_id)

    resp = session.post(f'{BASE_URL}/api/ebook/book/meta', json={'book_id': no_ch_book_id})
    data = resp.json()
    chapters = data['chapters']
    assert len(chapters) > 1
    for ch in chapters:
        assert ch['title'] is not None
        assert '段落' in ch['title']

    # --- 14. EPUB 书籍 ---
    epub_path = os.path.join(txt_dir, 'test.epub')
    _make_simple_epub(epub_path, title='Test EPUB Book')

    resp = session.post(f'{BASE_URL}/api/ebook/shelf/add', json={
        'items': [{'path': 'books/test.epub'}]
    })
    data = resp.json()
    assert data['success'] is True
    assert len(data['added']) == 1
    assert data['added'][0]['format'] == 'epub'
    epub_book_id = data['added'][0]['id']
    epub_batch_id = data['batch_id']

    progress = _wait_for_scan(session, epub_batch_id)
    assert progress['completed'] == 1

    resp = session.post(f'{BASE_URL}/api/ebook/book/meta', json={'book_id': epub_book_id})
    data = resp.json()
    assert data['success'] is True
    assert data['book']['title'] == 'Test EPUB Book'
    assert data['source_status'] == 'ok'
    chapters = data['chapters']
    assert len(chapters) >= 2

    resp = session.post(f'{BASE_URL}/api/ebook/book/download', json={'book_id': epub_book_id})
    assert resp.status_code == 200
    assert 'epub' in resp.headers.get('content-type', '')

    # --- 15. 目录扫描发现 ---
    scan_dir = os.path.join(root_path, 'scan_test')
    os.makedirs(scan_dir, exist_ok=True)
    _make_txt_file(os.path.join(scan_dir, 'book1.txt'), '内容1')
    _make_txt_file(os.path.join(scan_dir, 'book2.txt'), '内容2')
    sub_dir = os.path.join(scan_dir, 'subdir')
    os.makedirs(sub_dir, exist_ok=True)
    _make_txt_file(os.path.join(sub_dir, 'book3.txt'), '内容3')
    _make_txt_file(os.path.join(scan_dir, 'not_a_book.md'), '不是书')

    resp = session.post(f'{BASE_URL}/api/ebook/scan/discover', json={'path': 'scan_test'})
    data = resp.json()
    assert data['success'] is True
    paths = [f['path'] for f in data['files']]
    assert len(data['files']) == 3
    assert any('book1.txt' in p for p in paths)
    assert any('book2.txt' in p for p in paths)
    assert any('book3.txt' in p for p in paths)
    assert not any('not_a_book.md' in p for p in paths)

    # --- 16. 源文件异常：文件不存在 ---
    os.rename(txt_path, txt_path + '.bak')

    resp = session.post(f'{BASE_URL}/api/ebook/book/meta', json={'book_id': book_id})
    data = resp.json()
    assert data['source_status'] == 'file_not_found'

    # relink: 重新指定路径
    resp = session.post(f'{BASE_URL}/api/ebook/shelf/relink', json={
        'book_id': book_id, 'new_path': 'books/test_chapter.txt.bak'
    })
    assert resp.json()['success'] is True

    relink_batch = resp.json().get('batch_id')
    if relink_batch:
        _wait_for_scan(session, relink_batch)
    else:
        time.sleep(1)

    resp = session.post(f'{BASE_URL}/api/ebook/book/meta', json={'book_id': book_id})
    data = resp.json()
    assert data['source_status'] == 'ok'

    # --- 17. 源文件异常：内容变化 ---
    with open(txt_path + '.bak', 'a', encoding='utf-8') as f:
        f.write('\n\n追加的内容改变了文件的hash\n')

    resp = session.post(f'{BASE_URL}/api/ebook/book/meta', json={'book_id': book_id})
    data = resp.json()
    assert data['source_status'] == 'content_changed'

    # relink: 更新元信息（不传 new_path，对原路径重算）
    resp = session.post(f'{BASE_URL}/api/ebook/shelf/relink', json={
        'book_id': book_id
    })
    assert resp.json()['success'] is True
    relink_batch = resp.json().get('batch_id')
    if relink_batch:
        _wait_for_scan(session, relink_batch)
    else:
        time.sleep(2)

    resp = session.post(f'{BASE_URL}/api/ebook/book/meta', json={'book_id': book_id})
    data = resp.json()
    assert data['source_status'] == 'ok'

    # 恢复文件名
    os.rename(txt_path + '.bak', txt_path)

    # --- 18. 分类删除 ---
    resp = session.post(f'{BASE_URL}/api/ebook/category/remove', json={'id': cat_id})
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/ebook/category/list')
    assert len(resp.json()['categories']) == 0

    resp = session.post(f'{BASE_URL}/api/ebook/shelf/list')
    data = resp.json()
    for b in data['books']:
        if b['id'] == book_id:
            assert b['category_id'] is None

    # --- 19. 移除书架 ---
    resp = session.post(f'{BASE_URL}/api/ebook/shelf/remove', json={'book_id': book_id})
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/ebook/shelf/list')
    data = resp.json()
    book_ids = [b['id'] for b in data['books']]
    assert book_id not in book_ids

    # --- 20. 移除不存在的书 ---
    resp = session.post(f'{BASE_URL}/api/ebook/shelf/remove', json={'book_id': 'nonexistent-id'})
    assert resp.json()['success'] is True

    # --- 21. PDF 书籍 ---
    pdf_path = os.path.join(txt_dir, 'test.pdf')
    _make_simple_pdf(pdf_path, num_pages=5)

    resp = session.post(f'{BASE_URL}/api/ebook/shelf/add', json={
        'items': [{'path': 'books/test.pdf'}]
    })
    data = resp.json()
    assert data['success'] is True
    assert len(data['added']) == 1
    assert data['added'][0]['format'] == 'pdf'
    pdf_book_id = data['added'][0]['id']
    pdf_batch_id = data['batch_id']

    progress = _wait_for_scan(session, pdf_batch_id)
    assert progress['completed'] == 1

    resp = session.post(f'{BASE_URL}/api/ebook/book/meta', json={'book_id': pdf_book_id})
    data = resp.json()
    assert data['success'] is True
    assert data['source_status'] == 'ok'
    chapters = data['chapters']
    assert len(chapters) >= 1

    resp = session.post(f'{BASE_URL}/api/ebook/book/download', json={'book_id': pdf_book_id})
    assert resp.status_code == 200
    assert 'pdf' in resp.headers.get('content-type', '')

    # --- 22. 更换数据目录（恢复） ---
    resp = session.post(f'{BASE_URL}/api/ebook/shelf/list')
    old_books = resp.json()['books']

    resp = session.post(f'{BASE_URL}/api/ebook/config/set', json={'ebook_path': ''})
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/ebook/config/get')
    data = resp.json()
    assert data['ebook_enabled'] is False

    resp = session.post(f'{BASE_URL}/api/ebook/config/set', json={'ebook_path': 'ebook_data'})
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/ebook/shelf/list')
    new_books = resp.json()['books']
    assert len(new_books) == len(old_books)

    # --- 23. 未配置电子书时操作返回 EBOOK_NOT_CONFIGURED ---
    resp = session.post(f'{BASE_URL}/api/ebook/config/set', json={'ebook_path': ''})
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/ebook/shelf/list')
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'EBOOK_NOT_CONFIGURED'

    # --- 24. 不安全的路径 ---
    resp = session.post(f'{BASE_URL}/api/ebook/config/set', json={'ebook_path': 'ebook_data'})
    assert resp.json()['success'] is True

    resp = session.post(f'{BASE_URL}/api/ebook/shelf/add', json={
        'items': [{'path': '../etc/passwd'}]
    })
    data = resp.json()
    assert data['success'] is True
    assert len(data['added']) == 0

    resp = session.post(f'{BASE_URL}/api/ebook/scan/discover', json={'path': '..'})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PATH_INVALID'

    print('\n所有电子书 API 测试通过！')


if __name__ == '__main__':
    run_tests(test_ebook_reader_api)

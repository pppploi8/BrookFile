import os
import time
import requests
from test_utils import run_frontend_test, BASE_URL, FRONTEND_URL
import common


def _make_reader_txt(path):
    def body(no, lines=120):
        return '\n'.join(f'第{no}章内容行{i}: ' + '测试文本段落' * 8 for i in range(lines))

    content = (
        '第一章 开始\n\n' + body(1) + '\n\n'
        '第二章 发展\n\n' + body(2) + '\n\n'
        '第三章 转折\n\n' + body(3) + '\n\n'
        '第四章 结局\n\n' + body(4) + '\n\n'
    )
    with open(path, 'w', encoding='utf-8') as f:
        f.write(content)


def _wait_scan(session, batch_id, timeout=30):
    for _ in range(timeout * 2):
        resp = session.post(f'{BASE_URL}/api/ebook/scan/progress', json={'batch_id': batch_id})
        data = resp.json()
        if not data['is_running']:
            return data
        time.sleep(0.5)
    return data


def _get_progress(session, book_id):
    resp = session.post(f'{BASE_URL}/api/ebook/progress/get', json={'book_id': book_id})
    data = resp.json()
    assert data['success'] is True
    return data['slots']


def _get_bookmarks(session, book_id):
    resp = session.post(f'{BASE_URL}/api/ebook/bookmark/list', json={'book_id': book_id})
    data = resp.json()
    assert data['success'] is True
    return data['bookmarks']


def _scroll(page, top):
    page.eval_on_selector('.reader-viewport', f'el => {{ el.scrollTop = {top}; }}')


def test_ebook_bookmark_persistence(page, agent, root_path, workdir):
    # ===== 1. 后端准备：ebook 目录 + 添加多章节 txt + 等待扫描 =====
    session = requests.Session()
    common.login(session)

    ebook_dir = os.path.join(root_path, 'ebook_data')
    os.makedirs(ebook_dir, exist_ok=True)
    resp = session.post(f'{BASE_URL}/api/ebook/config/set', json={'ebook_path': 'ebook_data'})
    assert resp.json()['success'] is True

    books_dir = os.path.join(root_path, 'books')
    os.makedirs(books_dir, exist_ok=True)
    txt_path = os.path.join(books_dir, 'bookmark_flow.txt')
    _make_reader_txt(txt_path)

    resp = session.post(f'{BASE_URL}/api/ebook/shelf/add', json={
        'items': [{'path': 'books/bookmark_flow.txt'}]
    })
    data = resp.json()
    assert data['success'] is True
    book_id = data['added'][0]['id']
    progress = _wait_scan(session, data['batch_id'])
    assert progress['completed'] == 1
    assert progress['failed'] == 0

    # ===== 2. 打开阅读器 =====
    page.goto(f'{FRONTEND_URL}/ebooks/read/{book_id}')
    page.wait_for_selector('.reader-shell', timeout=20000)
    time.sleep(3.0)

    # ===== 3. 滚动 → 防抖后保存到槽 1 =====
    _scroll(page, 1500)
    time.sleep(1.2)
    slots = _get_progress(session, book_id)
    assert len(slots) == 1
    assert slots[0]['slot'] == 1
    assert slots[0]['content_coord'].startswith('1:')
    coord_first = slots[0]['content_coord']

    # ===== 4. 继续滚动 → 仍只刷新槽 1 =====
    _scroll(page, 3500)
    time.sleep(1.2)
    slots = _get_progress(session, book_id)
    assert len(slots) == 1
    assert slots[0]['slot'] == 1
    assert slots[0]['content_coord'] != coord_first

    # ===== 5. 目录跳转 → 轮转开新槽 2 =====
    page.locator('.reader-toolbar .el-button').nth(2).click()
    page.locator('.reader-toc .toc-item').nth(1).click()
    time.sleep(1.2)
    slots = _get_progress(session, book_id)
    assert len(slots) == 2
    assert slots[0]['slot'] == 2
    assert slots[0]['content_coord'].startswith('2:')

    # ===== 6. 跳转输入框跳章 → 开槽 3 =====
    page.fill('.jump-input input', '3')
    page.press('.jump-input input', 'Enter')
    time.sleep(1.2)
    slots = _get_progress(session, book_id)
    assert len(slots) == 3
    assert slots[0]['slot'] == 3
    assert slots[0]['content_coord'].startswith('3:')

    # ===== 7. 再次跳转 → 槽满轮转回槽 1（替换最旧） =====
    page.fill('.jump-input input', '4')
    page.press('.jump-input input', 'Enter')
    time.sleep(1.2)
    slots = _get_progress(session, book_id)
    assert len(slots) == 3
    assert slots[0]['slot'] == 1
    assert slots[0]['content_coord'].startswith('4:')
    saved_coord = slots[0]['content_coord']

    # ===== 8. 手动添加书签 → 入库 =====
    page.locator('.reader-toolbar .el-button').nth(1).click()
    page.click('.bm-add')
    time.sleep(1.2)
    bookmarks = _get_bookmarks(session, book_id)
    assert len(bookmarks) == 1
    assert bookmarks[0]['content_coord'].startswith('4:')
    assert bookmarks[0]['summary'] and '第四章' in bookmarks[0]['summary']

    # ===== 9. 重新打开 → 自动恢复最新槽位 + 书签列表 =====
    page.reload()
    page.wait_for_selector('.reader-shell', timeout=20000)
    time.sleep(3.0)

    label = (page.text_content('.loc-label') or '').strip()
    assert label.startswith('4'), f'恢复位置错误，locationLabel={label}'

    page.locator('.reader-toolbar .el-button').nth(1).click()
    page.wait_for_selector('.bookmark-item', timeout=5000)
    assert page.locator('.bookmark-item').count() == 1

    slots = _get_progress(session, book_id)
    latest = slots[0]
    assert latest['content_coord'].startswith('4:')
    restored = latest['content_coord'].split(':')[1]
    saved = saved_coord.split(':')[1]
    assert abs(int(restored) - int(saved)) < 200, (
        f'恢复坐标漂移过大: saved={saved_coord}, restored={latest["content_coord"]}'
    )

    # ===== 9.5 「最近位置」列表：点击最旧槽跳回（跳转语义 → 轮转开新槽） =====
    assert page.locator('.recent-item').count() == 3
    page.locator('.recent-item').nth(2).click()
    time.sleep(1.2)
    label = (page.text_content('.loc-label') or '').strip()
    assert label.startswith('2'), f'最近位置跳转失败: {label}'
    slots = _get_progress(session, book_id)
    assert slots[0]['slot'] == 2
    assert slots[0]['content_coord'].startswith('2:')

    # ===== 10. 书架显示阅读进度 =====
    page.goto(f'{FRONTEND_URL}/ebooks')
    page.wait_for_selector('.book-progress', timeout=15000)
    progress_text = (page.text_content('.book-progress') or '').strip()
    assert progress_text.endswith('2 / 4'), f'书架进度显示异常: {progress_text}'


if __name__ == '__main__':
    run_frontend_test(test_ebook_bookmark_persistence, init=True)

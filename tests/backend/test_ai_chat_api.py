import json
import os
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import requests

from test_utils import run_tests

MOCK_CAPTURED = []
MOCK_SCRIPT = []
MOCK_TITLE_SCRIPT = []
MOCK_SUMMARY_SCRIPT = []
MOCK_HANG = []
# 待返回的上游错误：(状态码, 响应体)，用于验证原始报错透传
MOCK_ERROR = []

SUMMARIZATION_MARKER = 'context summarization assistant'


class MockResponsesHandler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_POST(self):
        try:
            return self._handle_post()
        except Exception:
            import traceback
            traceback.print_exc()
            try:
                self.send_response(500)
                self.end_headers()
            except Exception:
                pass

    def _handle_post(self):
        if MOCK_HANG:
            # 模拟"连得上但永不回包"：读走请求后挂住，不发任何响应
            MOCK_HANG.pop(0)
            time.sleep(90)
            return
        if MOCK_ERROR:
            status, payload = MOCK_ERROR.pop(0)
            raw = json.dumps(payload, ensure_ascii=False).encode('utf-8')
            self.send_response(status)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(raw)))
            self.end_headers()
            self.wfile.write(raw)
            return
        length = int(self.headers.get('Content-Length', 0))
        body = json.loads(self.rfile.read(length).decode('utf-8')) if length else {}
        MOCK_CAPTURED.append(body)
        if str(body.get('instructions', '')).startswith('You are a ' + SUMMARIZATION_MARKER):
            script = MOCK_SUMMARY_SCRIPT.pop(0) if MOCK_SUMMARY_SCRIPT else {'deltas': ['压缩后的摘要']}
        elif any(
            isinstance(i, dict) and i.get('role') == 'user' and '会话标题' in str(i.get('content', ''))
            for i in body.get('input', [])
        ):
            script = MOCK_TITLE_SCRIPT.pop(0) if MOCK_TITLE_SCRIPT else {'deltas': ['标题']}
        else:
            script = MOCK_SCRIPT.pop(0) if MOCK_SCRIPT else {'deltas': ['ok'], 'reasonings': []}

        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.end_headers()

        def emit(event, obj):
            obj = dict(obj)
            obj['type'] = event
            self.wfile.write(f"event: {event}\ndata: {json.dumps(obj, ensure_ascii=False)}\n\n".encode('utf-8'))
            self.wfile.flush()

        for chunk in script.get('reasonings', []):
            emit('response.reasoning_text.delta', {'delta': chunk})
        for delta in script.get('deltas', []):
            emit('response.output_text.delta', {'delta': delta})
        usage = script.get('usage', {'input_tokens': 100, 'output_tokens': 20, 'total_tokens': 120})
        # genai 从 response.completed 的 output 数组提取完整的 function_call 项；
        # response 对象需带 id/status/model 才能通过 genai 的反序列化
        output_items = [dict(call, type='function_call') for call in script.get('tool_calls', [])]
        emit('response.completed', {'response': {
            'id': 'resp_mock', 'status': 'completed', 'model': 'mock-model',
            'usage': usage, 'output': output_items,
        }})
        self.wfile.flush()


def start_mock_server():
    server = ThreadingHTTPServer(('127.0.0.1', 0), MockResponsesHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, server.server_address[1]


def read_sse_events(resp):
    events = []
    event_name = ''
    for raw in resp.iter_lines(decode_unicode=True):
        if raw is None:
            continue
        if raw == '':
            if event_name:
                events.append(event_name)
            event_name = ''
            continue
        if raw.startswith('event:'):
            event_name = raw[len('event:'):].strip()
    if event_name:
        events.append(event_name)
    return events


def read_sse_frames(resp):
    """返回 [(事件名, 数据 dict), ...]，用于断言错误事件携带的原始报错"""
    frames = []
    event_name = ''
    data_lines = []
    for raw in resp.iter_lines(decode_unicode=True):
        if raw is None:
            continue
        if raw == '':
            if event_name:
                frames.append((event_name, json.loads('\n'.join(data_lines)) if data_lines else {}))
            event_name = ''
            data_lines = []
            continue
        if raw.startswith('event:'):
            event_name = raw[len('event:'):].strip()
        elif raw.startswith('data:'):
            data_lines.append(raw[len('data:'):].strip())
    if event_name:
        frames.append((event_name, json.loads('\n'.join(data_lines)) if data_lines else {}))
    return frames


def test_ai_chat_api(session, root_path):
    server, mock_port = start_mock_server()
    try:
        base = 'http://127.0.0.1:3000'

        # --- 未设置存储目录时创建会话失败 ---
        resp = session.post(f'{base}/api/ai/chat/create', json={'biz_type': 'ebook', 'biz_id': 'book-1'})
        assert resp.json()['fail_code'] == 'AI_CHAT_PATH_NOT_SET', resp.text

        # --- 设置存储目录（服务端自动创建） ---
        resp = session.post(f'{base}/api/user/set_ai_chat_path', json={'ai_chat_path': 'aichat'})
        assert resp.json()['success'] is True, resp.text

        # --- 配置供应商与模型（指向 mock 服务） ---
        resp = session.post(f'{base}/api/ai/provider/create', json={
            'name': 'Mock', 'provider_type': 'openai_responses',
            'base_url': f'http://127.0.0.1:{mock_port}/v1', 'api_key': 'sk-test',
            'models': [
                {'model_id': 'mock-model'},
                {'model_id': 'compact-model', 'context_length': 52000},
            ],
        })
        provider_id = resp.json()['provider_id']
        resp = session.post(f'{base}/api/ai/provider/list')
        models = next(p['models'] for p in resp.json()['providers'] if p['id'] == provider_id)
        model_key = next(m['id'] for m in models if m['model_id'] == 'mock-model')
        compact_key = next(m['id'] for m in models if m['model_id'] == 'compact-model')

        # --- 创建会话 / 列表 ---
        resp = session.post(f'{base}/api/ai/chat/create', json={'biz_type': 'ebook', 'biz_id': 'book-1', 'title': '初始标题'})
        chat_id = resp.json()['chat_id']
        resp = session.post(f'{base}/api/ai/chat/list', json={'biz_type': 'ebook', 'biz_id': 'book-1'})
        data = resp.json()
        assert data['success'] is True and len(data['chats']) == 1, resp.text
        assert data['chats'][0]['id'] == chat_id

        # --- send：思考等级透传 + 工具调用循环 + 流式输出 ---
        MOCK_CAPTURED.clear()
        MOCK_SCRIPT.append({
            'reasonings': ['让我查一下'],
            'tool_calls': [{
                'call_id': 'call_1', 'name': 'get_chapter_content',
                'arguments': '{"chapter_no":1}',
            }],
        })
        MOCK_SCRIPT.append({'deltas': ['你好', '，这是回答']})

        with session.post(f'{base}/api/ai/chat/send', json={
            'biz_type': 'ebook', 'chat_id': chat_id, 'model_key': model_key, 'content': '第一章讲了什么？',
            'thinking': 'high',
        }, stream=True) as resp:
            assert resp.status_code == 200
            assert resp.headers['Content-Type'].startswith('text/event-stream')
            events = read_sse_events(resp)

        assert events[0] == 'user_message', events
        assert 'reasoning' in events, events
        assert 'tool_start' in events and 'tool_end' in events, events
        assert events[-1] == 'done', events

        # 请求体校验：responses 协议 + reasoning 等级 + 扁平 tools
        assert len(MOCK_CAPTURED) == 2, len(MOCK_CAPTURED)
        first, second = MOCK_CAPTURED
        assert first['model'] == 'mock-model'
        assert first['reasoning']['effort'] == 'high', first.get('reasoning')
        assert first['store'] is False
        assert first['tools'][0]['type'] == 'function'
        tool_names = [t['name'] for t in first['tools']]
        assert 'get_chapter_content' in tool_names, tool_names
        # 第二次上游调用带完整工具链上下文（responses input items）
        kinds = [m.get('role') or m.get('type') for m in second['input']]
        assert kinds == ['user', 'function_call', 'function_call_output'], kinds
        assert second['input'][1]['call_id'] == 'call_1'
        assert second['input'][2]['call_id'] == 'call_1'
        assert '工具执行失败' in second['input'][2]['output']

        # --- 落盘验证：会话文件与索引（按功能名分目录） ---
        biz_dir = os.path.join(root_path, 'aichat', 'ebook')
        chat_file = os.path.join(biz_dir, f'{chat_id}.json')
        assert os.path.exists(chat_file), os.listdir(biz_dir)
        index_file = os.path.join(biz_dir, 'index.json')
        assert os.path.exists(index_file), os.listdir(biz_dir)
        with open(chat_file, encoding='utf-8') as f:
            stored = json.load(f)
        assert [m['role'] for m in stored['messages']] == ['user', 'assistant', 'tool', 'assistant']
        assert stored['title'] == '初始标题'
        assert stored['messages'][-1]['content'] == '你好，这是回答'
        assert stored['messages'][-1]['model_key'] == model_key
        assert stored['messages'][1]['tool_calls'][0]['id'] == 'call_1'
        assert stored['context_tokens'] > 0

        # --- 不传 thinking 时请求体无 effort 字段 ---
        MOCK_CAPTURED.clear()
        MOCK_SCRIPT.append({'deltas': ['好的']})
        with session.post(f'{base}/api/ai/chat/send', json={
            'biz_type': 'ebook', 'chat_id': chat_id, 'model_key': model_key, 'content': '不用思考',
        }, stream=True) as resp:
            assert 'done' in read_sse_events(resp)
        assert MOCK_CAPTURED[0].get('reasoning', {}).get('effort') is None, MOCK_CAPTURED[0].get('reasoning')

        # --- messages 分页 ---
        resp = session.post(f'{base}/api/ai/chat/messages', json={'biz_type': 'ebook', 'chat_id': chat_id, 'limit': 3})
        data = resp.json()
        assert data['success'] is True and len(data['messages']) == 3, resp.text
        assert data['has_more_before'] is True and data['has_more_after'] is False
        first_id = data['messages'][0]['id']
        resp = session.post(f'{base}/api/ai/chat/messages', json={
            'biz_type': 'ebook', 'chat_id': chat_id, 'before_id': first_id, 'limit': 10,
        })
        assert len(resp.json()['messages']) == 3

        # --- 重命名 ---
        resp = session.post(f'{base}/api/ai/chat/rename', json={'biz_type': 'ebook', 'chat_id': chat_id, 'title': '自定义标题'})
        assert resp.json()['success'] is True
        resp = session.post(f'{base}/api/ai/chat/list', json={'biz_type': 'ebook', 'biz_id': 'book-1'})
        assert resp.json()['chats'][0]['title'] == '自定义标题'

        # --- 图片随消息落盘，删除会话时一并清理 ---
        png = ('data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJ'
               'AAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==')
        MOCK_SCRIPT.append({'deltas': ['收到图片']})
        with session.post(f'{base}/api/ai/chat/send', json={
            'biz_type': 'ebook', 'chat_id': chat_id, 'model_key': model_key, 'content': '看图', 'images': [png],
        }, stream=True) as resp:
            assert 'done' in read_sse_events(resp)
        images_dir = os.path.join(biz_dir, 'images')
        saved = os.listdir(images_dir) if os.path.isdir(images_dir) else []
        assert len(saved) == 1, saved

        # --- 删除 ---
        resp = session.post(f'{base}/api/ai/chat/delete', json={'biz_type': 'ebook', 'chat_id': chat_id})
        assert resp.json()['success'] is True
        assert not os.path.exists(chat_file)
        assert not os.listdir(images_dir), os.listdir(images_dir)
        resp = session.post(f'{base}/api/ai/chat/list', json={'biz_type': 'ebook', 'biz_id': 'book-1'})
        assert resp.json()['chats'] == []

        # --- 首轮 AI 标题生成（并行后台、最低思考等级；主流程结束后落盘） ---
        MOCK_TITLE_SCRIPT.append({'deltas': ['自动', '生成标题']})
        resp = session.post(f'{base}/api/ai/chat/create', json={'biz_type': 'ebook', 'biz_id': 'book-1'})
        chat2 = resp.json()['chat_id']
        with session.post(f'{base}/api/ai/chat/send', json={
            'biz_type': 'ebook', 'chat_id': chat2, 'model_key': model_key, 'content': '帮我总结第三章的要点',
        }, stream=True) as resp:
            assert 'done' in read_sse_events(resp)
        chat2_file = os.path.join(biz_dir, f'{chat2}.json')
        title = None
        for _ in range(40):
            with open(chat2_file, encoding='utf-8') as f:
                title = json.load(f)['title']
            if title == '自动生成标题':
                break
            time.sleep(0.5)
        assert title == '自动生成标题', title
        # 标题请求应使用最低思考等级（none）
        title_calls = [c for c in MOCK_CAPTURED if any('会话标题' in str(i.get('content', '')) for i in c.get('input', []))]
        assert title_calls and title_calls[-1]['reasoning']['effort'] == 'none', title_calls[-1:]

        # --- 上下文自动压缩（pi 逻辑）：usage 触发阈值 -> 摘要替换旧历史 ---
        MOCK_CAPTURED.clear()
        resp = session.post(f'{base}/api/ai/chat/create', json={'biz_type': 'ebook', 'biz_id': 'book-1'})
        chat3 = resp.json()['chat_id']
        big = 'A' * 30000
        big_reply = 'R' * 30000
        # 三轮大消息：mock 返回 usage.total_tokens=50000 超过 compact-model 阈值(52000-16384)
        for i in range(3):
            # 第三轮的 AI 回复也放大，使第四轮的切点落在第二轮用户消息（非分裂轮次）
            MOCK_SCRIPT.append({
                'deltas': [big_reply if i == 2 else '收到'],
                'usage': {'input_tokens': 49000, 'output_tokens': 1000, 'total_tokens': 50000},
            })
            with session.post(f'{base}/api/ai/chat/send', json={
                'biz_type': 'ebook', 'chat_id': chat3, 'model_key': compact_key, 'content': big,
            }, stream=True) as resp:
                events = read_sse_events(resp)
            assert 'compact_start' not in events, events
        with open(os.path.join(biz_dir, f'{chat3}.json'), encoding='utf-8') as f:
            stored = json.load(f)
        assert [m['role'] for m in stored['messages']] == ['user', 'assistant'] * 3
        assert stored['context_tokens'] == 50000

        # 第四轮触发压缩：切点保留近两轮，摘要替换最早一轮
        MOCK_CAPTURED.clear()
        with session.post(f'{base}/api/ai/chat/send', json={
            'biz_type': 'ebook', 'chat_id': chat3, 'model_key': compact_key, 'content': '现在总结一下',
        }, stream=True) as resp:
            events = read_sse_events(resp)
        assert 'compact_start' in events and 'compact_end' in events, events
        assert events[-1] == 'done', events
        # 摘要请求使用 pi 提示词（instructions 以 summarization system prompt 开头）
        summary_calls = [c for c in MOCK_CAPTURED if str(c.get('instructions', '')).startswith('You are a ' + SUMMARIZATION_MARKER)]
        assert summary_calls, [str(c.get('instructions', ''))[:80] for c in MOCK_CAPTURED]
        with open(os.path.join(biz_dir, f'{chat3}.json'), encoding='utf-8') as f:
            stored = json.load(f)
        roles = [m['role'] for m in stored['messages']]
        # 压缩替换最早一轮(user+assistant)，保留近两轮，末尾是本轮的 user+assistant
        assert roles == ['compact', 'user', 'assistant', 'user', 'assistant', 'user', 'assistant'], roles
        assert stored['messages'][0]['content'] == '压缩后的摘要'
        # 压缩后的主请求把摘要作为用户消息注入（<summary> 包装）
        main_calls = [c for c in MOCK_CAPTURED if not str(c.get('instructions', '')).startswith('You are a ' + SUMMARIZATION_MARKER)]
        assert any('<summary>' in str(i.get('content', '')) for i in main_calls[-1]['input']), main_calls[-1]['input']
        # 压缩后 context_tokens 清零重计：第四轮主调用 mock 默认 usage 的 total_tokens
        assert stored['context_tokens'] == 120

        # --- 上游挂死（连得上但永不回包）：建流超时必须发 error，不能静默 ---
        # 先发一轮把会话转为非首轮，避免并行的标题生成请求抢走挂死标记
        resp = session.post(f'{base}/api/ai/chat/create', json={'biz_type': 'ebook', 'biz_id': 'book-1'})
        chat4 = resp.json()['chat_id']
        MOCK_SCRIPT.append({'deltas': ['预热']})
        with session.post(f'{base}/api/ai/chat/send', json={
            'biz_type': 'ebook', 'chat_id': chat4, 'model_key': model_key, 'content': '预热',
        }, stream=True) as resp:
            assert 'done' in read_sse_events(resp)

        MOCK_HANG.append(True)
        hang_start = time.time()
        with session.post(f'{base}/api/ai/chat/send', json={
            'biz_type': 'ebook', 'chat_id': chat4, 'model_key': model_key, 'content': '供应商挂死',
        }, stream=True) as resp:
            events = read_sse_events(resp)
        assert events[0] == 'user_message', events
        # 错误必须是最后一个事件：不能再补一个 TOOL_ROUNDS_EXCEEDED 掩盖真实原因
        assert events[-1] == 'error', events
        assert time.time() - hang_start < 60, '建流超时未生效'

        # --- 上游返回 401：error 事件要带上原始报错，用户才能分辨密钥错误 / 限流 / 5xx ---
        resp = session.post(f'{base}/api/ai/chat/create', json={'biz_type': 'ebook', 'biz_id': 'book-1'})
        chat5 = resp.json()['chat_id']
        MOCK_SCRIPT.append({'deltas': ['预热']})
        with session.post(f'{base}/api/ai/chat/send', json={
            'biz_type': 'ebook', 'chat_id': chat5, 'model_key': model_key, 'content': '预热',
        }, stream=True) as resp:
            assert 'done' in read_sse_events(resp)

        MOCK_ERROR.append((401, {'error': {'message': 'Invalid token', 'type': 'invalid_request_error'}}))
        with session.post(f'{base}/api/ai/chat/send', json={
            'biz_type': 'ebook', 'chat_id': chat5, 'model_key': model_key, 'content': '错误密钥',
        }, stream=True) as resp:
            frames = read_sse_frames(resp)
        event, data = frames[-1]
        assert event == 'error', frames
        assert data['fail_code'] == 'AI_CALL_FAILED', data
        detail = data.get('detail', '')
        # 只暴露状态码 + 上游错误消息，不带 genai 的内部描述与原始 JSON
        assert detail.startswith('HTTP 401'), detail
        assert 'Invalid token' in detail, detail
        assert 'Body:' not in detail and '{"' not in detail, detail
    finally:
        server.shutdown()


# run_tests 会以 (session, root_path) 调用本模块的全部测试函数
if __name__ == '__main__':
    run_tests(test_ai_chat_api)

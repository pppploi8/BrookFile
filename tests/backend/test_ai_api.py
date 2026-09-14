import socket
import threading
import time

import requests
from test_utils import run_tests, BASE_URL


def test_ai_config(session, root_path):
    # --- presets: 供应商类型清单 ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/presets')
    data = resp.json()
    assert data['success'] is True
    type_ids = {p['type_id'] for p in data['presets']}
    assert {'openai_completions', 'openai_responses', 'deepseek', 'anthropic', 'gemini', 'ollama'} <= type_ids
    # 三个协议入口置顶；与 OpenAI (Chat Completions) 重复的 custom 已移除
    assert [p['type_id'] for p in data['presets'][:3]] == ['openai_completions', 'openai_responses', 'anthropic']
    assert 'custom' not in type_ids
    deepseek_preset = next(p for p in data['presets'] if p['type_id'] == 'deepseek')
    assert deepseek_preset['default_base_url'] == 'https://api.deepseek.com/v1/'
    assert deepseek_preset['requires_api_key'] is True
    ollama_preset = next(p for p in data['presets'] if p['type_id'] == 'ollama')
    assert ollama_preset['requires_api_key'] is False

    # --- create: 未知供应商类型 ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/create', json={
        'name': 'X', 'provider_type': 'no-such-type', 'api_key': 'sk-test'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PROVIDER_TYPE_INVALID'

    # --- create: invalid base_url ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/create', json={
        'name': 'OpenAI', 'provider_type': 'openai_completions',
        'base_url': 'no-scheme.com', 'api_key': 'sk-test'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'BASE_URL_INVALID'

    # --- create: empty name ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/create', json={
        'name': '  ', 'provider_type': 'deepseek', 'api_key': 'sk-test'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PARAM_INVALID'

    # --- create: 需要密钥的预设密钥为空 ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/create', json={
        'name': 'DeepSeek', 'provider_type': 'deepseek', 'api_key': ''
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PARAM_INVALID'

    # --- create: ollama 无需密钥 ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/create', json={
        'name': 'Local', 'provider_type': 'ollama'
    })
    data = resp.json()
    assert data['success'] is True
    ollama_id = data['provider_id']

    # --- create: invalid proxy ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/create', json={
        'name': 'OpenAI', 'provider_type': 'openai_completions',
        'base_url': 'http://127.0.0.1:9/v1', 'api_key': 'sk-test',
        'proxy': 'socks5://127.0.0.1:1080'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PROXY_INVALID'

    # --- create: success, trailing slash trimmed ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/create', json={
        'name': 'OpenAI', 'provider_type': 'openai_responses',
        'base_url': 'http://127.0.0.1:9/v1/', 'api_key': 'sk-test'
    })
    data = resp.json()
    assert data['success'] is True
    provider_id = data['provider_id']

    # --- atomic create: provider + models saved together ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/create', json={
        'name': 'Atomic', 'provider_type': 'openai_completions',
        'base_url': 'http://127.0.0.1:9/v1', 'api_key': 'sk-atomic',
        'proxy': 'http://127.0.0.1:8080',
        'models': [
            {'model_id': 'model-a', 'supports_vision': True, 'context_length': 1000, 'max_output_tokens': 500},
            {'model_id': 'model-b'},
        ]
    })
    data = resp.json()
    assert data['success'] is True
    atomic_id = data['provider_id']

    # --- atomic create: duplicate within one call -> whole thing rolls back ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/create', json={
        'name': 'AtomicDup', 'provider_type': 'openai_completions',
        'base_url': 'http://127.0.0.1:9/v1', 'api_key': 'sk-atomic',
        'models': [
            {'model_id': 'dup'},
            {'model_id': 'dup'},
        ]
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'MODEL_DUPLICATE'

    # verify atomic provider persisted, rollback provider absent
    resp = session.post(f'{BASE_URL}/api/ai/provider/list')
    atomic = next(p for p in resp.json()['providers'] if p['id'] == atomic_id)
    assert atomic['name'] == 'Atomic'
    assert atomic['provider_type'] == 'openai_completions'
    assert atomic['proxy'] == 'http://127.0.0.1:8080'
    assert [m['model_id'] for m in atomic['models']] == ['model-a', 'model-b']
    assert all(p['name'] != 'AtomicDup' for p in resp.json()['providers'])

    # --- create: 未传 base_url 时使用预设默认地址 ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/list')
    ollama_info = next(p for p in resp.json()['providers'] if p['id'] == ollama_id)
    assert ollama_info['base_url'] == 'http://localhost:11434'
    assert ollama_info['has_api_key'] is False

    # --- fetch models: api_key 为空但提供 provider_id 时回退已存储密钥 ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/fetch_models', json={
        'provider_type': 'openai_completions',
        'base_url': 'http://127.0.0.1:9/v1',
        'api_key': '',
        'provider_id': atomic_id
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'AI_MODEL_LIST_FAILED'  # 非 PARAM_INVALID，说明回退密钥成功

    # --- fetch models: proxy 为空但有 provider_id 时回退已存储代理并实际使用（代理不可达 -> AI_MODEL_LIST_FAILED） ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/fetch_models', json={
        'provider_type': 'openai_completions',
        'base_url': 'http://127.0.0.1:9/v1',
        'api_key': 'sk-test',
        'proxy': '',
        'provider_id': atomic_id
    })
    assert resp.json()['fail_code'] == 'AI_MODEL_LIST_FAILED'

    # --- fetch models: 非法代理 ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/fetch_models', json={
        'provider_type': 'openai_completions',
        'base_url': 'http://127.0.0.1:9/v1',
        'api_key': 'sk-test',
        'proxy': 'socks5://127.0.0.1:1080'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PROXY_INVALID'

    # --- fetch models: 未知供应商类型 ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/fetch_models', json={
        'provider_type': 'no-such-type', 'api_key': 'k'
    })
    assert resp.json()['fail_code'] == 'PROVIDER_TYPE_INVALID'

    # --- fetch models: 上游挂死（accept 后永不回包）必须在超时内失败，不能永久挂住 ---
    hang_srv = socket.socket()
    hang_srv.bind(('127.0.0.1', 0))
    hang_srv.listen(5)
    hang_port = hang_srv.getsockname()[1]
    held_conns = []

    def _hold_connections():
        while True:
            try:
                conn, _ = hang_srv.accept()
            except OSError:
                return
            held_conns.append(conn)  # 保持连接、不回任何数据

    threading.Thread(target=_hold_connections, daemon=True).start()
    try:
        started = time.time()
        resp = session.post(f'{BASE_URL}/api/ai/provider/fetch_models', json={
            'provider_type': 'openai_completions',
            'base_url': f'http://127.0.0.1:{hang_port}/v1',
            'api_key': 'sk-test',
        }, timeout=90)
        elapsed = time.time() - started
        assert resp.json()['fail_code'] == 'AI_MODEL_LIST_FAILED'
        assert elapsed < 60, f'模型列表请求未在超时内返回，耗时 {elapsed:.1f}s'
    finally:
        hang_srv.close()
        for conn in held_conns:
            conn.close()

    # --- update provider + 整体对账模型：改一个、删一个、加一个 ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/list')
    info = next(p for p in resp.json()['providers'] if p['id'] == atomic_id)
    model_a = next(m for m in info['models'] if m['model_id'] == 'model-a')
    resp = session.post(f'{BASE_URL}/api/ai/provider/update', json={
        'id': atomic_id,
        'name': 'Atomic-Upd',
        'provider_type': 'openai_completions',
        'base_url': 'http://127.0.0.1:9/v1',
        'api_key': '',
        'models': [
            {'id': model_a['id'], 'model_id': 'model-a-rev', 'supports_vision': True, 'context_length': 100, 'max_output_tokens': 50},
            {'model_id': 'model-c'},
        ]
    })
    assert resp.json()['success'] is True
    resp = session.post(f'{BASE_URL}/api/ai/provider/list')
    info = next(p for p in resp.json()['providers'] if p['id'] == atomic_id)
    assert [m['model_id'] for m in info['models']] == ['model-a-rev', 'model-c']
    rev = next(m for m in info['models'] if m['model_id'] == 'model-a-rev')
    assert rev['supports_vision'] is True
    assert rev['context_length'] == 100

    # --- update 对账中模型重复 -> 整体回滚，供应商信息不变 ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/update', json={
        'id': atomic_id,
        'name': 'Atomic-Bad',
        'provider_type': 'openai_completions',
        'base_url': 'http://127.0.0.1:9/v1',
        'models': [
            {'model_id': 'dup'},
            {'model_id': 'dup'},
        ]
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'MODEL_DUPLICATE'
    resp = session.post(f'{BASE_URL}/api/ai/provider/list')
    info = next(p for p in resp.json()['providers'] if p['id'] == atomic_id)
    assert info['name'] == 'Atomic-Upd'  # 回滚，名称未变

    # --- fetch models: unreachable provider -> AI_MODEL_LIST_FAILED ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/fetch_models', json={
        'provider_type': 'openai_completions',
        'base_url': 'http://127.0.0.1:9/v1',
        'api_key': 'sk-test'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'AI_MODEL_LIST_FAILED'

    # --- fetch models: empty api_key ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/fetch_models', json={
        'provider_type': 'openai_completions',
        'base_url': 'http://127.0.0.1:9/v1',
        'api_key': ''
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PARAM_INVALID'

    # --- 经供应商更新对账添加模型：一个带完整字段、一个用默认值 ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/update', json={
        'id': provider_id,
        'name': 'OpenAI',
        'provider_type': 'openai_responses',
        'base_url': 'http://127.0.0.1:9/v1',
        'models': [
            {'model_id': 'gpt-4o', 'supports_vision': True, 'context_length': 128000, 'max_output_tokens': 4096},
            {'model_id': 'gpt-4o-mini'},
        ]
    })
    assert resp.json()['success'] is True

    # --- list: verify provider & models, api key not returned ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/list')
    data = resp.json()
    assert data['success'] is True
    provider = next(p for p in data['providers'] if p['id'] == provider_id)
    assert provider['name'] == 'OpenAI'
    assert provider['provider_type'] == 'openai_responses'
    assert provider['base_url'] == 'http://127.0.0.1:9/v1'
    assert provider['has_api_key'] is True
    assert 'api_key' not in provider
    assert len(provider['models']) == 2
    gpt4 = next(m for m in provider['models'] if m['model_id'] == 'gpt-4o')
    assert gpt4['supports_vision'] is True
    assert gpt4['context_length'] == 128000
    assert gpt4['max_output_tokens'] == 4096
    mini = next(m for m in provider['models'] if m['model_id'] == 'gpt-4o-mini')
    assert mini['supports_vision'] is False
    assert mini['context_length'] == 200000
    assert mini['max_output_tokens'] == 65536
    model_id = gpt4['id']

    # --- update provider: keep api key unchanged ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/update', json={
        'id': provider_id,
        'name': 'OpenAI-Pro',
        'provider_type': 'openai_responses',
        'base_url': 'http://127.0.0.1:9/v1',
        'api_key': ''
    })
    assert resp.json()['success'] is True
    resp = session.post(f'{BASE_URL}/api/ai/provider/list')
    updated = next(p for p in resp.json()['providers'] if p['id'] == provider_id)
    assert updated['name'] == 'OpenAI-Pro'
    assert updated['has_api_key'] is True

    # --- update provider: change api key ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/update', json={
        'id': provider_id,
        'name': 'OpenAI-Pro',
        'provider_type': 'openai_responses',
        'base_url': 'http://127.0.0.1:9/v1',
        'api_key': 'sk-new'
    })
    assert resp.json()['success'] is True

    # --- update provider: nonexistent ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/update', json={
        'id': 'nonexistent',
        'name': 'X',
        'provider_type': 'openai_responses',
        'base_url': 'http://127.0.0.1:9/v1'
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PROVIDER_NOT_FOUND'

    # --- 经供应商更新对账修改模型 ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/update', json={
        'id': provider_id,
        'name': 'OpenAI-Pro',
        'provider_type': 'openai_responses',
        'base_url': 'http://127.0.0.1:9/v1',
        'models': [
            {'id': model_id, 'model_id': 'gpt-4o-rev', 'supports_vision': False, 'context_length': 200, 'max_output_tokens': 100},
            {'model_id': 'gpt-4o-mini'},
        ]
    })
    assert resp.json()['success'] is True
    resp = session.post(f'{BASE_URL}/api/ai/provider/list')
    updated_model = next(
        m for p in resp.json()['providers'] if p['id'] == provider_id
        for m in p['models'] if m['id'] == model_id
    )
    assert updated_model['model_id'] == 'gpt-4o-rev'
    assert updated_model['supports_vision'] is False
    assert updated_model['context_length'] == 200

    # --- 对账中模型 id 与已有重复 -> MODEL_DUPLICATE ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/update', json={
        'id': provider_id,
        'name': 'OpenAI-Pro',
        'provider_type': 'openai_responses',
        'base_url': 'http://127.0.0.1:9/v1',
        'models': [
            {'id': model_id, 'model_id': 'gpt-4o-mini'},
            {'model_id': 'gpt-4o-mini'},
        ]
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'MODEL_DUPLICATE'

    # --- 对账中模型 id 不存在 -> MODEL_NOT_FOUND ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/update', json={
        'id': provider_id,
        'name': 'OpenAI-Pro',
        'provider_type': 'openai_responses',
        'base_url': 'http://127.0.0.1:9/v1',
        'models': [
            {'id': model_id, 'model_id': 'gpt-4o-rev'},
            {'id': 'nonexistent-model-id', 'model_id': 'x'},
        ]
    })
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'MODEL_NOT_FOUND'

    # --- delete model ---
    resp = session.post(f'{BASE_URL}/api/ai/model/delete', json={'id': model_id})
    assert resp.json()['success'] is True
    resp = session.post(f'{BASE_URL}/api/ai/model/delete', json={'id': model_id})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'MODEL_NOT_FOUND'

    # --- list: only one model left ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/list')
    provider_after = next(p for p in resp.json()['providers'] if p['id'] == provider_id)
    assert len(provider_after['models']) == 1

    # --- delete provider cascades models ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/delete', json={'id': provider_id})
    assert resp.json()['success'] is True
    resp = session.post(f'{BASE_URL}/api/ai/provider/list')
    assert all(p['id'] != provider_id for p in resp.json()['providers'])

    # --- delete provider again: not found ---
    resp = session.post(f'{BASE_URL}/api/ai/provider/delete', json={'id': provider_id})
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'PROVIDER_NOT_FOUND'

    # --- model ops on deleted provider's cascade: model no longer accessible via list ---
    # (create provider + model for isolation check)
    resp = session.post(f'{BASE_URL}/api/ai/provider/create', json={
        'name': 'Isolation',
        'provider_type': 'openai_completions',
        'base_url': 'http://127.0.0.1:9/v1',
        'api_key': 'k',
        'models': [{'model_id': 'm1'}]
    })
    p2 = resp.json()['provider_id']
    resp = session.post(f'{BASE_URL}/api/ai/provider/list')
    assert any(p['id'] == p2 for p in resp.json()['providers'])

    # --- not logged in ---
    session.post(f'{BASE_URL}/api/auth/logout')
    resp = session.post(f'{BASE_URL}/api/ai/provider/list')
    data = resp.json()
    assert data['success'] is False
    assert data['fail_code'] == 'NOT_LOGGED_IN'


if __name__ == '__main__':
    run_tests(test_ai_config)

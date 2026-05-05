import os
import re
import sys

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
FRONTEND_SRC = os.path.join(SCRIPT_DIR, '..', '..', 'frontend', 'src')
EN_FILE = os.path.join(FRONTEND_SRC, 'i18n', 'locales', 'en.ts')
ZH_FILE = os.path.join(FRONTEND_SRC, 'i18n', 'locales', 'zh.ts')
API_DIR = os.path.join(SCRIPT_DIR, '..', '..', 'api')

def flatten_keys(obj, prefix=''):
    keys = set()
    for key, value in obj.items():
        full = f'{prefix}{key}' if not prefix else f'{prefix}.{key}'
        if isinstance(value, dict):
            keys |= flatten_keys(value, full)
        else:
            keys.add(full)
    return keys

def parse_ts_file(filepath):
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()
    content = re.sub(r'//.*$', '', content, flags=re.MULTILINE)
    content = re.sub(r'/\*.*?\*/', '', content, flags=re.DOTALL)
    start = content.index('{', content.index('export default'))
    obj = _parse_object(content, start)[0]
    return flatten_keys(obj)

def _parse_object(content, pos):
    pos = _skip_ws(content, pos)
    assert content[pos] == '{', f"Expected '{{' at {pos}, got '{content[pos]}'"
    pos += 1
    result = {}
    while True:
        pos = _skip_ws(content, pos)
        if content[pos] == '}':
            return result, pos + 1
        key, pos = _read_key(content, pos)
        pos = _skip_ws(content, pos)
        assert content[pos] == ':', f"Expected ':' at {pos}"
        pos += 1
        pos = _skip_ws(content, pos)
        if content[pos] == '{':
            value, pos = _parse_object(content, pos)
        else:
            value, pos = _read_value(content, pos)
        result[key] = value
        pos = _skip_ws(content, pos)
        if content[pos] == ',':
            pos += 1

def _skip_ws(content, pos):
    while pos < len(content) and content[pos] in ' \t\n\r':
        pos += 1
    return pos

def _read_key(content, pos):
    pos = _skip_ws(content, pos)
    if content[pos] == "'":
        end = content.index("'", pos + 1)
        return content[pos+1:end], end + 1
    if content[pos] == '"':
        end = content.index('"', pos + 1)
        return content[pos+1:end], end + 1
    end = pos
    while end < len(content) and (content[end].isalnum() or content[end] == '_'):
        end += 1
    return content[pos:end], end

def _read_value(content, pos):
    pos = _skip_ws(content, pos)
    if content[pos] == "'":
        end = pos + 1
        while end < len(content):
            if content[end] == '\\':
                end += 2
                continue
            if content[end] == "'":
                return content[pos+1:end], end + 1
            end += 1
    if content[pos] == '"':
        end = pos + 1
        while end < len(content):
            if content[end] == '\\':
                end += 2
                continue
            if content[end] == '"':
                return content[pos+1:end], end + 1
            end += 1
    if content[pos:].startswith('true'):
        return True, pos + 4
    if content[pos:].startswith('false'):
        return False, pos + 5
    return '', pos

def _is_valid_i18n_key(key):
    if not key or len(key) < 2:
        return False
    if key.startswith('#') or key.startswith('/') or key.startswith('@'):
        return False
    if key.startswith('Content-') or key.startswith('content-'):
        return False
    if '.' not in key:
        return False
    if re.match(r'^[a-z]+$', key):
        return False
    return True

def find_usage_keys():
    used = set()
    for root, dirs, files in os.walk(FRONTEND_SRC):
        for f in files:
            if not f.endswith(('.ts', '.vue')):
                continue
            fp = os.path.join(root, f)
            if fp in (EN_FILE, ZH_FILE):
                continue
            with open(fp, 'r', encoding='utf-8') as fh:
                content = fh.read()
            for m in re.finditer(r"t\(['\"]([^'\"]+)['\"]\s*(?:,|\))", content):
                key = m.group(1)
                if _is_valid_i18n_key(key):
                    used.add(key)
            for m in re.finditer(r"__key:\s*'([^']+)'", content):
                key = m.group(1)
                if _is_valid_i18n_key(key):
                    used.add(key)
            for m in re.finditer(r'__key:\s*"([^"]+)"', content):
                key = m.group(1)
                if _is_valid_i18n_key(key):
                    used.add(key)
            for m in re.finditer(r'__key:\s*`([^`]+)`', content):
                key = m.group(1)
                if _is_valid_i18n_key(key) and '${' not in key:
                    used.add(key)
            for m in re.finditer(r"__key:\s*[^'\"]*?\?\s*'([a-zA-Z]+\.[a-zA-Z]+)'", content):
                key = m.group(1)
                if _is_valid_i18n_key(key):
                    used.add(key)
            for m in re.finditer(r"__key:\s*[^'\"]*?\?\s*\"([a-zA-Z]+\.[a-zA-Z]+)\"", content):
                key = m.group(1)
                if _is_valid_i18n_key(key):
                    used.add(key)
            for m in re.finditer(r":\s*'([a-zA-Z]+\.[a-zA-Z]+[a-zA-Z.]*)'", content):
                key = m.group(1)
                if _is_valid_i18n_key(key):
                    used.add(key)
    return used

def find_api_fail_codes():
    codes = set()
    for f in os.listdir(API_DIR):
        if not f.endswith('.md'):
            continue
        fp = os.path.join(API_DIR, f)
        with open(fp, 'r', encoding='utf-8') as fh:
            content = fh.read()
        for m in re.finditer(r'`([A-Z][A-Z0-9_]+)`', content):
            code = m.group(1)
            if any(re.search(r'\b' + kw + r'\b', code) for kw in ['POST', 'GET', 'PUT', 'DELETE', 'PATCH', 'HTTP', 'JSON', 'API', 'SQL', 'SSE']):
                continue
            codes.add(f'errors.{code}')
    return codes

def main():
    errors = []

    en_keys = parse_ts_file(EN_FILE)
    zh_keys = parse_ts_file(ZH_FILE)

    en_only = sorted(en_keys - zh_keys)
    zh_only = sorted(zh_keys - en_keys)

    if en_only:
        errors.append(f"Keys only in en.ts ({len(en_only)}):\n  " + "\n  ".join(en_only))
    if zh_only:
        errors.append(f"Keys only in zh.ts ({len(zh_only)}):\n  " + "\n  ".join(zh_only))

    all_keys = en_keys | zh_keys
    used_keys = find_usage_keys()
    api_fail_codes = find_api_fail_codes()

    unused = sorted(all_keys - used_keys - api_fail_codes)
    if unused:
        errors.append(f"Unused keys (not in code or API docs, {len(unused)}):\n  " + "\n  ".join(unused))

    missing = sorted((used_keys | api_fail_codes) - all_keys)
    if missing:
        errors.append(f"Used keys missing from translations ({len(missing)}):\n  " + "\n  ".join(missing))

    if errors:
        print("FAIL\n")
        for e in errors:
            print(e)
            print()
        sys.exit(1)
    else:
        en_count = len(en_keys)
        zh_count = len(zh_keys)
        used_count = len(used_keys | api_fail_codes)
        print(f"PASS - {en_count} en keys, {zh_count} zh keys, {used_count} used keys checked")

if __name__ == '__main__':
    main()

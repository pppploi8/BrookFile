import re
zh = open(r'C:\Users\pppploi8\Documents\Code\RustFile\frontend\src\i18n\locales\zh.ts', encoding='utf-8').read()
en = open(r'C:\Users\pppploi8\Documents\Code\RustFile\frontend\src\i18n\locales\en.ts', encoding='utf-8').read()
def extract_keys(content):
    keys = set()
    for m in re.finditer(r"'([a-zA-Z0-9_]+)'\s*:", content):
        keys.add(m.group(1))
    return keys
zh_keys = extract_keys(zh)
en_keys = extract_keys(en)
only_zh = zh_keys - en_keys
only_en = en_keys - zh_keys
if only_zh:
    print('Keys only in zh:')
    for k in sorted(only_zh):
        print(f'  {k}')
if only_en:
    print('Keys only in en:')
    for k in sorted(only_en):
        print(f'  {k}')
if not only_zh and not only_en:
    print('Keys are identical')

# 笔记本模块接口

## 1. 获取笔记本列表接口

**路径**：POST /api/notebook/list

**功能**：获取当前用户的所有笔记本。需要登录后才能访问。

**请求参数**：无

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "notebooks": [
    {
      "id": "uuid-string",
      "name": "工作笔记",
      "description": "日常工作记录",
      "path": "notes",
      "encrypted": false,
      "created_at": "2026-01-15T10:30:00Z",
      "updated_at": "2026-01-15T10:30:00Z"
    }
  ]
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| success | boolean | 是否成功 |
| notebooks | array | 笔记本列表，按名称升序排列 |
| notebooks[].id | string | 笔记本ID |
| notebooks[].name | string | 笔记本名称 |
| notebooks[].description | string | 笔记本说明 |
| notebooks[].path | string | 存储路径（相对路径，不以 `/` 开头） |
| notebooks[].encrypted | boolean | 是否加密（以前端传入值存入数据库，读取时以数据库值为准） |
| notebooks[].created_at | string | 创建时间 |
| notebooks[].updated_at | string | 更新时间 |

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `INTERNAL_ERROR` | 内部错误 |

## 2. 创建笔记本接口

**路径**：POST /api/notebook/create

**功能**：创建新笔记本，路径必须为空目录。需要登录后才能访问。支持创建加密笔记本，创建时需同时传入签名内容。

**请求参数**：
```json
{
  "name": "工作笔记",
  "description": "日常工作记录",
  "path": "notes",
  "encrypted": false
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| name | string | 是 | 笔记本名称 |
| description | string | 否 | 笔记本说明 |
| path | string | 是 | 存储路径（必须在用户 root_path 下，相对路径不以 `/` 开头） |
| encrypted | boolean | 否 | 是否加密，默认 false |
| signature | string | 条件必填 | 加密笔记本的签名文件内容（JSON 字符串），encrypted=true 时必传 |

**signature 字段格式**（encrypted=true 时）：
```json
{
  "salt": "base64-encoded-salt",
  "iv": "base64-encoded-iv",
  "rounds": 100000,
  "signature": "base64-encoded-signature"
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| salt | string | PBKDF2 盐值（Base64） |
| iv | string | AES-CBC 初始向量（Base64） |
| rounds | number | PBKDF2 迭代次数 |
| signature | string | AES-CBC 加密后的验证签名（Base64） |

**说明**：当 encrypted=true 时，后端会将 signature 内容写入笔记本目录下的 `.notebook.sig` 文件。

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "id": "uuid-string"
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| id | string | 新创建的笔记本ID |

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `INVALID_PARAM` | encrypted=true 但 signature 缺失或格式不合法，或路径为空 |
| `INVALID_FILE_PATH` | 路径不安全或不在用户 root_path 下 |
| `PATH_NOT_FOUND` | 路径不是目录 |
| `PATH_NOT_EMPTY` | 路径非空，提示使用"打开笔记本"功能 |
| `DUPLICATE_NOTEBOOK_PATH` | 该路径已被其他笔记本使用 |
| `NESTED_ENCRYPTED_NOT_ALLOWED` | 父路径已存在加密笔记本，不允许嵌套加密 |
| `FILE_WRITE_ERROR` | 签名文件写入失败 |
| `INTERNAL_ERROR` | 内部错误 |

## 3. 打开笔记本接口

**路径**：POST /api/notebook/open

**功能**：将已有的非空目录注册为笔记本。需要登录后才能访问。加密状态以前端传入值为准。非加密笔记本打开时自动构建全文搜索索引。

**请求参数**：
```json
{
  "name": "工作笔记",
  "description": "从已有目录打开",
  "path": "notes",
  "encrypted": false
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| name | string | 是 | 笔记本名称 |
| description | string | 否 | 笔记本说明 |
| path | string | 是 | 存储路径（必须在用户 root_path 下，相对路径不以 `/` 开头） |
| encrypted | boolean | 否 | 是否加密，默认 false |

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "id": "uuid-string"
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| id | string | 新创建的笔记本ID |

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `INVALID_PARAM` | 参数错误（包括路径为空，或 encrypted=true 但 .notebook.sig 不存在） |
| `INVALID_FILE_PATH` | 路径不安全或不在用户 root_path 下 |
| `PATH_NOT_FOUND` | 路径不存在或不是目录 |
| `DUPLICATE_NOTEBOOK_PATH` | 该路径已被其他笔记本使用 |
| `NESTED_ENCRYPTED_NOT_ALLOWED` | 父路径已存在加密笔记本，不允许嵌套加密 |
| `INTERNAL_ERROR` | 内部错误 |

## 4. 更新笔记本接口

**路径**：POST /api/notebook/update

**功能**：更新笔记本名称和说明。需要登录后才能访问。`path` 和 `encrypted` 不可修改。

**请求参数**：
```json
{
  "id": "uuid-string",
  "name": "新名称",
  "description": "新说明"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| id | string | 是 | 笔记本ID |
| name | string | 是 | 笔记本名称 |
| description | string | 否 | 笔记本说明 |

**返回值**：

**成功响应**：
```json
{
  "success": true
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `NOTEBOOK_NOT_FOUND` | 笔记本不存在 |
| `INVALID_PARAM` | 参数错误 |
| `INTERNAL_ERROR` | 内部错误 |

## 5. 删除笔记本接口

**路径**：POST /api/notebook/delete

**功能**：删除笔记本数据库记录，不删除物理文件。同时删除对应的搜索数据库文件。需要登录后才能访问。

**请求参数**：
```json
{
  "id": "uuid-string"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| id | string | 是 | 笔记本ID |

**返回值**：

**成功响应**：
```json
{
  "success": true
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `NOTEBOOK_NOT_FOUND` | 笔记本不存在 |
| `INTERNAL_ERROR` | 内部错误 |

## 6. 读取笔记接口

**路径**：POST /api/notebook/read_note

**功能**：读取笔记内容并返回内容哈希，用于冲突检测。需要登录后才能访问。

**请求参数**：
```json
{
  "notebook_id": "uuid-string",
  "path": "folder1/note1.md"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| notebook_id | string | 是 | 笔记本ID |
| path | string | 是 | 笔记文件相对路径（相对于笔记本路径） |

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "content": "笔记内容...",
  "hash": "sha256-hash-string"
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| success | boolean | 是否成功 |
| content | string | 笔记文件内容 |
| hash | string | 内容的 SHA-256 哈希值 |

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `NOTEBOOK_NOT_FOUND` | 笔记本不存在 |
| `INVALID_FILE_PATH` | 路径不安全或不在允许范围内 |
| `FILE_NOT_FOUND` | 文件不存在 |
| `FILE_READ_ERROR` | 文件读取失败 |
| `INTERNAL_ERROR` | 内部错误 |

## 7. 保存笔记接口

**路径**：POST /api/notebook/save_note

**功能**：保存笔记内容，通过哈希检测并发编辑冲突。不传 hash 视为新建笔记。需要登录后才能访问。

**请求参数**：
```json
{
  "notebook_id": "uuid-string",
  "path": "folder1/note1.md",
  "content": "更新后的笔记内容...",
  "hash": "之前读取时返回的sha256-hash-string"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| notebook_id | string | 是 | 笔记本ID |
| path | string | 是 | 笔记文件相对路径（相对于笔记本路径） |
| content | string | 是 | 笔记内容 |
| hash | string | 否 | 读取笔记时返回的哈希值；新建笔记时不传 |

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "hash": "sha256-hash-string"
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| hash | string | 保存后文件内容的 SHA-256 哈希值 |

**冲突响应**：
```json
{
  "success": false,
  "fail_code": "CONFLICT_DETECTED",
  "server_content": "服务端当前文件内容",
  "server_hash": "服务端当前文件哈希"
}
```

**说明**：后端检测到冲突时不生成冲突文件，仅返回冲突响应。前端负责调用 `/api/notebook/save_conflict` 保存冲突内容。

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `NOTEBOOK_NOT_FOUND` | 笔记本不存在 |
| `INVALID_FILE_PATH` | 路径不安全或不在允许范围内 |
| `FILE_NOT_FOUND` | 文件不存在（更新已有笔记时） |
| `FILE_READ_ERROR` | 读取当前文件内容失败 |
| `FILE_ALREADY_EXISTS` | 新建笔记时文件已存在 |
| `FILE_WRITE_ERROR` | 文件写入失败（包括父目录创建失败） |
| `CONFLICT_DETECTED` | 检测到编辑冲突，返回服务端内容供前端处理 |
| `INTERNAL_ERROR` | 内部错误 |

## 8. 保存冲突文件接口

**路径**：POST /api/notebook/save_conflict

**功能**：保存冲突文件，后端自动生成唯一的冲突文件名。需要登录后才能访问。

**请求参数**：
```json
{
  "notebook_id": "uuid-string",
  "path": "folder1/note1.md",
  "content": "冲突内容..."
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| notebook_id | string | 是 | 笔记本ID |
| path | string | 是 | 原始笔记文件相对路径 |
| content | string | 是 | 冲突内容 |

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "conflict_path": "folder1/note1_conflict.md",
  "hash": "sha256-hash-string"
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| conflict_path | string | 实际保存的冲突文件路径 |
| hash | string | 冲突文件内容的 SHA-256 哈希值 |

**说明**：后端负责生成唯一的冲突文件名（原文件名追加 `_conflict`，已存在时追加递增数字），前端无需预判冲突文件是否存在。冲突文件会自动建立搜索索引，可通过全文搜索找到。

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `NOTEBOOK_NOT_FOUND` | 笔记本不存在 |
| `INVALID_FILE_PATH` | 路径不安全或不在允许范围内 |
| `FILE_WRITE_ERROR` | 文件写入失败（包括父目录创建失败） |
| `ENCRYPTED_NOTEBOOK` | 加密笔记本无法保存冲突文件 |
| `INTERNAL_ERROR` | 内部错误 |

## 9. 获取文件树接口

**路径**：POST /api/notebook/file_tree

**功能**：获取笔记本的完整文件树（递归），专门给笔记模块使用。需要登录后才能访问。

**请求参数**：
```json
{
  "notebook_id": "uuid-string"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| notebook_id | string | 是 | 笔记本ID |

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "tree": [
    {
      "name": "folder1",
      "path": "folder1",
      "is_dir": true,
      "children": [
        {
          "name": "note1.md",
          "path": "folder1/note1.md",
          "is_dir": false,
          "children": null
        }
      ]
    },
    {
      "name": "note2.md",
      "path": "note2.md",
      "is_dir": false,
      "children": null
    }
  ]
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| success | boolean | 是否成功 |
| tree | array | 顶层文件/文件夹列表 |
| tree[].name | string | 文件或文件夹名称 |
| tree[].path | string | 相对于笔记本根目录的路径 |
| tree[].is_dir | boolean | 是否为目录 |
| tree[].children | array/null | 子项列表（文件为 null） |

**说明**：
- 递归返回笔记本下所有目录和 `.md` 文件的完整树结构
- 过滤掉 `.notebook.sig` 文件和非 `.md` 的普通文件
- `attachment` 目录会出现在结果中（目录本身不过滤），但其中的非 `.md` 附件文件被过滤，前端通过附件相关接口单独管理附件
- 目录排序在前，文件排序在后，各自按名称升序
- 加密笔记本返回加密后的文件名，前端负责解密（`attachment` 目录名不加密）

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `NOTEBOOK_NOT_FOUND` | 笔记本不存在 |
| `INTERNAL_ERROR` | 内部错误 |

## 10. 全文搜索接口

**路径**：POST /api/notebook/search

**功能**：搜索普通（非加密）笔记本的笔记。需要登录后才能访问。

**请求参数**：
```json
{
  "keyword": "搜索关键词",
  "notebook_id": "uuid-string"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| keyword | string | 是 | 搜索关键词 |
| notebook_id | string | 否 | 限定搜索范围，不传则搜索所有普通笔记本 |

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "results": [
    {
      "notebook_id": "uuid-string",
      "notebook_name": "工作笔记",
      "note_path": "folder1/note1.md",
      "title": "note1",
      "title_matched": false,
      "matches": [
        {
          "line_number": 3,
          "content": "前面的内容<match>匹配的关键词</match>后面的内容"
        }
      ],
      "match_count": 5,
      "modified": "2026-01-15T10:30:00Z"
    }
  ]
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| results | array | 搜索结果列表 |
| results[].notebook_id | string | 笔记本ID |
| results[].notebook_name | string | 笔记本名称 |
| results[].note_path | string | 笔记路径 |
| results[].title | string | 笔记标题（从路径提取） |
| results[].title_matched | boolean | 标题是否匹配了关键词 |
| results[].matches | array | 匹配内容详情数组，最多 10 条 |
| results[].matches[].line_number | number | 匹配所在行号 |
| results[].matches[].content | string | 整行内容，匹配部分用 `<match></match>` 标签包裹，一行中多次匹配会有多个标签 |
| results[].match_count | number | 总匹配次数 |
| results[].modified | string | 最后修改时间 |

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `INVALID_PARAM` | 参数错误 |
| `SEARCH_FAILED` | 搜索失败 |
| `INTERNAL_ERROR` | 内部错误 |

## 11. 创建文件夹接口

**路径**：POST /api/notebook/create_folder

**功能**：在笔记本内创建文件夹。需要登录后才能访问。

**请求参数**：
```json
{
  "notebook_id": "uuid-string",
  "path": "folder1/subfolder"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| notebook_id | string | 是 | 笔记本ID |
| path | string | 是 | 文件夹相对路径（相对于笔记本路径） |

**返回值**：

**成功响应**：
```json
{
  "success": true
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `NOTEBOOK_NOT_FOUND` | 笔记本不存在 |
| `INVALID_FILE_PATH` | 路径不安全或不在允许范围内 |
| `INVALID_PARAM` | 参数错误 |
| `FOLDER_ALREADY_EXISTS` | 文件夹已存在 |
| `INTERNAL_ERROR` | 内部错误 |

## 12. 重命名笔记/文件夹接口

**路径**：POST /api/notebook/rename

**功能**：重命名笔记本内的笔记或文件夹。需要登录后才能访问。

**请求参数**：
```json
{
  "notebook_id": "uuid-string",
  "old_path": "folder1/note1.md",
  "new_name": "note_renamed.md"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| notebook_id | string | 是 | 笔记本ID |
| old_path | string | 是 | 原文件/文件夹相对路径 |
| new_name | string | 是 | 新名称（仅文件名，不含路径） |

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "new_path": "folder1/note_renamed.md"
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| new_path | string | 重命名后的完整相对路径 |

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `NOTEBOOK_NOT_FOUND` | 笔记本不存在 |
| `INVALID_FILE_PATH` | 路径不安全或不在允许范围内 |
| `FILE_NOT_FOUND` | 源文件不存在 |
| `FILE_ALREADY_EXISTS` | 目标文件/文件夹已存在 |
| `INVALID_PARAM` | 参数错误 |
| `FILE_WRITE_ERROR` | 重命名失败 |
| `INTERNAL_ERROR` | 内部错误 |

## 13. 移动笔记/文件夹接口

**路径**：POST /api/notebook/move

**功能**：将笔记或文件夹移动到笔记本内的另一个文件夹。需要登录后才能访问。

**请求参数**：
```json
{
  "notebook_id": "uuid-string",
  "source_path": "folder1/note1.md",
  "target_folder": "folder2"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| notebook_id | string | 是 | 笔记本ID |
| source_path | string | 是 | 源文件/文件夹相对路径 |
| target_folder | string | 是 | 目标文件夹相对路径（空字符串表示移动到笔记本根目录） |

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "new_path": "folder2/note1.md"
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| new_path | string | 移动后的完整相对路径 |

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `NOTEBOOK_NOT_FOUND` | 笔记本不存在 |
| `INVALID_FILE_PATH` | 路径不安全或不在允许范围内 |
| `FILE_NOT_FOUND` | 源文件不存在 |
| `FOLDER_NOT_FOUND` | 目标文件夹不存在 |
| `FILE_ALREADY_EXISTS` | 目标位置已存在同名文件 |
| `CANNOT_MOVE_TO_SELF` | 源和目标相同 |
| `CANNOT_MOVE_TO_SUBDIR` | 不能将文件夹移动到其子目录中 |
| `FILE_WRITE_ERROR` | 移动失败 |
| `INTERNAL_ERROR` | 内部错误 |

## 14. 获取附件 Token 接口

**路径**：POST /api/notebook/attachment_token

**功能**：获取访问笔记本附件的临时 Token（有效期 1 小时）。需要登录后才能访问。

**请求参数**：
```json
{
  "notebook_id": "uuid-string",
  "key": "base64-encoded-aes-key"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| notebook_id | string | 是 | 笔记本ID |
| key | string | 否 | 加密笔记本的 AES 密钥（Base64 编码），加密笔记本必传 |

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "token": "base64-payload.base64-signature",
  "expires_in": 3600
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| token | string | 临时访问 Token |
| expires_in | number | 有效期（秒） |

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `NOTEBOOK_NOT_FOUND` | 笔记本不存在 |
| `INVALID_PARAM` | 加密笔记本未传 key 或 key 验证失败 |
| `INTERNAL_ERROR` | 内部错误 |

## 15. 获取附件内容接口

**路径**：GET /api/notebook/attachment

**功能**：通过 Token 获取笔记本附件文件内容。不需要登录 Session，通过 Token 鉴权。加密笔记本的附件会自动解密后返回。

**请求参数**（Query String）：

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| notebook_id | string | 是 | 笔记本ID |
| path | string | 是 | 附件相对路径（相对于笔记本路径） |
| token | string | 是 | 通过 `/api/notebook/attachment_token` 获取的 Token |

**返回值**：

**成功响应**：直接返回文件二进制内容。非加密笔记本的 Content-Type 根据文件扩展名自动设置；加密笔记本的附件始终返回 `application/octet-stream`，前端应作为文件下载处理，不支持在线预览。

非加密笔记本支持的 MIME 类型：image/jpeg、image/png、image/gif、image/webp、image/svg+xml、application/pdf、video/mp4、audio/mpeg、text/plain、text/html、application/json，其他为 application/octet-stream。

**错误编码说明**（JSON 响应）：
| 错误编码 | 说明 |
|---------|------|
| `TOKEN_EXPIRED` | Token 过期或无效 |
| `INVALID_PARAM` | 参数错误（包括 notebook_id 与 token 不匹配） |
| `NOTEBOOK_NOT_FOUND` | 笔记本不存在 |
| `INVALID_FILE_PATH` | 路径不安全或不在允许范围内 |
| `FILE_NOT_FOUND` | 文件不存在或不是文件 |
| `FILE_READ_ERROR` | 文件读取失败（包括解密失败） |

## 16. 上传笔记本附件接口

**路径**：POST /api/notebook/upload_attachment

**功能**：上传附件文件到笔记本的 `attachment` 目录。需要登录后才能访问。后端自动将文件保存到 `<笔记本路径>/attachment/` 目录下。

**请求参数**（multipart/form-data）：

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| notebook_id | string | 是 | 笔记本ID |
| path | string | 是 | 附件文件名（仅文件名，不含路径，后端自动放入 attachment 目录） |
| file | file | 是 | 文件内容 |

**返回值**：

**成功响应**：
```json
{
  "success": true
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `MULTIPART_PARSE_ERROR` | multipart 数据解析失败 |
| `INVALID_PARAM` | 缺少 notebook_id |
| `INVALID_FILE_PATH` | 缺少文件路径或路径不安全 |
| `NOTEBOOK_NOT_FOUND` | 笔记本不存在 |
| `FILE_ALREADY_EXISTS` | 附件文件已存在 |
| `FILE_WRITE_ERROR` | 父目录创建失败 |
| `INTERNAL_ERROR` | 内部错误 |

## 17. 删除笔记本空文件夹接口

**路径**：POST /api/notebook/delete_folder

**功能**：删除笔记本内的空文件夹。需要登录后才能访问。如果用户启用了回收站，文件夹会移入回收站而非直接删除。

**请求参数**：
```json
{
  "notebook_id": "uuid-string",
  "path": "folder1"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| notebook_id | string | 是 | 笔记本ID |
| path | string | 是 | 要删除的文件夹相对路径（相对于笔记本路径） |

**返回值**：

**成功响应**：
```json
{
  "success": true
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `NOTEBOOK_NOT_FOUND` | 笔记本不存在 |
| `INVALID_FILE_PATH` | 路径不安全或不在允许范围内 |
| `PATH_NOT_FOUND` | 路径不存在 |
| `NOT_A_DIRECTORY` | 路径不是目录 |
| `FOLDER_NOT_EMPTY` | 文件夹不为空 |
| `RECYCLE_MOVE_FAILED` | 移入回收站失败 |
| `DELETE_FAILED` | 删除失败 |
| `INTERNAL_ERROR` | 内部错误 |

## 18. 批量删除笔记本文件接口

**路径**：POST /api/notebook/batch_delete

**功能**：批量删除笔记本内的文件（不允许删除目录）。需要登录后才能访问。如果用户启用了回收站，文件会移入回收站而非直接删除。先检查所有路径合法性，再逐个删除。不存在的文件路径不会报错，视为删除成功。

**请求参数**：
```json
{
  "notebook_id": "uuid-string",
  "paths": ["folder1/note1.md", "folder1/note2.md"]
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| notebook_id | string | 是 | 笔记本ID |
| paths | string[] | 是 | 要删除的文件相对路径列表（相对于笔记本路径），不允许传入目录 |

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "failed_paths": ["folder1/note3.md"]
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| success | boolean | 是否成功（始终为 true） |
| failed_paths | string[] | 删除失败的文件路径列表，无失败时不返回 |

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `NOTEBOOK_NOT_FOUND` | 笔记本不存在 |
| `INVALID_FILE_PATH` | 路径不安全或不在允许范围内 |
| `IS_DIRECTORY` | 路径是目录，请使用 delete_folder 接口 |
| `INTERNAL_ERROR` | 内部错误 |

# 分享模块接口

## 1. 获取分享信息

**路径**：POST /api/share/info

**功能**：根据分享码获取文件分享信息。无需登录即可访问（公开接口）。无论是否设置密码，均返回文件信息。

**请求参数**：
```json
{
  "share_code": "Ab3kX9mP"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| share_code | string | 是 | 8位分享码 |

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "file_name": "report.pdf",
  "file_size": 1048576,
  "is_directory": false,
  "share_mode": "page",
  "need_password": true,
  "password_salt": "randomSaltString32chars",
  "expire_type": "time",
  "expire_at": "2026-04-19 10:00:00",
  "max_downloads": null,
  "download_count": 3,
  "created_at": "2026-04-12 10:00:00"
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| file_name | string | 文件/文件夹名称 |
| file_size | integer | 文件大小（字节），文件夹时为文件夹内文件总大小 |
| is_directory | boolean | 是否为文件夹 |
| share_mode | string | 分享模式：`page`（下载页）/ `direct`（直链） |
| need_password | boolean | 是否需要密码 |
| password_salt | string/null | 密码盐值，need_password 为 true 时返回，用于前端计算密码 hash |
| expire_type | string | 过期类型：`permanent`/`time`/`count` |
| expire_at | string/null | 过期时间 |
| max_downloads | integer/null | 最大下载次数 |
| download_count | integer | 已下载次数 |
| created_at | string | 创建时间 |

**失败响应**：
```json
{
  "success": false,
  "fail_code": "SHARE_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `SHARE_NOT_FOUND` | 分享不存在 |
| `SHARE_EXPIRED` | 分享已过期 |
| `SHARE_FILE_MISSING` | 分享的文件已被删除 |
| `SHARE_OVER_LIMIT` | 下载次数已达上限 |
| `INTERNAL_ERROR` | 内部错误 |

**处理流程**：
1. 根据 share_code 查询分享记录
2. 检查过期条件（时间、下载次数），若已失效则返回对应错误
3. 检查物理文件是否存在，若不存在则返回错误
4. 返回分享信息（含 need_password 和 password_salt）

## 2. 获取下载 Token

**路径**：POST /api/share/get_download_token

**功能**：获取分享文件的下载 token。设置了密码的分享需要提交密码 hash 进行验证。无需登录即可访问（公开接口）。

**请求参数**：
```json
{
  "share_code": "Ab3kX9mP",
  "password_hash": "hex_encoded_hash_string"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| share_code | string | 是 | 8位分享码 |
| password_hash | string | 否 | 密码的 HMAC-SHA256 hash（使用 info 接口返回的 password_salt 计算），need_password 为 true 时必填 |

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "download_token": "hex_encoded_token_string"
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| download_token | string | 下载凭证，用于下载接口的 token 参数 |

**失败响应**：
```json
{
  "success": false,
  "fail_code": "SHARE_PASSWORD_WRONG"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `SHARE_NOT_FOUND` | 分享不存在 |
| `SHARE_EXPIRED` | 分享已过期 |
| `SHARE_FILE_MISSING` | 分享的文件已被删除 |
| `SHARE_OVER_LIMIT` | 下载次数已达上限 |
| `INTERNAL_ERROR` | 内部错误 |
| `SHARE_PASSWORD_REQUIRED` | 需要密码但未提供 |
| `SHARE_PASSWORD_WRONG` | 密码错误 |

**处理流程**：
1. 根据 share_code 查询分享记录
2. 检查过期条件（同 info 接口）
3. 若设置了密码，验证 password_hash 是否与存储的 hash 匹配
4. 验证通过，生成 download_token 并返回

## 3. 下载分享文件

**路径**：GET /api/share/file/{share_code}?token=xxx

**功能**：下载分享的文件。无需登录即可访问（公开接口）。文件夹自动打包为 zip 下载。GET 方式方便 wget / curl / 下载管理器直接使用。

**请求参数**：

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| share_code | string | 是 | URL路径中的分享码 |
| token | string | page 模式时必填 | 通过 get_download_token 接口获取的下载凭证 |

**返回值**：

**成功响应**：文件流（Content-Disposition: attachment）

**失败响应**：

对于分享验证类错误，返回 JSON，包含具体的失败原因：
```json
{
  "success": false,
  "fail_code": "SHARE_NOT_FOUND"
}
```

对于文件读取/压缩失败等内部错误（验证通过后实际提供文件时出错），返回 HTTP 404 或 HTTP 500，不返回 JSON。这是因为浏览器/下载工具在文件下载请求中收到 JSON 会将其当作文件内容下载，无法正确展示错误信息。

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `SHARE_NOT_FOUND` | 分享不存在 |
| `SHARE_EXPIRED` | 分享已过期 |
| `SHARE_FILE_MISSING` | 分享的文件已被删除 |
| `SHARE_OVER_LIMIT` | 下载次数已达上限 |
| `SHARE_DOWNLOAD_DENIED` | page 模式下未提供有效 token |
| `INTERNAL_ERROR` | 内部错误 |

**处理流程**：
1. 根据 share_code 查询分享记录
2. 检查过期条件、文件是否存在
3. page 模式下验证 token 是否有效
4. 文件分享：直接返回文件流
5. 文件夹分享：打包为 zip 后返回
6. 发起下载时 download_count + 1（无论最终下载是否成功）

**说明**：单文件和目录分享的下载次数均在发起下载时立刻统计，而非下载完成后统计。不支持多线程/断点续传下载。

## 4. 创建分享

**路径**：POST /api/share/create

**功能**：创建文件分享。同一文件只能创建一个活跃分享，如已有分享返回 `SHARE_ALREADY_EXISTS`。需要登录后才能访问。

**请求参数**：
```json
{
  "file_path": "documents/report.pdf",
  "expire_type": "time",
  "expire_at": "2026-04-19T10:00:00Z",
  "max_downloads": null,
  "share_mode": "page",
  "password": "mypassword"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| file_path | string | 是 | 文件/文件夹相对路径 |
| expire_type | string | 是 | 过期类型：`permanent`/`time`/`count` |
| expire_at | string | expire_type=time 时必填 | 过期时间，ISO 8601 格式 |
| max_downloads | integer | expire_type=count 时必填 | 最大下载次数，大于0 |
| share_mode | string | 是 | 分享模式：`page`/`direct` |
| password | string | 否 | 访问密码，不设置则传 null 或不传 |

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "share_code": "Ab3kX9mP",
  "share_url": "/s/Ab3kX9mP",
  "direct_url": "/api/share/file/Ab3kX9mP"
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| share_code | string | 8位分享码 |
| share_url | string | 下载页地址 |
| direct_url | string | 直链下载地址 |

**失败响应**：
```json
{
  "success": false,
  "fail_code": "FILE_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `FILE_NOT_FOUND` | 文件不存在 |
| `PATH_INVALID` | 文件路径无效 |
| `PARAM_INVALID` | 参数无效（缺少必填字段或值不合法） |
| `SHARE_DIRECT_NO_PASSWORD` | 直链分享不支持密码 |
| `SHARE_ALREADY_EXISTS` | 该文件已有活跃分享 |

**处理流程**：
1. 验证文件路径安全性
2. 检查文件/文件夹是否存在
3. 检查该文件是否已有活跃分享（未过期则不允许重复创建）
4. 生成 8 位随机 share_code，确保唯一性
5. 若提供了密码，使用 HMAC-SHA256 哈希后存储
6. 插入 shares 表记录
7. 返回 share_code、share_url、direct_url

## 5. 查询文件分享状态

**路径**：POST /api/share/get_by_path

**功能**：根据文件路径查询是否已有活跃分享。用于文件浏览页点击"分享"时判断显示创建表单还是已有分享信息。需要登录后才能访问。

**请求参数**：
```json
{
  "file_path": "documents/report.pdf"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| file_path | string | 是 | 文件/文件夹相对路径 |

**返回值**：

**成功响应（已分享）**：
```json
{
  "success": true,
  "share": {
    "id": "uuid-string",
    "file_path": "documents/report.pdf",
    "file_name": "report.pdf",
    "is_directory": false,
    "share_code": "Ab3kX9mP",
    "expire_type": "time",
    "expire_at": "2026-04-19T10:00:00Z",
    "max_downloads": null,
    "download_count": 3,
    "share_mode": "page",
    "has_password": true,
    "status": "active",
    "created_at": "2026-04-12 10:00:00"
  }
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| share | object/null | 分享信息，未分享时为 null |
| share.id | string | 分享ID |
| share.file_path | string | 文件相对路径 |
| share.file_name | string | 文件/文件夹名称 |
| share.is_directory | boolean | 是否为文件夹 |
| share.share_code | string | 分享码 |
| share.expire_type | string | 过期类型 |
| share.expire_at | string/null | 过期时间 |
| share.max_downloads | integer/null | 最大下载次数 |
| share.download_count | integer | 已下载次数 |
| share.share_mode | string | 分享模式 |
| share.has_password | boolean | 是否设置了密码 |
| share.status | string | 状态 |
| share.created_at | string | 创建时间 |

**成功响应（未分享）**：
```json
{
  "success": true,
  "share": null
}
```

**失败响应**：
```json
{
  "success": false,
  "fail_code": "INTERNAL_ERROR"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |

## 6. 获取分享列表

**路径**：POST /api/share/list

**功能**：获取当前用户的所有分享列表。需要登录后才能访问。

**请求参数**：无

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "shares": [
    {
      "id": "uuid-string",
      "file_name": "report.pdf",
      "file_path": "documents/report.pdf",
      "is_directory": false,
      "share_code": "Ab3kX9mP",
      "expire_type": "time",
      "expire_at": "2026-04-19T10:00:00Z",
      "max_downloads": null,
      "download_count": 3,
      "share_mode": "page",
      "has_password": true,
      "status": "active",
      "created_at": "2026-04-12 10:00:00"
    }
  ]
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| shares | array | 分享列表 |
| shares[].id | string | 分享ID |
| shares[].file_name | string | 文件/文件夹名称 |
| shares[].file_path | string | 文件相对路径 |
| shares[].is_directory | boolean | 是否为文件夹 |
| shares[].share_code | string | 分享码 |
| shares[].expire_type | string | 过期类型 |
| shares[].expire_at | string/null | 过期时间 |
| shares[].max_downloads | integer/null | 最大下载次数 |
| shares[].download_count | integer | 已下载次数 |
| shares[].share_mode | string | 分享模式 |
| shares[].has_password | boolean | 是否设置了密码 |
| shares[].status | string | 状态：`active`/`expired`/`file_missing` |
| shares[].created_at | string | 创建时间 |

**失败响应**：
```json
{
  "success": false,
  "fail_code": "INTERNAL_ERROR"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |

## 7. 删除分享

**路径**：POST /api/share/delete

**功能**：删除一条或多条分享记录。需要登录后才能访问。

**请求参数**：
```json
{
  "ids": ["uuid-1", "uuid-2"]
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| ids | string[] | 是 | 要删除的分享ID列表 |

**返回值**：

**成功响应**：
```json
{
  "success": true
}
```

**失败响应**：
```json
{
  "success": false,
  "fail_code": "INTERNAL_ERROR"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |

**处理流程**：
1. 仅删除属于当前用户的分享记录
2. 不存在的 ID 静默跳过，不报错

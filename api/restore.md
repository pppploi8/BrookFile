# 恢复模块接口

## 1. 检查目标目录接口

**路径**：POST /api/restore/check

**功能**：检查目标目录是否为空，返回文件列表供前端显示警告（仅前端提示，不影响后端逻辑）。需要登录后才能访问。

**请求参数**：
```json
{
  "local_path": "/data/restore"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| local_path | string | 是 | 恢复目标路径（相对路径） |

**返回值**：

**成功响应**：
```json
{
  "is_empty": false,
  "file_count": 5,
  "files": ["file1.txt", "file2.txt"]
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| is_empty | boolean | 目录是否为空 |
| file_count | number | 文件数量 |
| files | array | 文件名列表（最多返回10个） |

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `INVALID_FILE_PATH` | 恢复目标路径无效 |
| `INTERNAL_ERROR` | 内部错误 |

## 2. 启动恢复接口

**路径**：POST /api/restore/start

**功能**：启动恢复任务。启动恢复即覆盖已存在文件，不限制并发恢复。验证存储连接、密码、解析 .index 都在启动阶段完成，失败直接返回错误不创建任务。需要登录后才能访问。

**请求参数**：
```json
{
  "storage_type": "webdav",
  "storage_config": {
    "address": "https://webdav.example.com",
    "username": "user",
    "password": "password123",
    "path": "/backup/main"
  },
  "encrypted": true,
  "backup_password": "encrypt123",
  "local_path": "/data/restore"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| storage_type | string | 是 | 存储类型，目前仅支持 `webdav` |
| storage_config | object | 是 | 存储配置 |
| storage_config.address | string | 是 | WebDAV 服务地址 |
| storage_config.username | string | 是 | WebDAV 用户名 |
| storage_config.password | string | 是 | WebDAV 密码 |
| storage_config.path | string | 是 | WebDAV 存储路径 |
| encrypted | boolean | 否 | 是否加密备份，默认 false |
| backup_password | string | 否 | 备份加密密码（encrypted=true 时必填） |
| local_path | string | 是 | 恢复目标路径（相对路径） |

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "task_id": "uuid-string"
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| success | boolean | 是否成功 |
| task_id | string | 任务ID，用于后续查询进度 |

**失败响应**：
```json
{
  "success": false,
  "fail_code": "STORAGE_CONNECTION_ERROR",
  "message": "连接超时：无法访问 WebDAV 服务器"
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| success | boolean | 是否成功 |
| fail_code | string | 错误编码 |
| message | string | 具体错误信息（如：IP不通、账号错误、路径不存在、解密失败等） |

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `INVALID_FILE_PATH` | 恢复目标路径无效 |
| `STORAGE_CONNECTION_ERROR` | 存储连接失败（具体原因见 message 字段） |
| `INTERNAL_ERROR` | 内部错误 |

## 3. 查询进度接口

**路径**：POST /api/restore/progress

**功能**：查询恢复任务进度。需要登录后才能访问。

**请求参数**：
```json
{
  "task_id": "uuid-string"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| task_id | string | 是 | 任务ID |

**返回值**：

**任务运行中响应**：
```json
{
  "is_running": true,
  "downloading_items": [
    {"name": "file1.txt", "status": "downloading", "total_bytes": 1024, "downloaded_bytes": 512, "error": null}
  ],
  "failed_items": [
    {"name": "file2.txt", "status": "failed", "total_bytes": 2048, "downloaded_bytes": 0, "error": "Connection error"}
  ],
  "pending_count": 50,
  "total_count": 100,
  "success_count": 48,
  "downloaded_bytes": 52428800
}
```

**任务未运行响应**：
```json
{
  "is_running": false,
  "downloading_items": [],
  "failed_items": [],
  "pending_count": 0,
  "total_count": 0,
  "success_count": 0,
  "downloaded_bytes": 0
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| is_running | boolean | 任务是否正在运行 |
| downloading_items | array | 正在下载的文件列表（最多显示100个） |
| failed_items | array | 下载失败的文件列表（全部显示） |
| pending_count | number | 等待下载的文件数量 |
| total_count | number | 总文件数量 |
| success_count | number | 成功下载的文件数量 |
| downloaded_bytes | number | 已下载的字节数 |

**downloading_items 元素字段**：

| 字段 | 类型 | 说明 |
|-----|------|------|
| name | string | 文件名 |
| status | string | 状态：downloading/retrying |
| total_bytes | number | 总字节数 |
| downloaded_bytes | number | 已下载字节数 |
| error | string \| null | 错误信息 |

**failed_items 元素字段**：

| 字段 | 类型 | 说明 |
|-----|------|------|
| name | string | 文件名 |
| status | string | 状态：failed |
| total_bytes | number | 总字节数 |
| downloaded_bytes | number | 已下载字节数 |
| error | string | 失败原因 |

## 4. 取消恢复接口

**路径**：POST /api/restore/cancel

**功能**：取消正在执行的恢复任务。需要登录后才能访问。

**请求参数**：
```json
{
  "task_id": "uuid-string"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| task_id | string | 是 | 任务ID |

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
  "fail_code": "TASK_NOT_RUNNING"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `TASK_NOT_RUNNING` | 任务未运行 |
| `INTERNAL_ERROR` | 内部错误 |

## 5. 重试失败文件接口

**路径**：POST /api/restore/retry_file

**功能**：重试下载单个失败的文件。需要登录后才能访问。

**请求参数**：
```json
{
  "task_id": "uuid-string",
  "file_path": "documents/report.pdf"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| task_id | string | 是 | 任务ID |
| file_path | string | 是 | 失败文件的相对路径 |

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
  "fail_code": "FILE_NOT_IN_FAILED_LIST"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `TASK_NOT_RUNNING` | 任务未运行 |
| `FILE_NOT_IN_FAILED_LIST` | 文件不在失败列表中 |
| `ALREADY_RETRYING` | 文件正在重试中 |
| `INTERNAL_ERROR` | 内部错误 |

---

## 前端显示规则

1. **列表排序**：下载中 → 失败 → 等待下载（只显示数量）
2. **成功处理**：下载成功的文件直接从列表移除，不显示
3. **数量限制**：页面最多显示100个下载中的文件，避免卡死
4. **重试按钮**：失败文件行显示重试按钮，点击调用 `retry_file` 接口
5. **弹窗关闭**：
   - 任务未完成时阻止关闭弹窗（用户需取消或等待完成）
   - 任务完成后允许关闭

---

## 内存清理策略

- **自动清理**：任务完成后，如果超过 5 分钟没有进度查询请求，自动从内存清理

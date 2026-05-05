# 备份模块接口

## 1. 获取备份规则列表接口

**路径**：POST /api/backup/list

**功能**：获取当前用户的所有备份规则列表。需要登录后才能访问。此接口不返回 `success` 字段，直接返回数组。

**请求参数**：无

**返回值**：

**成功响应**：
```json
[
  {
    "id": "uuid-string",
    "name": "主存储备份",
    "storage_type": "webdav",
    "local_path": "/data/backup/main",
    "cycle": "daily",
    "backup_time": {"time": "08:00"},
    "status": "success",
    "next_backup_time": "2026-02-21 08:00:00",
    "last_backup_time": "2026-02-20T08:00:00Z",
    "created_at": "2025-01-15T10:30:00Z"
  }
]
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| id | string | 备份规则ID |
| name | string | 备份规则名称 |
| storage_type | string | 存储类型 |
| local_path | string | 本地备份路径 |
| cycle | string | 备份周期 |
| backup_time | object | 备份时间配置 |
| status | string | 当前状态 |
| next_backup_time | string | 下次备份时间（根据周期和时间配置计算得出） |
| last_backup_time | string | 上次备份时间（ISO 8601格式） |
| created_at | string | 创建时间 |

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `BACKUP_RULE_NOT_FOUND` | 备份规则不存在 |

## 加密设计说明

### 文件内容加密

使用 ChaCha20-Poly1305（AEAD）加密，提供机密性和完整性保护。每个文件生成随机 12 字节 nonce，按块加密上传。

### 文件名加密

使用 ChaCha20（流密码，无认证 tag）加密文件名。nonce 从密码确定性派生（SHA-256(password)），不存储在输出中。

**设计决策**：文件名加密不使用 AEAD（如 ChaCha20-Poly1305），原因如下：

- AEAD 会产生 16 字节认证 tag + 12 字节 nonce，额外膨胀 28 字节/每段路径
- 路径的每一段（目录名/文件名）分别加密后 base64 编码，使用 AEAD 时一段 5 字节的文件名会膨胀到约 44 字符
- 多级目录的加密路径会轻易超过 Windows 260 字符的路径长度限制，导致备份失败
- ChaCha20（流密码）加密后输出长度等于输入长度，base64 后膨胀约 4/3 倍，5 字节文件名仅约 8 字符

**安全性影响**：相同的文件名在相同密码下会产生相同的密文（确定性加密），攻击者可判断两个位置是否存在同名文件。这是出于路径长度限制的必要取舍，不构成实际安全威胁——文件内容的加密完整性不受影响。

## 3. 创建备份规则接口

**路径**：POST /api/backup/create

**功能**：创建新的备份规则。需要登录后才能访问。

**请求参数**：
```json
{
  "name": "主存储备份",
  "storage_type": "webdav",
  "storage_config": {
    "address": "https://webdav.example.com",
    "username": "user",
    "password": "password123",
    "path": "/backup/main"
  },
  "local_path": "/data/backup/main",
  "encrypted": true,
  "backup_password": "encrypt123",
  "cycle": "daily",
  "backup_time": {"time": "08:00"}
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| name | string | 是 | 备份规则名称 |
| storage_type | string | 是 | 存储类型，目前仅支持 `webdav` |
| storage_config | object | 是 | 存储配置 |
| storage_config.address | string | 是 | WebDAV 服务地址 |
| storage_config.username | string | 是 | WebDAV 用户名 |
| storage_config.password | string | 是 | WebDAV 密码 |
| storage_config.path | string | 是 | WebDAV 存储路径 |
| local_path | string | 是 | 本地备份路径 |
| encrypted | boolean | 否 | 是否加密备份，默认 false |
| backup_password | string | 否 | 备份加密密码（encrypted=true 时必填） |
| cycle | string | 是 | 备份周期：daily/weekly/monthly/yearly |
| backup_time | object | 是 | 备份时间配置 |

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
  "fail_code": "INVALID_PARAM"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `NAME_EMPTY` | 备份规则名称不能为空 |
| `LOCAL_PATH_EMPTY` | 本地备份路径不能为空 |
| `INVALID_STORAGE_TYPE` | 存储类型无效 |
| `INVALID_CYCLE` | 备份周期无效 |
| `INVALID_PARAM` | 参数错误（如加密未提供密码） |
| `INTERNAL_ERROR` | 内部错误 |

## 4. 更新备份规则接口

**路径**：POST /api/backup/update

**功能**：更新指定的备份规则。需要登录后才能访问。

**请求参数**：
```json
{
  "id": "uuid-string",
  "name": "主存储备份",
  "storage_type": "webdav",
  "storage_config": {
    "address": "https://webdav.example.com",
    "username": "user",
    "password": "newpassword",
    "path": "/backup/main"
  },
  "local_path": "/data/backup/main",
  "encrypted": true,
  "backup_password": "newencrypt123",
  "cycle": "weekly",
  "backup_time": {"week_day": 1, "time": "10:00"}
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| id | string | 是 | 备份规则ID |
| name | string | 是 | 备份规则名称 |
| storage_type | string | 是 | 存储类型 |
| storage_config | object | 是 | 存储配置 |
| local_path | string | 是 | 本地备份路径 |
| encrypted | boolean | 否 | 是否加密备份 |
| backup_password | string | 否 | 备份加密密码（为空表示不修改） |
| cycle | string | 是 | 备份周期 |
| backup_time | object | 是 | 备份时间配置 |

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
  "fail_code": "BACKUP_RULE_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `BACKUP_RULE_NOT_FOUND` | 备份规则不存在 |
| `INVALID_PARAM` | 参数错误 |
| `BACKUP_RUNNING` | 备份正在进行中，无法修改 |
| `INTERNAL_ERROR` | 内部错误 |

## 5. 删除备份规则接口

**路径**：POST /api/backup/delete

**功能**：删除指定的备份规则。需要登录后才能访问。

**请求参数**：
```json
{
  "id": "uuid-string"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| id | string | 是 | 备份规则ID |

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
  "fail_code": "BACKUP_RULE_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `BACKUP_RULE_NOT_FOUND` | 备份规则不存在 |
| `BACKUP_RUNNING` | 备份正在进行中，无法删除 |

## 6. 立即开始备份接口

**路径**：POST /api/backup/start

**功能**：立即启动备份任务。需要登录后才能访问。

**请求参数**：
```json
{
  "rule_id": "uuid-string",
  "mode": "full"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| rule_id | string | 是 | 备份规则ID |
| mode | string | 是 | 执行模式：`full`（备份+清理）/ `cleanup_only`（仅清理） |

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
| task_id | string | 任务ID |

**失败响应**：
```json
{
  "success": false,
  "fail_code": "TASK_ALREADY_RUNNING"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `BACKUP_RULE_NOT_FOUND` | 备份规则不存在 |
| `TASK_ALREADY_RUNNING` | 该规则已有任务在执行中 |

## 7. 取消备份任务接口

**路径**：POST /api/backup/cancel

**功能**：取消正在执行的备份任务。需要登录后才能访问。

**请求参数**：
```json
{
  "rule_id": "uuid-string"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| rule_id | string | 是 | 备份规则ID |

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
| `BACKUP_RULE_NOT_FOUND` | 备份规则不存在 |
| `TASK_NOT_RUNNING` | 该规则没有正在运行的任务 |

## 8. 获取任务进度接口

**路径**：POST /api/backup/progress

**功能**：获取备份任务执行状态。需要登录后才能访问。

**请求参数**：
```json
{
  "rule_id": "uuid-string"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| rule_id | string | 是 | 备份规则ID |

**返回值**：

**正在执行时的响应**：
```json
{
  "is_running": true,
  "phase": "backup",
  "sub_phase": "scanning",
  "pending_items": [],
  "total_count": 0,
  "scanned_bytes": 1048576
}
```

或

```json
{
  "is_running": true,
  "phase": "backup",
  "sub_phase": null,
  "pending_items": [
    {"name": "file1.txt", "status": "uploading", "total_bytes": 1024, "uploaded_bytes": 512, "error": null},
    {"name": "file2.jpg", "status": "waiting_retry (1/2)", "total_bytes": 204800, "uploaded_bytes": 0, "error": null},
    {"name": "file3.png", "status": "failed", "total_bytes": 102400, "uploaded_bytes": 0, "error": "ConnectionError: Failed to connect"}
  ],
  "total_count": 10,
  "scanned_bytes": 104857600
}
```

**未执行时的响应**：
```json
{
  "is_running": false,
  "phase": "backup",
  "sub_phase": null,
  "pending_items": [],
  "total_count": 0,
  "scanned_bytes": 0
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| is_running | boolean | 是否正在执行任务 |
| phase | string | 当前执行阶段：`backup`（备份）/ `cleanup`（清理） |
| sub_phase | string \| null | 当前执行子阶段：`scanning`（扫描中），仅在 phase=backup 且正在扫描文件时有值 |
| pending_items | array | 等待处理的文件列表（备份阶段）或目录列表（清理阶段），扫描阶段为空数组 |
| total_count | number | 等待处理的文件总数 |
| scanned_bytes | number | 已扫描的字节数，用于在扫描阶段显示进度 |

**pending_items 元素字段**：

| 字段 | 类型 | 说明 |
|-----|------|------|
| name | string | 文件名或目录名 |
| status | string | 状态（见下表） |
| total_bytes | number | 总字节数（仅备份阶段有效） |
| uploaded_bytes | number | 已上传字节数（仅备份阶段有效） |
| error | string \| null | 失败原因（仅失败状态时有值） |

**status 状态值说明**：

| 状态值 | 说明 |
|-------|------|
| `pending` | 等待处理 |
| `uploading` | 正在上传 |
| `waiting_retry (n/m)` | 等待重试，n为当前重试次数，m为最大重试次数 |
| `retrying (n/m)` | 正在重试上传 |
| `completed` | 已完成 |
| `failed` | 失败 |
| `cleaning` | 正在清理（清理阶段） |

**失败响应**：
```json
{
  "success": false,
  "fail_code": "BACKUP_RULE_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `BACKUP_RULE_NOT_FOUND` | 备份规则不存在 |
| `INTERNAL_ERROR` | 内部错误 |

## 9. 获取历史备份日志接口

**路径**：POST /api/backup/logs

**功能**：获取指定备份规则的历史备份日志。需要登录后才能访问。

**请求参数**：
```json
{
  "rule_id": "uuid-string",
  "page": 1,
  "page_size": 20
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| rule_id | string | 是 | 备份规则ID |
| page | number | 是 | 页码，从1开始 |
| page_size | number | 是 | 每页数量 |

**返回值**：

**成功响应**：
```json
{
  "total": 100,
  "page": 1,
  "page_size": 20,
  "items": [
    {
      "id": "uuid-string",
      "rule_id": "uuid-string",
      "mode": "full",
      "status": "completed",
      "started_at": "2026-02-23 10:00:00",
      "finished_at": "2026-02-23 10:30:00",
      "backup_success_count": 100,
      "backup_fail_count": 2,
      "cleanup_deleted_count": 5,
      "fail_reason": null
    }
  ]
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| total | number | 总记录数 |
| page | number | 当前页码 |
| page_size | number | 每页数量 |
| items | array | 日志列表 |

**items 元素字段**：

| 字段 | 类型 | 说明 |
|-----|------|------|
| id | string | 日志ID |
| rule_id | string | 备份规则ID |
| mode | string | 执行模式：full / cleanup_only |
| status | string | 状态：completed / failed / cancelled / interrupted |
| started_at | string | 开始时间 |
| finished_at | string | 结束时间（未结束为null） |
| backup_success_count | number | 备份成功文件数 |
| backup_fail_count | number | 备份失败文件数 |
| cleanup_deleted_count | number | 清理删除文件数 |
| fail_reason | string | 失败原因（无则为null） |

**失败响应**：
```json
{
  "success": false,
  "fail_code": "BACKUP_RULE_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `BACKUP_RULE_NOT_FOUND` | 备份规则不存在 |

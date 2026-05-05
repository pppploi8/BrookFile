# 回收站模块接口

## 1. 获取回收站列表接口

**路径**：POST /api/recycle/list

**功能**：获取当前用户回收站中的文件列表（分页）。需要登录后才能访问。

**请求参数**：
```json
{
  "page": 1,
  "page_size": 20
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| page | number | 否 | 页码，从1开始，默认1 |
| page_size | number | 否 | 每页数量，默认20，最大1000 |

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "data": {
    "items": [
      {
        "id": "uuid-string",
        "original_path": "documents/report.pdf",
        "original_name": "report.pdf",
        "is_directory": false,
        "file_size": 1048576,
        "deleted_at": "2026-04-12 10:00:00"
      }
    ],
    "total": 100,
    "page": 1,
    "page_size": 20
  }
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| data | object | 分页数据 |
| data.items | array | 回收站文件列表 |
| data.items[].id | string | 回收站记录ID |
| data.items[].original_path | string | 原始文件相对路径 |
| data.items[].original_name | string | 原始文件名 |
| data.items[].is_directory | boolean | 是否为目录 |
| data.items[].file_size | number | 文件大小（字节） |
| data.items[].deleted_at | string | 删除时间 |
| data.total | number | 总记录数 |
| data.page | number | 当前页码 |
| data.page_size | number | 每页数量 |

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `RECYCLE_NOT_ENABLED` | 用户未启用回收站 |
| `INTERNAL_ERROR` | 内部错误 |

## 2. 恢复回收站文件接口

**路径**：POST /api/recycle/restore

**功能**：将回收站中的单个文件恢复到原始路径。需要登录后才能访问。

**请求参数**：
```json
{
  "id": "uuid-string"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| id | string | 是 | 回收站记录ID |

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
| `RECYCLE_NOT_ENABLED` | 用户未启用回收站 |
| `RECYCLE_ITEM_NOT_FOUND` | 回收站记录不存在 |
| `RESTORE_PATH_OCCUPIED` | 原始路径已被占用 |
| `RESTORE_FAILED` | 恢复失败 |
| `INTERNAL_ERROR` | 内部错误 |

## 3. 批量恢复回收站文件接口

**路径**：POST /api/recycle/batch_restore

**功能**：批量恢复回收站中的文件到原始路径。需要登录后才能访问。先检查所有文件是否有冲突，再逐个恢复。

**请求参数**：
```json
{
  "ids": ["uuid-1", "uuid-2"]
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| ids | string[] | 是 | 回收站记录ID列表 |

**返回值**：

**成功响应**：
```json
{
  "success": true
}
```

**失败响应（路径冲突）**：
```json
{
  "success": false,
  "fail_code": "RESTORE_PATH_OCCUPIED",
  "data": {
    "conflict_items": [
      {
        "id": "uuid-string",
        "original_path": "documents/report.pdf",
        "original_name": "report.pdf",
        "is_directory": false
      }
    ]
  }
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| data | object | 响应数据 |
| data.conflict_items | array/null | 冲突文件列表 |

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `RECYCLE_NOT_ENABLED` | 用户未启用回收站 |
| `RECYCLE_ITEM_NOT_FOUND` | 部分回收站记录不存在 |
| `RESTORE_PATH_OCCUPIED` | 原始路径已被占用，返回冲突列表 |
| `RESTORE_FAILED` | 部分文件恢复失败，返回 `data.failed_paths` |
| `INTERNAL_ERROR` | 内部错误 |

**RESTORE_FAILED 失败响应**：
```json
{
  "success": false,
  "fail_code": "RESTORE_FAILED",
  "data": {
    "failed_paths": ["documents/report.pdf"]
  }
}
```

## 4. 删除回收站文件接口

**路径**：POST /api/recycle/delete

**功能**：永久删除回收站中的单个文件（同时删除物理文件和数据库记录）。需要登录后才能访问。

**请求参数**：
```json
{
  "id": "uuid-string"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| id | string | 是 | 回收站记录ID |

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
| `RECYCLE_NOT_ENABLED` | 用户未启用回收站 |
| `RECYCLE_ITEM_NOT_FOUND` | 回收站记录不存在 |
| `DELETE_FAILED` | 删除失败 |
| `INTERNAL_ERROR` | 内部错误 |

## 5. 批量删除回收站文件接口

**路径**：POST /api/recycle/batch_delete

**功能**：永久批量删除回收站中的文件。需要登录后才能访问。

**请求参数**：
```json
{
  "ids": ["uuid-1", "uuid-2"]
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| ids | string[] | 是 | 回收站记录ID列表 |

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
| `RECYCLE_NOT_ENABLED` | 用户未启用回收站 |
| `RECYCLE_ITEM_NOT_FOUND` | 部分回收站记录不存在 |
| `DELETE_FAILED` | 部分文件删除失败，返回 `data.failed_paths` |
| `INTERNAL_ERROR` | 内部错误 |

**DELETE_FAILED 部分失败响应**：
```json
{
  "success": false,
  "fail_code": "DELETE_FAILED",
  "data": {
    "failed_paths": ["uuid-1", "uuid-2"]
  }
}
```

## 6. 清空回收站接口

**路径**：POST /api/recycle/empty

**功能**：清空回收站中的所有文件（同时删除所有物理文件和数据库记录）。需要登录后才能访问。

**请求参数**：无

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
| `RECYCLE_NOT_ENABLED` | 用户未启用回收站 |
| `DELETE_FAILED` | 删除失败 |
| `INTERNAL_ERROR` | 内部错误 |

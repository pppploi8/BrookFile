# 密码库模块接口

## 1. 获取密码库列表

**路径**：POST /api/vault/list

**功能**：获取当前用户的密码库列表。需要登录后才能访问。此接口不返回 `success` 字段。

**请求参数**：无

**返回值**：

**成功响应**：
```json
{
  "vaults": [
    {
      "id": "uuid-string",
      "name": "个人密码",
      "description": "个人常用账号密码",
      "path": "密码库",
      "filename": "personal.dat",
      "created_at": "2026-03-22 12:00:00",
      "updated_at": "2026-03-22 12:00:00"
    }
  ]
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| vaults | array | 密码库列表 |
| vaults[].id | string | 密码库ID |
| vaults[].name | string | 密码库名称 |
| vaults[].description | string | 描述 |
| vaults[].path | string | 存储目录（相对路径） |
| vaults[].filename | string | 文件名 |
| vaults[].created_at | string | 创建时间 |
| vaults[].updated_at | string | 更新时间 |

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

## 2. 创建密码库

**路径**：POST /api/vault/create

**功能**：创建密码库，在数据库中添加记录，并将初始加密文件上传到网盘。需要登录后才能访问。

**请求参数**：
```json
{
  "name": "个人密码",
  "description": "个人常用账号密码",
  "path": "密码库",
  "filename": "personal.dat",
  "file_data": "Base64编码的密码库文件内容"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| name | string | 是 | 密码库名称 |
| description | string | 否 | 描述，默认空字符串 |
| path | string | 是 | 存储目录（相对路径） |
| filename | string | 是 | 文件名 |
| file_data | string | 是 | 密码库文件内容（Base64编码） |

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
| id | string | 密码库ID |

**失败响应**：
```json
{
  "success": false,
  "fail_code": "FILE_WRITE_ERROR"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `INVALID_PARAM` | 参数错误 |
| `INVALID_FILE_PATH` | 文件路径或文件名无效 |
| `INVALID_FILE_DATA` | 文件数据Base64解码失败 |
| `VAULT_ALREADY_EXISTS` | 该路径已存在密码库 |
| `FILE_ALREADY_EXISTS` | 文件已存在 |
| `FILE_WRITE_ERROR` | 文件写入失败 |

**处理流程**：
1. 验证路径和文件名安全性
2. Base64解码文件数据
3. 自动创建父目录（如不存在）
4. 写入文件到网盘
5. 在 vaults 表中插入记录
6. 返回密码库ID

## 3. 更新密码库文件

**路径**：POST /api/vault/update

**功能**：更新密码库文件内容。需要登录后才能访问。

**请求参数**：
```json
{
  "id": "uuid-string",
  "file_data": "Base64编码的密码库文件内容"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| id | string | 是 | 密码库ID |
| file_data | string | 否 | 新的密码库文件内容（Base64编码） |

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
  "fail_code": "VAULT_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `VAULT_NOT_FOUND` | 密码库不存在 |
| `INVALID_FILE_DATA` | 文件数据Base64解码失败 |
| `FILE_WRITE_ERROR` | 文件写入失败 |

**处理流程**：
1. 验证密码库存在且属于当前用户
2. 如果提供了 file_data，Base64解码后写入文件（覆盖原文件）

**说明**：密码库内容完整存储在文件中，数据库仅存储路径等元数据。更新时先写入文件再更新数据库，这是安全的，因为文件是自包含的。

## 3.1 更新密码库元数据

**路径**：POST /api/vault/update_meta

**功能**：更新密码库的名称和描述。需要登录后才能访问。

**请求参数**：
```json
{
  "id": "uuid-string",
  "name": "个人密码2",
  "description": "更新后的描述"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| id | string | 是 | 密码库ID |
| name | string | 否 | 新的密码库名称 |
| description | string | 否 | 新的描述 |

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
  "fail_code": "VAULT_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `VAULT_NOT_FOUND` | 密码库不存在 |
| `INTERNAL_ERROR` | 内部错误 |

## 4. 删除密码库

**路径**：POST /api/vault/delete

**功能**：删除密码库的数据库记录。不删除网盘中的加密文件，文件需用户在文件管理中手动删除。需要登录后才能访问。

**请求参数**：
```json
{
  "id": "uuid-string"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| id | string | 是 | 密码库ID |

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
  "fail_code": "VAULT_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `VAULT_NOT_FOUND` | 密码库不存在 |

## 5. 导入密码库

**路径**：POST /api/vault/import

**功能**：从网盘中已有文件导入为密码库。在数据库中创建记录，关联已有的网盘文件。需要登录后才能访问。

**请求参数**：
```json
{
  "name": "导入的密码库",
  "description": "从备份恢复的密码库",
  "file_path": "备份/passwords.dat"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| name | string | 是 | 密码库名称 |
| description | string | 否 | 描述，默认空字符串 |
| file_path | string | 是 | 网盘中的文件相对路径 |

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
| id | string | 密码库ID |

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
| `INVALID_PARAM` | 参数错误 |
| `FILE_NOT_FOUND` | 文件不存在 |
| `VAULT_ALREADY_EXISTS` | 该文件已被导入为密码库 |
| `INVALID_FILE_PATH` | 文件路径无效（含路径穿越时返回此错误而非 FILE_NOT_FOUND） |

**处理流程**：
1. 验证文件路径安全性
2. 检查文件是否存在
3. 从 file_path 解析出 path 和 filename
4. 检查该文件是否已被导入
5. 在 vaults 表中插入记录
6. 返回密码库ID

## 6. 单文件上传接口

**路径**：POST /api/vault/upload_single

**功能**：一次性上传完整文件内容，适用于密码库等小文件场景，无需分块。如果父目录不存在会自动创建。覆盖同名文件。需要登录后才能访问。

**请求参数**：Content-Type: multipart/form-data

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| path | string | 是 | 文件相对路径，如 "密码库/personal.dat" |
| file | binary | 是 | 文件完整内容 |

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
  "fail_code": "FILE_WRITE_ERROR"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `INVALID_FILE_PATH` | 文件路径无效 |
| `MULTIPART_PARSE_ERROR` | multipart数据解析失败 |
| `FILE_WRITE_ERROR` | 文件写入失败 |

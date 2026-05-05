# WebDAV模块接口

## 1. 获取WebDAV配置列表

**路径**：POST /api/webdav/list

**功能**：获取当前用户的所有 WebDAV 配置。需要登录后才能访问。

**请求参数**：无

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "configs": [
    {
      "id": "uuid-string",
      "dav_path": "photos",
      "access_path": "photos/2024",
      "permission": "full_control",
      "url": "/dav/photos/",
      "global_access": false,
      "created_at": "2026-04-16 10:00:00",
      "updated_at": "2026-04-16 10:00:00"
    }
  ]
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| configs | array | 配置列表 |
| configs[].id | string | 配置ID |
| configs[].dav_path | string | DAV路径标识，空字符串表示根路径 |
| configs[].access_path | string | 映射的文件系统路径 |
| configs[].permission | string | 访问权限：`full_control` / `edit` / `read_only` |
| configs[].url | string | WebDAV 访问路径 |
| configs[].global_access | boolean | 是否启用全局路径 |
| configs[].created_at | string | 创建时间 |
| configs[].updated_at | string | 更新时间 |

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

## 2. 创建WebDAV配置

**路径**：POST /api/webdav/create

**功能**：创建新的 WebDAV 配置。需要登录后才能访问。

**请求参数**：
```json
{
  "dav_path": "photos",
  "access_path": "photos/2024",
  "password": "mypassword",
  "permission": "full_control",
  "global_access": false
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| dav_path | string | 是 | DAV路径标识，只允许字母/数字/连字符/下划线。global_access=true 时必须为空，global_access=false 时不允许为空 |
| access_path | string | 是 | 映射的文件夹路径（相对于用户根目录） |
| password | string | 是 | 访问密码（明文，后端哈希存储），不允许为空 |
| permission | string | 是 | 访问权限：`full_control` / `edit` / `read_only` |
| global_access | boolean | 否 | 是否启用全局路径，默认false。启用后dav_path必须为空，且不可添加其他配置 |

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
  "fail_code": "DAV_PATH_DUPLICATE"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `DAV_PATH_DUPLICATE` | dav路径与当前用户已有配置重复 |
| `DAV_PATH_INVALID` | dav路径包含非法字符 |
| `DAV_CONFIGS_CONFLICT` | 已存在其他WebDAV配置，无法开启全局路径 |
| `DAV_GLOBAL_EXISTS` | 已存在全局WebDAV配置，无法添加更多配置 |
| `PARAM_INVALID` | 参数无效（缺少必填字段或值不合法） |

**处理流程**：
1. 验证 dav_path 格式（只允许字母、数字、连字符、下划线，或空字符串）
2. 验证 permission 值合法
3. 若 global_access 为 true，验证 dav_path 为空，且该用户没有其他配置
4. 若 global_access 为 false，验证该用户没有全局配置
5. 生成随机 salt，使用 HMAC-SHA256 对密码进行哈希
6. 插入 webdav_configs 表记录

## 3. 更新WebDAV配置

**路径**：POST /api/webdav/update

**功能**：更新指定的 WebDAV 配置。需要登录后才能访问。

**请求参数**：
```json
{
  "id": "uuid-string",
  "dav_path": "photos",
  "access_path": "photos/2024",
  "password": "",
  "permission": "edit",
  "global_access": false
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| id | string | 是 | 配置ID |
| dav_path | string | 是 | DAV路径标识，只允许字母/数字/连字符/下划线。global_access=true 时必须为空，global_access=false 时不允许为空 |
| access_path | string | 是 | 映射的文件夹路径（相对于用户根目录） |
| password | string | 否 | 新访问密码，空字符串或不传表示不修改密码。创建时密码不允许为空 |
| permission | string | 是 | 访问权限 |
| global_access | boolean | 否 | 是否启用全局路径，默认false |

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
  "fail_code": "DAV_CONFIG_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `DAV_CONFIG_NOT_FOUND` | 配置不存在或不属于当前用户 |
| `DAV_PATH_DUPLICATE` | dav路径与当前用户其他配置重复 |
| `DAV_PATH_INVALID` | dav路径包含非法字符 |
| `DAV_CONFIGS_CONFLICT` | 已存在其他WebDAV配置，无法开启全局路径 |
| `DAV_GLOBAL_EXISTS` | 已存在全局WebDAV配置，无法添加更多配置 |
| `PARAM_INVALID` | 参数无效 |

**处理流程**：
1. 验证配置存在且属于当前用户
2. 验证 dav_path 格式
3. 若 global_access 为 true，验证 dav_path 为空，且该用户没有其他配置（排除自身）
4. 若 global_access 为 false，验证该用户没有其他全局配置（排除自身）
5. 检查同一用户下 dav_path 是否与其他配置重复（排除自身）
6. 若 password 非空，重新生成 salt 并更新密码哈希
7. 更新 webdav_configs 表记录

## 4. 删除WebDAV配置

**路径**：POST /api/webdav/delete

**功能**：删除指定的 WebDAV 配置。需要登录后才能访问。

**请求参数**：
```json
{
  "id": "uuid-string"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|
| id | string | 是 | 配置ID |

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
  "fail_code": "DAV_CONFIG_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `DAV_CONFIG_NOT_FOUND` | 配置不存在或不属于当前用户 |

**处理流程**：
1. 仅删除属于当前用户的配置记录
2. 不存在的 ID 返回 DAV_CONFIG_NOT_FOUND

## WebDAV 协议端点

**基础路径**：`/dav/*`

**认证方式**：HTTP Basic Auth（用户名=系统账户用户名，密码=配置的访问密码）

**支持的 WebDAV 方法**（根据 permission 控制）：

**限制**：不支持 `Depth: infinity`，请求时返回 HTTP 403 Forbidden。

| 方法 | 操作 | full_control | edit | read_only |
|------|------|:---:|:---:|:---:|
| PROPFIND | 浏览目录/文件属性 | ✓ | ✓ | ✓ |
| GET | 下载文件 | ✓ | ✓ | ✓ |
| PUT | 上传文件 | ✓ | ✓ | ✗ |
| MKCOL | 创建目录 | ✓ | ✓ | ✗ |
| COPY | 复制文件/目录 | ✓ | ✓ | ✗ |
| MOVE | 移动/重命名 | ✓ | ✓ | ✗ |
| DELETE | 删除 | ✓ | ✗ | ✗ |
| OPTIONS | 查询支持的方法 | ✓ | ✓ | ✓ |

**请求路由逻辑**：
1. 从 Basic Auth 提取用户名和密码
2. 查找用户，验证密码与对应 `webdav_configs` 记录的哈希匹配
3. 从 URL 提取 dav_path（`/dav/{dav_path}/...` 中的第一段），匹配用户的配置
4. 剩余路径映射到 `access_path` 对应的文件系统位置
5. 根据 `permission` 检查操作是否允许

**MOVE/COPY 请求头**：

| 请求头 | 必填 | 说明 |
|-------|------|------|
| Destination | 是 | 目标路径（绝对路径或完整 URL） |
| Overwrite | 否 | `T`（默认）允许覆盖，`F` 禁止覆盖（返回 412 Precondition Failed） |

**说明**：MOVE/COPY 覆盖目标时，原目标内容会被备份为 `.davbak` 文件。操作失败时不会自动恢复备份文件，需手动处理。

**PROPFIND 自动创建根目录**：当 PROPFIND 请求访问的目录不存在，且路径为配置的根路径（即 `relative` 为空或 `/`）时，系统会自动创建该目录。这是为了在首次通过 WebDAV 客户端连接时确保根目录存在。非根路径的目录不存在时返回 404。

**文件名限制**：
PUT、MKCOL、MOVE、COPY 操作会对目标文件名进行安全校验，不允许以下文件名：
- 包含 `/`、`\`、`\0` 的文件名
- `.` 或 `..`
- 空文件名

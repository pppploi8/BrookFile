# 系统模块接口

## 1. 系统信息接口

**路径**：POST /api/system/info

**功能**：返回系统状态信息，包括是否已初始化、是否已登录、系统名称以及登录用户信息。此接口不返回 `success` 字段。

**请求参数**：无

**返回值**：

**成功响应**：
```json
{
  "initialized": true,
  "logged_in": true,
  "system_name": "BrookFile",
  "user": {
    "username": "admin",
    "is_admin": true
  }
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| initialized | boolean | 系统是否已初始化 |
| logged_in | boolean | 当前是否已登录 |
| system_name | string | 系统名称 |
| user | object | 登录用户信息（仅当 logged_in 为 true 时返回） |
| user.username | string | 用户名 |
| user.is_admin | boolean | 是否为管理员 |
| user.id | string | 用户ID |
| user.feature_order | string | 功能排序 |
| user.recycle_bin_enabled | boolean | 是否启用回收站 |
| user.has_shares | boolean | 是否有分享记录 |

## 2. 初始化接口

**路径**：POST /api/system/init

**功能**：初始化系统，创建管理员账号并设置系统配置。仅当系统未初始化时可调用。

**请求参数**：
```json
{
  "username": "admin",
  "password": "password123",
  "system_name": "BrookFile",
  "root_path": "D:\\data"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| username | string | 是 | 管理员用户名 |
| password | string | 是 | 管理员密码 |
| system_name | string | 是 | 系统名称，显示在登录页和主页标题 |
| root_path | string | 是 | 文件存储根路径，不允许为空 |
| recycle_bin_path | string | 否 | 回收站路径，留空则不开启回收站，不能是 root_path 或其子路径 |

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
  "fail_code": "SYSTEM_ALREADY_INITIALIZED"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `SYSTEM_ALREADY_INITIALIZED` | 系统已初始化，无法重复初始化 |
| `ROOT_PATH_EMPTY` | 文件存储路径不能为空 |
| `INVALID_PARAM` | 用户名或密码不能为空 |
| `PASSWORD_TOO_SHORT` | 密码长度不能少于8位 |
| `RECYCLE_BIN_PATH_INVALID` | 回收站路径不能是存储路径或其子路径 |

## 3. 文件夹浏览接口

**路径**：POST /api/system/browse

**功能**：浏览文件夹目录结构，仅当系统未初始化时可调用。用于初始化时选择文件存储路径。

**请求参数**：
```json
{
  "path": "C:\\Users"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| path | string | 否 | 要浏览的路径，为空时返回根目录（Windows返回盘符列表，Linux/OSX返回根目录） |

**返回值**：

**成功响应**：
```json
{
  "folders": [
    {
      "name": "Users",
      "path": "C:\\Users"
    }
  ],
  "has_parent": true,
  "parent_path": "C:\\"
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| folders | array | 文件夹列表 |
| folders[].name | string | 文件夹名称 |
| folders[].path | string | 文件夹完整路径 |
| has_parent | boolean | 是否存在父级文件夹 |
| parent_path | string | 父级文件夹路径（仅当 has_parent 为 true 时返回） |

**失败响应**：
```json
{
  "success": false,
  "fail_code": "SYSTEM_ALREADY_INITIALIZED"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `SYSTEM_ALREADY_INITIALIZED` | 系统已初始化，无法使用此接口 |
| `INVALID_PARAM` | path contains invalid characters or path traversal |

## 4. 获取系统设置

**路径**：POST /api/system/get_settings

**功能**：获取系统设置，包括系统名称、会话超时、笔记全文搜索状态。仅管理员可调用。

**请求参数**：无

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "system_name": "BrookFile",
  "session_timeout": 1800,
  "notebook_fulltext_search": true,
  "has_logo": false
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| system_name | string | 系统名称 |
| session_timeout | number | 会话超时时间（秒） |
| notebook_fulltext_search | boolean | 是否启用笔记全文搜索 |
| has_logo | boolean | 是否已上传系统 Logo |

## 5. 更新系统设置

**路径**：POST /api/system/update_settings

**功能**：更新系统设置。当笔记全文搜索状态变更时，自动标记需要重建索引。仅管理员可调用。

**请求参数**：
```json
{
  "system_name": "BrookFile",
  "session_timeout": 1800,
  "notebook_fulltext_search": true
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| system_name | string | 是 | 系统名称，不能为空 |
| session_timeout | number | 是 | 会话超时时间（秒），最小 60 |
| notebook_fulltext_search | boolean | 是 | 是否启用笔记全文搜索 |

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
| `INVALID_PARAM` | 系统名称为空或会话超时为 0 |

## 6. 重建笔记索引

**路径**：POST /api/system/rebuild_notebook_index

**功能**：手动触发重建所有非加密笔记的全文索引，在后台线程执行。仅管理员可调用。

**请求参数**：无

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
  "fail_code": "FULLTEXT_SEARCH_DISABLED"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `FULLTEXT_SEARCH_DISABLED` | 全文搜索未启用 |

## 7. 上传系统 Logo

**路径**：POST /api/system/upload_logo

**功能**：上传系统 Logo 图片，支持 jpg/jpeg/png/gif/webp/svg 格式，最大 2MB。仅管理员可调用。

**请求参数**：multipart/form-data，字段名为 `logo`

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
  "fail_code": "INVALID_FILE_TYPE"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `INVALID_FILE_TYPE` | 不支持的文件类型 |
| `FILE_TOO_LARGE` | 文件超过 2MB |
| `NO_FILE_UPLOADED` | 未上传文件 |

## 8. 获取系统 Logo

**路径**：POST /api/system/logo

**功能**：获取系统 Logo 图片，公开接口，无需登录。

**请求参数**：无

**返回值**：图片二进制数据，Content-Type 为对应 MIME 类型。未找到时返回 404。

## 9. 删除系统 Logo

**路径**：POST /api/system/delete_logo

**功能**：删除已上传的系统 Logo。仅管理员可调用。

**请求参数**：无

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
  "fail_code": "LOGO_NOT_FOUND"
}
```

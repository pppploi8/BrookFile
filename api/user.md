# 用户模块接口

## 1. 获取用户列表接口

**路径**：POST /api/user/list

**功能**：获取所有用户列表。仅管理员可访问。此接口不返回 `success` 字段，直接返回数组。

**请求参数**：无

**返回值**：

**成功响应**：
```json
[
  {
    "id": "uuid-string",
    "username": "admin",
    "root_path": "/data",
    "recycle_bin_path": "/recycle",
    "is_admin": true,
    "expire_at": "2025-12-31T23:59:59Z",
    "remark": "管理员",
    "feature_order": "file,music,video,book,note,password",
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-01T00:00:00Z"
  }
]
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| id | string | 用户ID |
| username | string | 用户名 |
| root_path | string | 文件存储根路径 |
| recycle_bin_path | string | 回收站路径 |
| is_admin | boolean | 是否为管理员 |
| expire_at | string | 过期时间（ISO 8601格式） |
| remark | string | 备注 |
| feature_order | string | 功能排序，逗号分隔 |
| created_at | string | 创建时间 |
| updated_at | string | 更新时间 |

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `PERMISSION_DENIED` | 权限不足，仅管理员可访问 |

## 2. 获取用户接口

**路径**：POST /api/user/get

**功能**：根据id获取用户信息。仅管理员可访问。

**请求参数**：
```json
{
  "id": "uuid-string"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| id | string | 是 | 用户ID |

**返回值**：

**成功响应**：
```json
{
  "id": "uuid-string",
  "username": "admin",
  "root_path": "/data",
  "recycle_bin_path": "/recycle",
  "is_admin": true,
  "expire_at": "2025-12-31T23:59:59Z",
  "remark": "管理员",
  "feature_order": "file,music,video,book,note,password",
  "created_at": "2024-01-01T00:00:00Z",
  "updated_at": "2024-01-01T00:00:00Z"
}
```

**失败响应**：
```json
{
  "success": false,
  "fail_code": "USER_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `PERMISSION_DENIED` | 权限不足，仅管理员可访问 |
| `USER_NOT_FOUND` | 用户不存在 |

## 3. 创建用户接口

**路径**：POST /api/user/create

**功能**：创建新用户。仅管理员可访问。

**请求参数**：
```json
{
  "username": "newuser",
  "password": "password123",
  "root_path": "/data/newuser",
  "is_admin": false,
  "expire_at": "2025-12-31T23:59:59Z",
  "remark": "备注"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| username | string | 是 | 用户名 |
| password | string | 是 | 密码 |
| root_path | string | 否 | 文件存储根路径 |
| recycle_bin_path | string | 否 | 回收站路径，留空则不开启回收站，不能是 root_path 或其子路径 |
| is_admin | boolean | 否 | 是否为管理员，默认false |
| expire_at | string | 否 | 过期时间（ISO 8601格式） |
| remark | string | 否 | 备注 |

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
  "fail_code": "USERNAME_ALREADY_EXISTS"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `PERMISSION_DENIED` | 权限不足，仅管理员可访问 |
| `USERNAME_EMPTY` | 用户名不能为空 |
| `PASSWORD_EMPTY` | 密码不能为空 |
| `USERNAME_ALREADY_EXISTS` | 用户名已存在 |
| `INVALID_PARAM` | 用户名超过64位或密码超过128位 |
| `PASSWORD_TOO_SHORT` | 密码长度不能少于8位 |
| `RECYCLE_BIN_PATH_INVALID` | 回收站路径不能是存储路径或其子路径 |
| `PATH_INVALID` | root_path contains invalid path traversal |

## 4. 更新用户接口

**路径**：POST /api/user/update

**功能**：更新用户信息。仅管理员可访问。更新成功后会强制下线该用户的所有登录会话。

**请求参数**：
```json
{
  "id": "uuid-string",
  "password": "newpassword",
  "root_path": "/new/path",
  "is_admin": false,
  "expire_at": "2025-12-31T23:59:59Z",
  "remark": "备注"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| id | string | 是 | 用户ID |
| password | string | 否 | 新密码，为空表示不更新密码 |
| root_path | string | 否 | 文件存储根路径 |
| recycle_bin_path | string/null | 否 | 回收站路径，传 null 清空回收站路径，不能是 root_path 或其子路径 |
| is_admin | boolean | 否 | 是否为管理员 |
| expire_at | string | 否 | 过期时间（ISO 8601格式） |
| remark | string | 否 | 备注 |

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
  "fail_code": "USER_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `PERMISSION_DENIED` | 权限不足，仅管理员可访问 |
| `USER_NOT_FOUND` | 用户不存在 |
| `CANNOT_MODIFY_SELF_ADMIN` | 不能修改自己的管理员身份 |
| `CANNOT_MODIFY_SELF_EXPIRE` | 不能修改自己的到期时间 |
| `RECYCLE_BIN_PATH_INVALID` | 回收站路径不能是存储路径或其子路径 |
| `PASSWORD_TOO_SHORT` | 新密码长度不能少于8位 |
| `PATH_INVALID` | 存储路径无效 |
| `INTERNAL_ERROR` | 内部错误 |

## 5. 删除用户接口

**路径**：POST /api/user/delete

**功能**：删除用户。仅管理员可访问。

**请求参数**：
```json
{
  "id": "uuid-string"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| id | string | 是 | 用户ID |

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
  "fail_code": "USER_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `PERMISSION_DENIED` | 权限不足，仅管理员可访问 |
| `USER_NOT_FOUND` | 用户不存在 |
| `CANNOT_DELETE_SELF` | 不能删除自己 |
| `INTERNAL_ERROR` | 内部错误 |

## 6. 上传头像接口

**路径**：POST /api/user/upload_avatar

**功能**：上传用户头像。需要登录后才能访问。头像存储在运行目录的 headicons/{userId}.扩展名。

**请求参数**：Content-Type: multipart/form-data

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| avatar | file | 是 | 头像图片文件，支持 jpg/jpeg/png/gif/webp 格式，最大 5MB |

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
| `NOT_LOGGED_IN` | 用户未登录 |
| `INVALID_FILE_TYPE` | 文件类型不支持 |
| `FILE_TOO_LARGE` | 文件大小超过 5MB 限制 |
| `NO_FILE_UPLOADED` | 未上传文件 |
| `INTERNAL_ERROR` | 内部错误 |

## 7. 获取头像接口

**路径**：POST /api/user/get_avatar

**功能**：获取用户头像。管理员可访问任意用户头像，普通用户只能访问自己的头像。

**请求参数**：
```json
{
  "id": "uuid-string"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| id | string | 是 | 用户ID |

**返回值**：

**成功响应**：返回头像图片文件，Content-Type 为对应的图片类型（image/jpeg、image/png、image/gif、image/webp）

**失败响应**：
```json
{
  "success": false,
  "fail_code": "AVATAR_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `PERMISSION_DENIED` | 权限不足，仅管理员可访问 |
| `INVALID_PARAM` | 参数无效（用户ID包含非法字符） |
| `AVATAR_NOT_FOUND` | 头像不存在 |

## 8. 删除头像接口

**路径**：POST /api/user/delete_avatar

**功能**：删除当前用户的头像。需要登录后才能访问。

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
  "fail_code": "AVATAR_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `AVATAR_NOT_FOUND` | 头像不存在 |

## 9. 修改密码接口

**路径**：POST /api/user/change_password

**功能**：修改当前登录用户的密码。需要登录后才能访问。修改成功后会强制下线该用户的所有登录会话。

**请求参数**：
```json
{
  "old_password": "current_password",
  "new_password": "new_password"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| old_password | string | 是 | 当前密码 |
| new_password | string | 是 | 新密码 |

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
  "fail_code": "OLD_PASSWORD_INCORRECT"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `PASSWORD_EMPTY` | 密码不能为空 |
| `INVALID_PARAM` | 新密码超过128位 |
| `PASSWORD_TOO_SHORT` | 新密码长度不能少于8位 |
| `OLD_PASSWORD_INCORRECT` | 原密码错误 |
| `INTERNAL_ERROR` | 内部错误 |

## 10. 修改功能排序接口

**路径**：POST /api/user/update_feature_order

**功能**：修改当前登录用户的功能排序。需要登录后才能访问。

**请求参数**：
```json
{
  "feature_order": "file,music,video,book,note,password"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| feature_order | string | 是 | 功能排序，逗号分隔，必须包含 file/photo/music/video/book/note/password |

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
  "fail_code": "INVALID_FEATURE_ORDER"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `INVALID_FEATURE_ORDER` | 排序格式无效 |
| `INTERNAL_ERROR` | 内部错误 |

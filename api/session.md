# 会话管理接口

## 1. 列出登录设备

**路径**：POST /api/session/list

**功能**：列出当前用户所有活跃会话（登录设备）。

**请求参数**：无

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "sessions": [
    {
      "id": "uuid-string",
      "device_name": "办公室电脑",
      "user_agent": "Mozilla/5.0 ...",
      "ip_address": "192.168.1.100",
      "created_at": 1721952000,
      "last_access_time": 1721955600,
      "is_current": true
    }
  ]
}
```

**说明**：
- 按 `last_access_time` 降序排列
- `is_current` 标识当前请求所使用的会话
- `created_at` / `last_access_time` 为 Unix 时间戳（秒）

## 2. 修改设备备注

**路径**：POST /api/session/update_name

**功能**：修改指定会话的设备备注名。仅允许修改自己的会话。

**请求参数**：
```json
{
  "session_id": "uuid-string",
  "device_name": "我的手机"
}
```

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
  "fail_code": "SESSION_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `SESSION_NOT_FOUND` | 会话不存在或不属于当前用户 |
| `INVALID_PARAMETER` | device_name 超过 100 字符 |
| `NOT_LOGGED_IN` | 用户未登录 |

## 3. 注销会话

**路径**：POST /api/session/revoke

**功能**：注销指定会话（强制下线）。仅允许注销自己的会话，不能注销当前会话。

**请求参数**：
```json
{
  "session_id": "uuid-string"
}
```

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
  "fail_code": "CANNOT_REVOKE_CURRENT"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `CANNOT_REVOKE_CURRENT` | 不能注销当前正在使用的会话 |
| `SESSION_NOT_FOUND` | 会话不存在或不属于当前用户 |
| `NOT_LOGGED_IN` | 用户未登录 |

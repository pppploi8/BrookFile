# 认证模块接口

## 1. 登录接口

**路径**：POST /api/auth/login

**功能**：用户登录。仅当系统已初始化后可调用。

**请求参数**：
```json
{
  "username": "admin",
  "password": "password123"
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
  "fail_code": "INVALID_USERNAME_OR_PASSWORD"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `INVALID_USERNAME_OR_PASSWORD` | 用户名或密码错误 |
| `SYSTEM_NOT_INITIALIZED` | 系统未初始化，无法登录 |
| `ACCOUNT_EXPIRED` | 账户已过期 |
| `INTERNAL_ERROR` | 内部错误 |

**认证机制说明**：
- 登录成功后，服务器通过 `Set-Cookie` 返回 Session ID（`HttpOnly`、`SameSite=Lax`），后续请求需携带此 Cookie
- Session Cookie 不设置 `Secure` 标志，以支持局域网 HTTP 部署场景。如需强制 HTTPS，请在反向代理层配置
- Session 超时时间默认 1800 秒（30 分钟），可通过 `config.json` 的 `session_timeout` 配置

## 2. 退出登录接口

**路径**：POST /api/auth/logout

**功能**：用户退出登录。仅当用户已登录时可调用。

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
  "fail_code": "NOT_LOGGED_IN"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `SYSTEM_NOT_INITIALIZED` | 系统未初始化 |

# WebDAV模块数据表

## WebDAV配置表 (webdav_configs)

| 字段名 | 数据类型 | 约束 | 描述 |
|-------|---------|------|------|
| id | TEXT | PRIMARY KEY | 配置ID，UUID格式 |
| user_id | TEXT | NOT NULL | 用户ID，关联 users.id |
| dav_path | TEXT | NOT NULL DEFAULT '' | DAV访问路径标识，空字符串表示根路径 |
| access_path | TEXT | NOT NULL | 映射的文件系统路径（相对于用户根目录） |
| password | TEXT | NOT NULL | 访问密码（HMAC-SHA256哈希） |
| password_salt | TEXT | NOT NULL | 密码盐值 |
| permission | TEXT | NOT NULL DEFAULT 'full_control' | 访问权限：`full_control`（完全控制）/ `edit`（编辑权限）/ `read_only`（只读权限） |
| global_access | INTEGER | NOT NULL DEFAULT 0 | 是否启用全局路径（0=否，1=是），启用后使用根路径且不可添加其他配置 |
| created_at | TIMESTAMP | DEFAULT CURRENT_TIMESTAMP | 创建时间 |
| updated_at | TIMESTAMP | DEFAULT CURRENT_TIMESTAMP | 更新时间 |
| digest_ha1 | TEXT | | Digest Auth 预计算的 HA1 = MD5(username:WebDAV:password) |

### 索引

| 索引名 | 字段 | 说明 |
|-------|------|------|
| idx_webdav_configs_user_id | user_id | 按用户查询配置 |
| idx_webdav_configs_user_dav_path | user_id, dav_path (UNIQUE) | 同一用户不允许重复的dav路径 |

### WebDAV配置表说明

- `dav_path` 只允许字母、数字、连字符、下划线，不允许为空。`global_access` 启用时强制为空
- 访问路径规则：`global_access` 启用时为 `/dav/`，否则为 `/dav/{dav_path}/`
- 同一用户下 `dav_path` 不允许重复（通过唯一索引保证）
- 不同用户可以配置相同的 `dav_path`，通过 Basic Auth 用户名区分
- `global_access` 启用时，`dav_path` 必须为空，且同一用户只能有一个 WebDAV 配置
- `global_access` 和非全局配置互斥：已存在全局配置时不能再添加其他配置，已存在其他配置时不能再开启全局
- `password` 使用 HMAC-SHA256 哈希存储，`password_salt` 为随机生成的盐值
- `access_path` 为相对于用户根目录的文件夹路径
- `permission` 对应操作权限：
  - `full_control`：浏览/上传/下载/创建目录/移动/删除
  - `edit`：浏览/上传/下载/创建目录/移动（不可删除）
  - `read_only`：仅浏览/下载
- WebDAV 认证支持 Basic Auth（仅 HTTPS）和 Digest Auth（HTTP 和 HTTPS 均支持）
- HTTPS 连接：同时支持 Basic 和 Digest 认证
- HTTP 连接：仅支持 Digest 认证，拒绝 Basic Auth
- `digest_ha1` 在创建或更新密码时自动计算并存储，用于 Digest Auth 验证
- 现有配置需重新设置密码才会生成 `digest_ha1`，否则 Digest Auth 不可用
- 删除用户时自动清理该用户的所有 WebDAV 配置

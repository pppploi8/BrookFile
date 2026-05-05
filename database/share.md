# 分享模块数据表

## 分享表 (shares)

| 字段名 | 数据类型 | 约束 | 描述 |
|-------|---------|------|------|
| id | TEXT | PRIMARY KEY | 分享ID，UUID格式 |
| user_id | TEXT | NOT NULL | 创建者用户ID，关联 users.id |
| file_path | TEXT | NOT NULL | 文件/文件夹相对路径（相对于用户根目录） |
| file_name | TEXT | NOT NULL | 文件/文件夹名称（冗余存储） |
| is_directory | INTEGER | NOT NULL DEFAULT 0 | 是否为文件夹（0否/1是） |
| share_code | TEXT | NOT NULL UNIQUE | 8位随机字母数字分享码 |
| expire_type | TEXT | NOT NULL DEFAULT 'permanent' | 过期类型：`permanent`（永久）/ `time`（按时间）/ `count`（按次数） |
| expire_at | TIMESTAMP | NULL | 过期时间，expire_type=time 时有值 |
| max_downloads | INTEGER | NULL | 最大下载次数，expire_type=count 时有值 |
| download_count | INTEGER | NOT NULL DEFAULT 0 | 已下载次数 |
| share_mode | TEXT | NOT NULL DEFAULT 'page' | 分享模式：`page`（下载页）/ `direct`（直链） |
| password | TEXT | NULL | 访问密码（HMAC-SHA256哈希），NULL表示无密码 |
| created_at | TIMESTAMP | DEFAULT CURRENT_TIMESTAMP | 创建时间 |
| updated_at | TIMESTAMP | DEFAULT CURRENT_TIMESTAMP | 更新时间 |

### 索引

| 索引名 | 字段 | 说明 |
|-------|------|------|
| idx_shares_user_id | user_id | 按用户查询分享列表 |
| idx_shares_user_path | user_id, file_path | 检查文件是否已有分享 |

### 分享表说明

- 同一用户同一文件路径只允许存在一条分享记录，重复分享时返回已有记录
- 分享状态（active/expired/file_missing）不存储在数据库中，由接口实时计算：
  - `expired`：超过过期时间（expire_type=time）或下载次数达到上限（expire_type=count）
  - `file_missing`：物理文件被删除，通过文件系统检查判断
  - `active`：以上条件均不满足
- `share_code` 为8位随机字母数字字符串，生成时需检查唯一性
- `password` 使用 HMAC-SHA256 哈希存储，与用户密码加密方式一致
- `file_name` 冗余存储用于分享页面展示，避免路径解析
- 文件大小信息不存入数据库，由 `/api/share/info` 接口实时从文件系统读取
- 文件夹分享时下载自动打包为 zip
- 已过期记录在 updated_at 超过 7 天后由定时任务自动从数据库删除
- 删除用户时会自动清理该用户的所有分享记录

# Session 模块数据表

## 会话表 (sessions)

| 字段名 | 数据类型 | 约束 | 描述 |
|-------|---------|------|------|
| id | TEXT | PRIMARY KEY | Session ID (UUID) |
| user_id | TEXT | NOT NULL | 用户 ID，关联 users.id |
| created_at | INTEGER | NOT NULL | 创建时间（Unix 时间戳，秒） |
| last_access_time | INTEGER | NOT NULL | 最后访问时间（Unix 时间戳，秒） |

### 索引

| 索引名 | 字段 | 说明 |
|-------|------|------|
| idx_sessions_user_id | user_id | 按用户查询会话 |
| idx_sessions_last_access | last_access_time | 清理过期会话 |

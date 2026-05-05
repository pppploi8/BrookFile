# 密码库模块数据表

## 密码库表 (vaults)

| 字段名 | 数据类型 | 约束 | 描述 |
|-------|---------|------|------|
| id | TEXT | PRIMARY KEY | 密码库ID，UUID格式 |
| user_id | TEXT | NOT NULL | 所属用户ID，关联 users.id |
| name | TEXT | NOT NULL | 密码库名称 |
| description | TEXT | DEFAULT '' | 描述 |
| path | TEXT | NOT NULL | 密码库文件所在目录（相对路径），如 "密码库" |
| filename | TEXT | NOT NULL | 密码库文件名，如 "personal.dat" |
| created_at | TIMESTAMP | DEFAULT CURRENT_TIMESTAMP | 创建时间 |
| updated_at | TIMESTAMP | DEFAULT CURRENT_TIMESTAMP | 更新时间 |

### 索引

| 索引名 | 字段 | 说明 |
|-------|------|------|
| idx_vaults_user_id | user_id | 按用户查询密码库列表 |

### 密码库表说明

- `path` + `filename` 拼接为密码库文件在用户网盘中的相对路径
- `rounds` 不存储在数据库中，明文存储在密码库文件内
- 删除密码库时仅删除数据库记录，不删除网盘文件，文件由用户在文件管理中手动删除
- 删除用户时会自动清理该用户的所有密码库记录
- 导入密码库时会检查该 path + filename 组合是否已被当前用户导入

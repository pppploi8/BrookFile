# 用户模块数据表

## 用户表 (users)

| 字段名 | 数据类型 | 约束 | 描述 |
|-------|---------|------|------|
| id | TEXT | PRIMARY KEY | 用户ID，UUID格式 |
| username | TEXT | UNIQUE NOT NULL | 用户名，唯一且非空 |
| password_hash | TEXT | NOT NULL | 密码哈希值，非空 |
| password_salt | TEXT | NOT NULL | 密码盐值，非空 |
| root_path | TEXT | NULL | 用户根路径 |
| is_admin | INTEGER | DEFAULT 0 | 是否为管理员（0否/1是） |
| expire_at | TIMESTAMP | NULL | 到期时间，null表示永久有效 |
| remark | TEXT | NULL | 备注信息 |
| recycle_bin_path | TEXT | | 回收站路径 |
| feature_order | TEXT | DEFAULT 'file,photo,music,video,book,note,password' | 功能排序，逗号分隔 |
| created_at | TIMESTAMP | DEFAULT CURRENT_TIMESTAMP | 创建时间 |
| updated_at | TIMESTAMP | DEFAULT CURRENT_TIMESTAMP | 更新时间 |

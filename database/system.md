# 系统模块数据表

## 系统配置表 (system_config)

| 字段名 | 数据类型 | 约束 | 描述 |
|-------|---------|------|------|
| key | TEXT | PRIMARY KEY | 配置键，主键 |
| value | TEXT | NOT NULL | 配置值 |

### 系统配置项说明

| key | 描述 |
|-----|------|
| initialized | 系统是否已初始化，值为 "true" 或 "false" |
| version | 数据库版本号 |
| system_name | 系统名称，显示在登录页和主页标题 |

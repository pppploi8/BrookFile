# 数据库文档

## 数据库表结构

| 表名 | 说明 |
|-----|------|
| users | 用户表，存储用户账号信息 |
| system_config | 系统配置表，存储系统初始化状态和配置项 |
| upload_cache | 上传缓存表，记录分块上传的临时状态 |
| compress_cache | 压缩缓存表，记录文件夹压缩任务的临时状态 |
| backup_rules | 备份规则表，存储用户的备份配置 |
| backup_logs | 备份日志表，记录备份任务的执行历史 |
| vaults | 密码库表，存储密码库的元信息 |

## 表详情

表详情按模块分类，请参阅以下文档：
- [系统模块数据表](database/system.md)
- [用户模块数据表](database/user.md)
- [文件模块数据表](database/file.md)
- [备份模块数据表](database/backup.md)
- [密码库模块数据表](database/vault.md)

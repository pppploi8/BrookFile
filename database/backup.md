# 备份模块数据表

## 备份规则表 (backup_rules)

| 字段名 | 数据类型 | 约束 | 描述 |
|-------|---------|------|------|
| id | TEXT | PRIMARY KEY | 备份规则ID，UUID格式 |
| user_id | TEXT | NOT NULL | 所属用户ID，关联 users.id |
| name | TEXT | NOT NULL | 备份规则名称 |
| storage_type | TEXT | NOT NULL | 存储类型，目前支持 `webdav` |
| storage_config | TEXT | NOT NULL | 存储配置，JSON格式 |
| local_path | TEXT | NOT NULL | 本地备份路径（相对路径） |
| encrypted | INTEGER | DEFAULT 0 | 是否加密备份（0否/1是） |
| backup_password | TEXT | NULL | 备份加密密码（HMAC-SHA256加密存储） |
| cycle | TEXT | NOT NULL | 备份周期：daily/weekly/monthly/yearly |
| backup_time | TEXT | NOT NULL | 备份时间配置，JSON格式 |
| last_backup_time | TIMESTAMP | NULL | 上次备份时间 |
| created_at | TIMESTAMP | DEFAULT CURRENT_TIMESTAMP | 创建时间 |
| updated_at | TIMESTAMP | DEFAULT CURRENT_TIMESTAMP | 更新时间 |

### 备份规则表说明

- `status` 字段不入库，由接口动态计算：如果 BackupManager 中有正在运行的任务则为 `running`，否则为 `idle`
- 删除规则时，如果有正在运行的任务会自动取消后再删除
- 编辑规则时不影响正在运行的任务，下次执行时使用新配置

### storage_config 字段说明

根据 `storage_type` 不同，`storage_config` 存储不同的 JSON 结构：

**当 storage_type = "webdav" 时**：
```json
{
  "address": "https://webdav.example.com",
  "username": "user",
  "password": "encrypted_password",
  "path": "/backup/main"
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| address | string | WebDAV 服务地址 |
| username | string | WebDAV 用户名 |
| password | string | WebDAV 密码（HMAC-SHA256加密存储，key为 `_brookfile_backup_key_`） |
| path | string | WebDAV 存储路径 |

### backup_time 字段说明

根据 `cycle` 不同，`backup_time` 存储不同的 JSON 结构：

**当 cycle = "daily" 时**：
```json
{
  "time": "08:00"
}
```

**当 cycle = "weekly" 时**：
```json
{
  "week_day": 1,
  "time": "08:00"
}
```
`week_day` 取值 1-7，对应周一到周日。

**当 cycle = "monthly" 时**：
```json
{
  "month_day": 15,
  "time": "08:00"
}
```
`month_day` 取值 1-31。

**当 cycle = "yearly" 时**：
```json
{
  "year_date": "02-21",
  "time": "08:00"
}
```
`year_date` 格式为 MM-DD。

### 备份规则表说明

- 每个用户可以有多个备份规则
- `storage_config` 中的密码字段和 `backup_password` 使用 HMAC-SHA256 加密存储，key 为 `_brookfile_backup_key_`
- 下次备份时间根据 `cycle` 和 `backup_time` 动态计算，不入库

## 备份日志表 (backup_logs)

| 字段名 | 数据类型 | 约束 | 描述 |
|-------|---------|------|------|
| id | TEXT | PRIMARY KEY | 日志ID，UUID格式 |
| backup_rule_id | TEXT | NOT NULL | 关联的备份规则ID，关联 backup_rules.id |
| mode | TEXT | DEFAULT 'full' | 执行模式：full(备份+清理) / cleanup_only(仅清理) |
| status | TEXT | NOT NULL | 任务状态：running(执行中) / completed(成功) / failed(失败) / cancelled(已取消) / interrupted(中断) |
| backup_success_count | INTEGER | DEFAULT 0 | 备份成功的文件数量 |
| backup_fail_count | INTEGER | DEFAULT 0 | 备份失败的文件数量 |
| cleanup_deleted_count | INTEGER | DEFAULT 0 | 清理删除的文件数量 |
| fail_reason | TEXT | NULL | 失败原因描述 |
| started_at | TIMESTAMP | NOT NULL | 任务开始时间 |
| finished_at | TIMESTAMP | NULL | 任务结束时间（任务完成时设置） |

### 索引

| 索引名 | 字段 | 说明 |
|-------|------|------|
| idx_backup_logs_rule_started | backup_rule_id, started_at | 联合索引，用于日志查询和清理 |

### 备份日志表说明

- 记录备份任务的执行历史
- `mode` 字段记录任务执行模式
- `status` 包含：running(执行中)、completed(成功完成)、failed(失败)、cancelled(用户取消)、interrupted(系统中断，如重启)
- 通过联合索引支持按规则ID和时间范围高效查询
- 每个规则最多保留1000条历史日志，超过后自动清理最旧的记录

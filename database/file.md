# 文件模块数据表

## 上传缓存表 (upload_cache)

| 字段名 | 数据类型 | 约束 | 描述 |
|-------|---------|------|------|
| id | TEXT | PRIMARY KEY | 上传任务ID（UUID） |
| file_path | TEXT | NOT NULL | 文件相对路径（目标路径） |
| temp_file_path | TEXT | NOT NULL | 临时文件完整路径（系统临时目录） |
| created_at | TIMESTAMP | DEFAULT CURRENT_TIMESTAMP | 创建时间 |
| last_updated_at | TIMESTAMP | DEFAULT CURRENT_TIMESTAMP | 最后更新时间 |

### 上传缓存表说明

用于记录分块上传的临时状态，支持断点续传和超时清理。后台清理线程会自动清理超过5分钟未更新的上传任务及其对应的临时文件。

## ~~压缩缓存表 (compress_cache)~~ 已移除

文件夹压缩下载已改为流式响应，不再使用此表。

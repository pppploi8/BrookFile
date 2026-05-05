# 笔记本模块数据表

## 笔记本表 (notebooks)

| 字段名 | 数据类型 | 约束 | 描述 |
|-------|---------|------|------|
| id | TEXT | PRIMARY KEY | 笔记本ID，UUID格式 |
| user_id | TEXT | NOT NULL | 所属用户ID |
| name | TEXT | NOT NULL | 笔记本名称 |
| description | TEXT | | 笔记本说明 |
| path | TEXT | NOT NULL | 存储路径（相对于用户 root_path，不以 `/` 开头） |
| encrypted | INTEGER | DEFAULT 0 | 是否加密（0否/1是，以前端传入值为准直接存入数据库） |
| created_at | TIMESTAMP | DEFAULT CURRENT_TIMESTAMP | 创建时间 |
| updated_at | TIMESTAMP | DEFAULT CURRENT_TIMESTAMP | 更新时间 |

### 索引

| 索引名 | 字段 | 类型 | 说明 |
|-------|------|------|------|
| idx_notebooks_user_id | user_id | 普通索引 | 按用户查询笔记本列表 |

### 笔记本表说明

- `path` 不要求唯一，允许多个用户将相同路径注册为笔记本，按 `user_id` 隔离；同一用户不能为同一路径创建多个笔记本（数据库有 `(user_id, path)` 唯一索引约束）
- `encrypted` 字段以前端传入值为准直接存入数据库（创建和导入笔记本时均由前端决定）；打开笔记本时前端以数据库值为准决定是否弹密码框
- 导入笔记本时，当 `encrypted=1`，后端额外校验 `.notebook.sig` 文件是否存在（二次校验），但不以文件存在与否为入库依据
- 如果加密笔记本的 `.notebook.sig` 文件被删除，数据库仍记录 `encrypted=1`，前端解锁时会因找不到签名文件而失败
- `path` 和 `encrypted` 字段不可通过更新接口修改
- 删除笔记本仅删除数据库记录，不删除物理文件，同时删除对应的搜索数据库文件
- 允许笔记本路径嵌套（如 `notes` 和 `notes/sub`），但不允许在已有加密笔记本的子路径中创建加密笔记本

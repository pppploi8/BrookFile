# 电子书模块数据表

电子书模块使用独立的 SQLite 数据库（`metadata.db`），位于电子书数据目录下的 `.brookfile/` 子目录中。

## 数据库位置

```
{ebook_path}/.brookfile/metadata.db
```

- `ebook_path` 为用户配置的电子书数据目录（相对于用户 root_path）
- 数据库使用 WAL 模式（`PRAGMA journal_mode=WAL`）
- 当前 schema 版本：2（`PRAGMA user_version = 2`）

## 元数据表 (meta)

| 字段名 | 数据类型 | 约束 | 描述 |
|-------|---------|------|------|
| key | TEXT | PRIMARY KEY | 元数据键 |
| value | TEXT | | 元数据值 |

## 书籍表 (books)

| 字段名 | 数据类型 | 约束 | 描述 |
|-------|---------|------|------|
| id | TEXT | PRIMARY KEY | 书籍ID，UUID格式 |
| path | TEXT | UNIQUE | 源文件相对路径（相对于用户 root_path） |
| title | TEXT | | 书籍标题 |
| author | TEXT | | 作者（EPUB 提取，其他格式为空） |
| format | TEXT | | 文件格式：txt/epub/pdf |
| cover_path | TEXT | | 封面路径（遗留字段，当前未使用） |
| file_size | INTEGER | | 文件大小（字节） |
| added_at | TEXT | | 添加时间 |
| updated_at | TEXT | | 更新时间 |
| sha256 | TEXT | | 文件 SHA-256 哈希值（用于去重和内容变化检测） |
| category_id | TEXT | | 所属分类ID，外键关联 category.id |
| scan_status | TEXT | DEFAULT 'pending' | 扫描状态：pending/scanning/ready/failed |
| preview_filename | TEXT | | 预览图文件名（存储在 .brookfile/previews/ 下） |
| txt_encoding | TEXT | | TXT 文件的检测编码（如 utf-8/gbk） |
| txt_cache_filename | TEXT | | TXT 转码缓存文件名（存储在 .brookfile/cache/ 下） |

### 索引

| 索引名 | 字段 | 类型 | 说明 |
|-------|------|------|------|
| （自动） | path | UNIQUE | 路径唯一约束，用于去重 |

### 去重规则

添加书籍时，通过 `path + file_size + sha256` 三元组判断是否重复。三者完全一致则视为重复书籍，跳过添加。

## 分类表 (category)

| 字段名 | 数据类型 | 约束 | 描述 |
|-------|---------|------|------|
| id | TEXT | PRIMARY KEY | 分类ID，UUID格式 |
| name | TEXT | NOT NULL | 分类名称 |
| sort_order | INTEGER | DEFAULT 0 | 排序序号，新建时自动取当前最大值+1 |

### 说明

- 删除分类时，该分类下所有书籍的 `category_id` 自动置为 NULL（移至根目录）
- 分类列表按 `sort_order ASC, name ASC` 排序

## 章节索引表 (chapter_index)

| 字段名 | 数据类型 | 约束 | 描述 |
|-------|---------|------|------|
| book_id | TEXT | NOT NULL | 所属书籍ID |
| chapter_no | INTEGER | NOT NULL | 章节序号（从1开始） |
| title | TEXT | | 章节标题（可为空） |
| location | TEXT | NOT NULL | 章节定位信息（txt: UTF-16 码元偏移，与前端 JS 字符串索引一致；epub: spine id；pdf: 页码） |

### 索引

| 索引名 | 字段 | 类型 | 说明 |
|-------|------|------|------|
| （主键） | (book_id, chapter_no) | PRIMARY KEY | 复合主键，防止重复 |

### 说明

- 重新扫描书籍元数据时，先删除该书所有章节记录，再重新插入
- 章节按 `chapter_no ASC` 排序

## 书签/进度表 (bookmark)

| 字段名 | 数据类型 | 约束 | 描述 |
|-------|---------|------|------|
| id | TEXT | PRIMARY KEY | 书签/进度ID，UUID格式 |
| book_id | TEXT | NOT NULL | 所属书籍ID |
| bookmark_type | TEXT | NOT NULL | 类型：free（自由书签）/ progress_1/progress_2/progress_3（进度槽） |
| content_coord | TEXT | NOT NULL | 内容坐标（txt: 章节号:字符偏移；epub: CFI；pdf: 页码:滚动比例） |
| summary | TEXT | | 位置摘要文本 |
| created_at | TEXT | NOT NULL | 创建时间 |
| updated_at | TEXT | NOT NULL | 更新时间 |

### 说明

- 书签和阅读进度共用此表，通过 `bookmark_type` 字段区分
- `bookmark_type = 'free'` 表示自由书签，由用户手动添加
- `bookmark_type = 'progress_1'`、`'progress_2'`、`'progress_3'` 表示3个进度槽位
- 进度槽通过 `(book_id, bookmark_type)` 隐式唯一约束：保存进度时先查询是否存在，存在则 UPDATE，不存在则 INSERT
- 自由书签按 `created_at ASC` 排序
- 进度记录按 `updated_at DESC` 排序

## 阅读进度表 (reading_progress)（遗留）

| 字段名 | 数据类型 | 约束 | 描述 |
|-------|---------|------|------|
| book_id | TEXT | PRIMARY KEY | 所属书籍ID |
| position | REAL | | 阅读位置比例 |
| page | INTEGER | | 页码 |
| cfi | TEXT | | EPUB CFI 定位 |
| updated_at | TEXT | | 更新时间 |

### 说明

- 此表为 schema v1 遗留结构，当前版本未使用
- 阅读进度功能已迁移至 `bookmark` 表，通过 `bookmark_type = 'progress_{slot}'` 实现

## 文件存储结构

```
{ebook_path}/
├── .brookfile/
│   ├── metadata.db          # 元数据库
│   ├── previews/            # 封面预览图目录
│   │   └── {book_id}.png    # 每本书的预览图
│   └── cache/               # TXT 转码缓存目录
│       └── {book_id}.txt    # UTF-8 转码后的 TXT 文件
```

## Schema 迁移机制

- `PRAGMA user_version` 记录当前 schema 版本
- 新建数据库时直接创建所有表并设置 `user_version = 2`
- 从旧版本升级时调用 `migrate_incremental()` 逐步迁移
- `add_missing_columns()` 确保旧数据库缺少的列被补全（通过 `pragma_table_info` 检查）

## 删除书籍的级联清理

删除书籍时，后端自动清理以下关联数据：
1. `chapter_index` 表中该书的章节记录
2. `bookmark` 表中该书的所有书签和进度记录
3. `reading_progress` 表中该书的遗留进度记录（如有）
4. `previews/` 目录下该书的预览图文件
5. `cache/` 目录下该书的 TXT 转码缓存文件

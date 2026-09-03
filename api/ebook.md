# 电子书模块接口

## 1. 获取电子书配置

**路径**：POST /api/ebook/config/get

**功能**：获取当前用户的电子书数据目录配置及数据库状态。需要登录后才能访问。

**请求参数**：无

**返回值**：

```json
{
  "success": true,
  "ebook_path": "ebook_data",
  "ebook_enabled": true,
  "ebook_db_status": "ok"
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| success | boolean | 是否成功 |
| ebook_path | string\|null | 电子书数据目录相对路径，未配置时为 null |
| ebook_enabled | boolean | 是否已启用电子书功能 |
| ebook_db_status | string | 数据库状态：ok/missing/corrupt |

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `INTERNAL_ERROR` | 内部错误 |

## 2. 设置电子书配置

**路径**：POST /api/ebook/config/set

**功能**：设置或更换电子书数据目录。设置空串关闭电子书功能。新目录为空时自动初始化元数据库，已有数据时直接恢复。需要登录后才能访问。

**请求参数**：
```json
{
  "ebook_path": "ebook_data"
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| ebook_path | string | 电子书数据目录相对路径，空串表示关闭 |

**返回值**：

**成功响应**：
```json
{
  "success": true
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `PATH_INVALID` | 路径不安全或不存在 |
| `EBOOK_DB_INIT_FAILED` | 元数据库初始化失败 |
| `EBOOK_DB_INVALID` | 元数据库损坏或版本不匹配 |
| `INTERNAL_ERROR` | 内部错误 |

## 3. 书架列表

**路径**：POST /api/ebook/shelf/list

**功能**：获取书架上的所有书籍和分类列表。需要登录且已配置电子书目录。

**请求参数**：无

**返回值**：
```json
{
  "success": true,
  "books": [
    {
      "id": "uuid",
      "title": "书名",
      "format": "txt",
      "file_size": 1024,
      "sha256": "abc123...",
      "category_id": null,
      "added_at": "2026-01-15T10:30:00",
      "scan_status": "ready",
      "has_preview": true
    }
  ],
  "categories": [
    {
      "id": "uuid",
      "name": "分类名",
      "sort_order": 1
    }
  ]
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `EBOOK_NOT_CONFIGURED` | 未配置电子书目录 |
| `INTERNAL_ERROR` | 内部错误 |

## 4. 添加书籍

**路径**：POST /api/ebook/shelf/add

**功能**：添加一本或多本书籍到书架。自动计算文件尺寸+SHA-256+路径三元组去重。添加后自动触发后台元数据扫描。需要登录且已配置电子书目录。

**请求参数**：
```json
{
  "items": [
    { "path": "books/example.txt" }
  ]
}
```

**返回值**：
```json
{
  "success": true,
  "added": [
    { "id": "uuid", "title": "example", "format": "txt" }
  ],
  "duplicates": [
    { "path": "books/example.txt", "title": "example" }
  ],
  "batch_id": "uuid"
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| added | array | 新增的书籍列表 |
| duplicates | array | 重复被跳过的书籍列表 |
| batch_id | string\|null | 元数据扫描批次ID，用于查询进度 |

## 5. 移除书架

**路径**：POST /api/ebook/shelf/remove

**功能**：从书架移除书籍，同时清理预览图和缓存文件。不删除网盘源文件。

**请求参数**：
```json
{
  "book_id": "uuid"
}
```

## 6. 源文件异常修复

**路径**：POST /api/ebook/shelf/relink

**功能**：文件不存在时传 new_path 重新指定路径；内容变化时不传 new_path 对原路径重算。两者均触发元数据重建。

**请求参数**：
```json
{
  "book_id": "uuid",
  "new_path": "books/new_location.txt"
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| book_id | string | 书籍ID |
| new_path | string\|null | 新文件路径（文件不存在时必传，内容变化时不传） |

**返回值**包含 `batch_id` 字段，用于查询扫描进度。

## 7. 目录扫描发现

**路径**：POST /api/ebook/scan/discover

**功能**：递归扫描指定目录，返回发现的所有支持格式的书籍文件（txt/epub/pdf）。

**请求参数**：
```json
{
  "path": "books"
}
```

**返回值**：
```json
{
  "success": true,
  "files": [
    {
      "path": "books/example.txt",
      "format": "txt",
      "title": "example",
      "file_size": 1024
    }
  ]
}
```

## 8. 扫描进度查询

**路径**：POST /api/ebook/scan/progress

**功能**：查询元数据扫描任务的进度。

**请求参数**：
```json
{
  "batch_id": "uuid"
}
```

**返回值**：
```json
{
  "success": true,
  "is_running": false,
  "total": 3,
  "completed": 3,
  "failed": 0,
  "current_book": null
}
```

## 9. 分类管理

### 9.1 分类列表

**路径**：POST /api/ebook/category/list

**请求参数**：无

**返回值**：
```json
{
  "success": true,
  "categories": [
    { "id": "uuid", "name": "分类名", "sort_order": 1 }
  ]
}
```

### 9.2 添加分类

**路径**：POST /api/ebook/category/add

**请求参数**：`{ "name": "分类名" }`

**返回值**：`{ "success": true, "id": "uuid" }`

### 9.3 重命名分类

**路径**：POST /api/ebook/category/rename

**请求参数**：`{ "id": "uuid", "name": "新名称" }`

### 9.4 删除分类

**路径**：POST /api/ebook/category/remove

**请求参数**：`{ "id": "uuid" }`

删除分类后，该分类下的书籍自动移至根目录（category_id 设为 null）。

### 9.5 移动书籍到分类

**路径**：POST /api/ebook/category/move-book

**请求参数**：
```json
{
  "book_id": "uuid",
  "category_id": "uuid"
}
```

`category_id` 为 null 时移至根目录。

## 10. 书籍元数据

**路径**：POST /api/ebook/book/meta

**功能**：获取书籍元数据、章节索引，并校验源文件状态。

**请求参数**：`{ "book_id": "uuid" }`

**返回值**：
```json
{
  "success": true,
  "book": { ... },
  "chapters": [
    { "chapter_no": 0, "title": "第一章", "location": "0" }
  ],
  "source_status": "ok"
}
```

| source_status 值 | 说明 |
|-----------------|------|
| `ok` | 源文件正常 |
| `file_not_found` | 源文件不存在 |
| `content_changed` | 源文件内容已变化（尺寸或SHA-256不匹配） |

## 11. 书籍下载

**路径**：POST /api/ebook/book/download

**功能**：下载书籍本体。txt 返回 UTF-8 转码缓存，epub/pdf 返回原文件。

**请求参数**：`{ "book_id": "uuid" }`

**返回值**：二进制文件内容，Content-Type 根据格式设置。

## 12. 预览图获取

**路径**：POST /api/ebook/preview/get

**功能**：获取书籍封面预览图（PNG格式）。

**请求参数**：`{ "book_id": "uuid" }`

**返回值**：PNG 图片二进制数据。

## 13. 阅读进度

### 13.1 保存进度

**路径**：POST /api/ebook/progress/save

**功能**：保存阅读进度到指定槽位（1-3）。

**请求参数**：
```json
{
  "book_id": "uuid",
  "slot": 1,
  "content_coord": "0:100",
  "summary": "位置摘要"
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| slot | int | 进度槽位（1-3） |
| content_coord | string | 内容坐标（txt: 章节号:字符偏移；epub: CFI；pdf: 页码:滚动比例） |
| summary | string\|null | 位置摘要文本 |

### 13.2 获取进度

**路径**：POST /api/ebook/progress/get

**请求参数**：`{ "book_id": "uuid" }`

**返回值**：
```json
{
  "success": true,
  "slots": [
    {
      "slot": 1,
      "content_coord": "0:100",
      "summary": "位置摘要",
      "updated_at": "2026-01-15T10:30:00"
    }
  ]
}
```

## 14. 书签管理

### 14.1 添加书签

**路径**：POST /api/ebook/bookmark/add

**请求参数**：
```json
{
  "book_id": "uuid",
  "content_coord": "0:50",
  "summary": "书签摘要"
}
```

**返回值**：`{ "success": true, "id": "uuid" }`

### 14.2 书签列表

**路径**：POST /api/ebook/bookmark/list

**请求参数**：`{ "book_id": "uuid" }`

**返回值**：
```json
{
  "success": true,
  "bookmarks": [
    {
      "id": "uuid",
      "content_coord": "0:50",
      "summary": "书签摘要",
      "created_at": "2026-01-15T10:30:00"
    }
  ]
}
```

### 14.3 删除书签

**路径**：POST /api/ebook/bookmark/remove

**请求参数**：`{ "id": "uuid" }`

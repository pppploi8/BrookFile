# 文件模块接口

## 1. 文件浏览接口

**路径**：POST /api/file/browse

**功能**：浏览root_path下的文件和文件夹。需要登录后才能访问。

**请求参数**：
```json
{
  "path": "subfolder"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| path | string | 否 | 要浏览的相对路径，为空时返回root_path下的内容 |

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "files": [
    {
      "name": "documents",
      "file_type": "directory",
      "size": 0,
      "modified": "1234567890"
    },
    {
      "name": "test.txt",
      "file_type": "file",
      "size": 1024,
      "modified": "1234567890"
    }
  ]
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| success | boolean | 是否成功 |
| files | array | 文件/文件夹列表 |
| files[].name | string | 文件/文件夹名称 |
| files[].file_type | string | 类型，值为 "directory"、"file" 或 "other" |
| files[].size | number | 文件大小（字节），文件夹为0 |
| files[].modified | string | 修改时间（Unix时间戳秒数） |

**失败响应**：
```json
{
  "success": false,
  "fail_code": "PATH_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `PATH_NOT_FOUND` | 路径不存在 |
| `NOT_A_DIRECTORY` | 路径不是文件夹 |
| `INVALID_FILE_PATH` | 文件路径无效（包含路径穿越等） |

## 2. 文件下载接口

**路径**：POST /api/file/download

**功能**：流式下载文件。需要登录后才能访问。

**请求参数**：
```json
{
  "path": "file.txt"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| path | string | 是 | 要下载的文件相对路径 |

**返回值**：

**成功响应**：返回文件内容，使用流式传输，不将整个文件载入内存。

**失败响应**：
```json
{
  "success": false,
  "fail_code": "PATH_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `PATH_NOT_FOUND` | 文件不存在 |
| `NOT_A_FILE` | 路径不是文件 |
| `FILE_READ_ERROR` | 文件读取失败 |
| `INVALID_FILE_PATH` | 文件路径无效（包含路径穿越等） |

## 3. 创建文件夹接口

**路径**：POST /api/file/create_folder

**功能**：创建单层文件夹。需要登录后才能访问。

**请求参数**：
```json
{
  "parent_path": "parent/folder",
  "name": "new_folder"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| parent_path | string | 否 | 父目录相对路径，为空表示在根目录创建 |
| name | string | 是 | 新建文件夹名称 |

**返回值**：

**成功响应**：
```json
{
  "success": true
}
```

**失败响应**：
```json
{
  "success": false,
  "fail_code": "FOLDER_ALREADY_EXISTS"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `FOLDER_ALREADY_EXISTS` | 文件夹已存在 |
| `FILE_ALREADY_EXISTS` | 同名文件已存在 |
| `INVALID_FOLDER_NAME` | 文件夹名称无效（包含路径分隔符等） |
| `PATH_NOT_FOUND` | 父目录不存在 |
| `CREATE_FOLDER_FAILED` | 创建文件夹失败 |
| `NOT_A_DIRECTORY` | 父路径不是文件夹 |
| `INVALID_FILE_PATH` | 父目录路径无效（包含路径穿越等） |

## 4. 删除文件接口

**路径**：POST /api/file/delete

**功能**：删除文件或文件夹。需要登录后才能访问。文件不存在视为成功。

**请求参数**：
```json
{
  "path": "file_or_folder"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| path | string | 是 | 要删除的文件或文件夹相对路径 |

**返回值**：

**成功响应**：
```json
{
  "success": true
}
```

**失败响应**：
```json
{
  "success": false,
  "fail_code": "DELETE_FAILED"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `DELETE_FAILED` | 删除失败（文件被占用等） |
| `INVALID_FILE_PATH` | 文件路径无效（包含路径穿越等） |
| `RECYCLE_MOVE_FAILED` | 移入回收站失败 |

## 5. 文件移动接口

**路径**：POST /api/file/move

**功能**：批量移动文件或文件夹到目标目录。移动前检测目标目录是否存在同名文件，如有则拒绝移动。需要登录后才能访问。

**请求参数**：
```json
{
  "files": ["file1.txt", "folder1"],
  "current_path": "source_folder",
  "target_path": "destination_folder"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| files | array | 是 | 要移动的文件/文件夹名称列表 |
| current_path | string | 否 | 当前目录相对路径，为空表示根目录 |
| target_path | string | 是 | 目标目录相对路径，为空表示根目录 |

**返回值**：

**成功响应**：
```json
{
  "success": true
}
```

**失败响应**：
```json
{
  "success": false,
  "fail_code": "FILES_ALREADY_EXIST",
  "conflict_files": ["file1.txt"]
}
```
```json
{
  "success": false,
  "fail_code": "MOVE_FAILED",
  "failed_files": ["file2.txt"]
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| success | boolean | 是否成功 |
| fail_code | string | 错误编码（仅失败时返回） |
| conflict_files | array | 冲突文件列表（仅FILES_ALREADY_EXIST时返回） |
| failed_files | array | 移动失败的文件列表（仅MOVE_FAILED时返回） |

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `NO_FILES_SPECIFIED` | 未指定要移动的文件 |
| `PATH_NOT_FOUND` | 当前目录不存在 |
| `TARGET_PATH_NOT_FOUND` | 目标目录不存在 |
| `FILES_ALREADY_EXIST` | 目标目录存在同名文件 |
| `MOVE_FAILED` | 移动失败 |
| `INVALID_FILE_NAME` | 文件名无效 |
| `INVALID_PARAM` | 参数无效（文件列表包含重复项） |
| `CANNOT_MOVE_INTO_SUBDIR` | 不能将文件夹移动到其子目录中 |

## 6. 批量删除接口

**路径**：POST /api/file/batch_delete

**功能**：批量删除文件或文件夹。文件不存在视为成功。需要登录后才能访问。

**请求参数**：
```json
{
  "files": ["file1.txt", "folder1"],
  "current_path": "some_folder"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| files | array | 是 | 要删除的文件/文件夹名称列表 |
| current_path | string | 否 | 当前目录相对路径，为空表示根目录 |

**返回值**：

**成功响应**：
```json
{
  "success": true
}
```

**失败响应**：
```json
{
  "success": false,
  "fail_code": "DELETE_FAILED",
  "data": {
    "failed_files": ["file1.txt"]
  }
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| data.failed_files | array | 删除失败的文件列表（仅DELETE_FAILED时返回） |

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `PATH_NOT_FOUND` | 当前目录不存在 |
| `DELETE_FAILED` | 部分文件删除失败（文件被占用等），失败文件列表在data.failed_files中 |
| `INVALID_FILE_PATH` | 文件路径无效（包含路径穿越等） |
| `RECYCLE_MOVE_FAILED` | 移入回收站失败 |

## 7. 上传开始接口

**路径**：POST /api/file/upload_start

**功能**：批量开始上传，在系统临时目录创建临时文件和缓存记录。需要登录后才能访问。检查文件是否存在时，同时检查本地文件和上传缓存表。

**请求参数**：
```json
{
  "files": ["file1.txt", "file2.pdf"]
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| files | array | 是 | 要上传的文件相对路径列表 |

**返回值**：

**成功响应**：
```json
{
  "success": true,
  "uploads": [
    {"id": "uuid-1", "file": "file1.txt"},
    {"id": "uuid-2", "file": "file2.pdf"}
  ]
}
```

| 字段 | 类型 | 说明 |
|-----|------|------|
| uploads | array | 上传任务列表 |
| uploads[].id | string | 上传任务ID（UUID） |
| uploads[].file | string | 文件相对路径 |

**失败响应**：
```json
{
  "success": false,
  "fail_code": "FILES_ALREADY_EXIST",
  "existing_files": ["file1.txt"]
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `FILES_ALREADY_EXIST` | 部分文件已存在（本地文件或上传缓存中） |
| `CREATE_TEMP_FILE_FAILED` | 创建临时文件失败 |
| `INVALID_FILE_PATH` | 文件路径无效（包含路径穿越等） |
| `INVALID_PARAM` | 参数无效（文件列表为空或包含重复项） |

## 8. 上传分块接口

**路径**：POST /api/file/upload_chunk

**功能**：上传文件分块。需要登录后才能访问。

**请求参数**：Content-Type: multipart/form-data

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| upload_id | string | 是 | 上传任务ID |
| offset | number | 是 | 分块偏移位置（字节） |
| chunk | binary | 是 | 文件分块数据 |

**返回值**：

**成功响应**：
```json
{
  "success": true
}
```

**失败响应**：
```json
{
  "success": false,
  "fail_code": "UPLOAD_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `UPLOAD_NOT_FOUND` | 上传任务不存在 |
| `FILE_WRITE_ERROR` | 文件写入失败 |
| `MISSING_UPLOAD_ID` | 缺少upload_id参数 |
| `MISSING_OFFSET` | 缺少offset参数 |
| `MISSING_CHUNK` | 缺少chunk参数 |
| `MULTIPART_PARSE_ERROR` | multipart解析失败 |
| `INVALID_OFFSET` | 分块偏移无效 |
| `FILE_TOO_LARGE` | 文件大小超出限制 |

## 9. 上传完成接口

**路径**：POST /api/file/upload_complete

**功能**：完成上传，将临时文件移动到目标路径，清理缓存。需要登录后才能访问。如果父目录不存在，会自动创建。

**请求参数**：
```json
{
  "upload_id": "uuid-string"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| upload_id | string | 是 | 上传任务ID |

**返回值**：

**成功响应**：
```json
{
  "success": true
}
```

**失败响应**：
```json
{
  "success": false,
  "fail_code": "UPLOAD_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `UPLOAD_NOT_FOUND` | 上传任务不存在 |
| `FILE_MOVE_ERROR` | 文件移动失败 |
| `INVALID_FILE_PATH` | 文件路径无效（包含路径穿越等） |

## 10. 取消上传接口

**路径**：POST /api/file/upload_cancel

**功能**：取消上传任务，删除缓存记录和对应的临时文件。需要登录后才能访问。

**请求参数**：
```json
{
  "upload_id": "uuid-string"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| upload_id | string | 是 | 上传任务ID |

**返回值**：

**成功响应**：
```json
{
  "success": true
}
```

**失败响应**：
```json
{
  "success": false,
  "fail_code": "UPLOAD_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `UPLOAD_NOT_FOUND` | 上传任务不存在 |

## 11. 下载文件夹接口

**路径**：POST /api/file/download_folder

**功能**：流式下载文件夹的ZIP压缩包。需要登录后才能访问。

**请求参数**：
```json
{
  "path": "documents/project"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| path | string | 是 | 文件夹相对路径 |

**返回值**：

**成功响应**：返回ZIP文件流，Content-Type: application/zip，Content-Disposition: attachment; filename="{folder_name}.zip"

**失败响应**：
```json
{
  "success": false,
  "fail_code": "PATH_NOT_FOUND"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `PATH_NOT_FOUND` | 文件夹不存在 |
| `NOT_A_DIRECTORY` | 路径不是文件夹 |
| `INVALID_FILE_PATH` | 文件路径无效（空路径或包含路径穿越）

## 12. 文件重命名接口

**路径**：POST /api/file/rename

**功能**：重命名文件或文件夹。需要登录后才能访问。

**请求参数**：
```json
{
  "path": "old_name.txt",
  "new_name": "new_name.txt"
}
```

| 字段 | 类型 | 必填 | 说明 |
|-----|------|------|------|
| path | string | 是 | 文件或文件夹相对路径 |
| new_name | string | 是 | 新名称（仅名称，不含路径） |

**返回值**：

**成功响应**：
```json
{
  "success": true
}
```

**失败响应**：
```json
{
  "success": false,
  "fail_code": "TARGET_ALREADY_EXISTS"
}
```

**错误编码说明**：
| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录 |
| `PATH_NOT_FOUND` | 文件或文件夹不存在 |
| `TARGET_ALREADY_EXISTS` | 目标名称已存在 |
| `INVALID_FILE_PATH` | 文件路径无效（空路径或包含路径穿越） |
| `INVALID_FILE_NAME` | 新名称无效（包含路径分隔符等） |
| `RENAME_FAILED` | 重命名失败 | |

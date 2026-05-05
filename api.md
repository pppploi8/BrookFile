# API 文档

## 接口概述
本项目提供以下接口：
1. **系统信息接口**：POST /api/system/info
2. **初始化接口**：POST /api/system/init
3. **文件夹浏览接口**：POST /api/system/browse
4. **登录接口**：POST /api/auth/login
5. **退出登录接口**：POST /api/auth/logout
6. **文件浏览接口**：POST /api/file/browse
7. **文件下载接口**：POST /api/file/download
8. **创建文件夹接口**：POST /api/file/create_folder
9. **删除文件接口**：POST /api/file/delete
10. **文件移动接口**：POST /api/file/move
11. **批量删除接口**：POST /api/file/batch_delete
12. **上传开始接口**：POST /api/file/upload_start
13. **上传分块接口**：POST /api/file/upload_chunk
14. **上传完成接口**：POST /api/file/upload_complete
15. **取消上传接口**：POST /api/file/upload_cancel
16. **下载文件夹接口**：POST /api/file/download_folder
17. **获取用户列表接口**：POST /api/user/list
18. **获取用户接口**：POST /api/user/get
19. **创建用户接口**：POST /api/user/create
20. **更新用户接口**：POST /api/user/update
21. **删除用户接口**：POST /api/user/delete
22. **上传头像接口**：POST /api/user/upload_avatar
23. **获取头像接口**：POST /api/user/get_avatar
24. **删除头像接口**：POST /api/user/delete_avatar
25. **修改密码接口**：POST /api/user/change_password
26. **获取备份规则列表接口**：POST /api/backup/list
27. **获取备份规则详情接口**：POST /api/backup/get
28. **创建备份规则接口**：POST /api/backup/create
29. **更新备份规则接口**：POST /api/backup/update
30. **删除备份规则接口**：POST /api/backup/delete
31. **立即开始备份接口**：POST /api/backup/start
32. **取消备份任务接口**：POST /api/backup/cancel
33. **获取任务进度接口**：POST /api/backup/progress
34. **获取历史备份日志接口**：POST /api/backup/logs
35. **获取密码库列表接口**：POST /api/vault/list
36. **创建密码库接口**：POST /api/vault/create
37. **编辑密码库接口**：POST /api/vault/update
38. **删除密码库接口**：POST /api/vault/delete
39. **导入密码库接口**：POST /api/vault/import
40. **单文件上传接口**：POST /api/vault/upload_single
41. **更新密码库元数据接口**：POST /api/vault/update_meta
42. **笔记本列表接口**：POST /api/notebook/list
43. **创建笔记本接口**：POST /api/notebook/create
44. **打开笔记本接口**：POST /api/notebook/open
45. **更新笔记本接口**：POST /api/notebook/update
46. **删除笔记本接口**：POST /api/notebook/delete
47. **读取笔记接口**：POST /api/notebook/read_note
48. **创建文件夹接口**：POST /api/notebook/create_folder
49. **保存笔记接口**：POST /api/notebook/save_note
50. **保存冲突接口**：POST /api/notebook/save_conflict
51. **文件树接口**：POST /api/notebook/file_tree
52. **重命名接口**：POST /api/notebook/rename
53. **移动接口**：POST /api/notebook/move
54. **获取附件接口**：GET /api/notebook/attachment
55. **获取附件令牌接口**：POST /api/notebook/attachment_token
56. **搜索笔记接口**：POST /api/notebook/search
57. **上传附件接口**：POST /api/notebook/upload_attachment
58. **删除文件夹接口**：POST /api/notebook/delete_folder
59. **批量删除接口**：POST /api/notebook/batch_delete
60. **获取分享信息接口**：POST /api/share/info
61. **获取下载令牌接口**：POST /api/share/get_download_token
62. **下载分享文件接口**：GET /api/share/file/{code}
63. **创建分享接口**：POST /api/share/create
64. **查询文件分享接口**：POST /api/share/get_by_path
65. **分享列表接口**：POST /api/share/list
66. **删除分享接口**：POST /api/share/delete
67. **检查恢复目标接口**：POST /api/restore/check
68. **启动恢复接口**：POST /api/restore/start
69. **恢复进度接口**：POST /api/restore/progress
70. **取消恢复接口**：POST /api/restore/cancel
71. **重试文件接口**：POST /api/restore/retry_file
72. **WebDAV配置列表接口**：POST /api/webdav/list
73. **创建WebDAV配置接口**：POST /api/webdav/create
74. **更新WebDAV配置接口**：POST /api/webdav/update
75. **删除WebDAV配置接口**：POST /api/webdav/delete
76. **回收站列表接口**：POST /api/recycle/list
77. **恢复回收站项目接口**：POST /api/recycle/restore
78. **批量恢复回收站项目接口**：POST /api/recycle/batch_restore
79. **删除回收站项目接口**：POST /api/recycle/delete
80. **批量删除回收站项目接口**：POST /api/recycle/batch_delete
81. **清空回收站接口**：POST /api/recycle/empty

## 系统全局错误编码

以下错误编码为系统通用错误编码，可在多个接口中使用：

| 错误编码 | 说明 |
|---------|------|
| `NOT_LOGGED_IN` | 用户未登录，无法执行需要登录的操作 |
| `INTERNAL_ERROR` | 内部错误，服务器处理请求时发生异常 |
| `SYSTEM_NOT_INITIALIZED` | 系统未初始化，无法执行需要初始化的操作 |

## 接口详情

接口详情按模块分类，请参阅以下文档：
- [系统模块接口](api/system.md)
- [认证模块接口](api/auth.md)
- [文件模块接口](api/file.md)
- [用户模块接口](api/user.md)
- [备份模块接口](api/backup.md)
- [密码库模块接口](api/vault.md)
- [笔记本模块接口](api/notebook.md)
- [分享模块接口](api/share.md)
- [恢复模块接口](api/restore.md)
- [WebDAV模块接口](api/webdav.md)
- [回收站模块接口](api/recycle.md)

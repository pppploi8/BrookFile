# BrookFile - AI 编码代理指南

## 项目概述

个人云存储管理系统，后端 Rust/Actix-web/SQLite，前端 Vue 3/Element Plus/TypeScript，测试 Python。

## 构建 / 检查 / 测试命令

### 后端（backend/ 目录下）

```bash
cargo check          # 类型检查，不生成产物（推荐用于验证）
cargo build          # 编译（debug）
cargo run            # 编译并运行，监听 0.0.0.0:3000
```

### 前端（frontend/ 目录下）

```bash
npm install          # 安装依赖
npm run check        # vue-tsc 类型检查（必须通过）
npm run build        # 类型检查 + vite 构建
npm run dev          # 开发服务器，0.0.0.0:8080，代理 /api → localhost:3000
```

### API 集成测试（tests/ 目录下）

每个测试脚本是独立运行的 Python 文件，会自动编译后端、启动服务器、执行测试、关闭服务器。**不支持并行测试**，必须逐个串行执行，否则会因端口冲突或数据库状态冲突导致测试失败：

```bash
cd tests
pip install -r requirements.txt

# 后端接口测试
python backend/test_auth_api.py           # 认证测试
python backend/test_system_api.py         # 系统初始化测试
python backend/test_file_api.py           # 文件浏览/下载/删除测试
python backend/test_compress_api.py       # 压缩下载测试
python backend/test_backup_api.py         # 备份功能测试
python backend/test_backup_restore_api.py # 备份恢复测试
python backend/test_backup_restore_error.py # 备份恢复错误处理测试
python backend/test_notebook_api.py       # 笔记本测试
python backend/test_recycle_api.py        # 回收站测试
python backend/test_share_api.py          # 分享测试
python backend/test_user_api.py           # 用户管理测试
python backend/test_vault_api.py          # 保险箱测试
python backend/test_webdav_api.py         # WebDAV 测试
python backend/test_webdav_protocol.py    # WebDAV 协议测试

# 前端页面测试
python frontend/test_system_init.py       # 系统初始化页面测试
python frontend/test_login.py             # 登录页面测试
python frontend/test_file_manager.py      # 文件管理器页面测试
python frontend/test_notes.py             # 笔记页面测试
python frontend/test_passwords.py         # 密码管理页面测试
python frontend/test_recycle_bin.py       # 回收站页面测试
python frontend/test_share_page.py        # 分享页面测试
python frontend/test_webdav.py            # WebDAV 页面测试
python frontend/test_backup_restore.py    # 备份恢复页面测试
python frontend/test_account_management.py # 账号管理页面测试
python frontend/test_profile_center.py    # 个人中心页面测试
python frontend/test_i18n.py              # 国际化键值检查
```

运行单个测试：直接执行对应的 `python tests/backend/xxx.py` 或 `python tests/frontend/xxx.py` 即可。

测试公共脚本在 `tests/common.py`，提供编译后端、启动后端/前端、初始化系统、登录、错误日志打印等公共函数，被 `tests/backend/test_utils.py` 和 `tests/frontend/test_utils.py` 复用。

## 后端代码规范

### 技术栈
Rust + Actix-web + r2d2 + rusqlite + serde

### 目录结构
```
backend/src/
├── main.rs              # 入口，服务器启动、 AppState 初始化
├── routes.rs            # 所有路由注册
├── app_state.rs         # 共享状态（models + managers）
├── config.rs            # 配置加载（config.json）
├── database.rs          # SQLite 连接池、建表
├── session_manager.rs   # 内存会话管理
├── error_logger.rs      # 错误日志写入 error.log
├── compress_manager.rs  # 压缩任务管理
├── handlers/            # 请求处理器（每个模块一个文件）
├── models/              # 数据模型（每个表一个文件）
├── middleware/           # 中间件（auth、session）
├── backup/              # 备份功能模块
├── restore/             # 恢复功能模块
└── storage/             # 存储功能模块
```

### 接口规范
- 除 SSE 外，统一使用 POST，不使用 RESTful 风格
- 错误响应使用 `fail_code` 字符串（全大写 + 下划线），不返回错误文本
- 成功响应：`{ "success": true }`
- 失败响应：`{ "success": false, "fail_code": "ERROR_CODE" }`
- 内部错误统一返回 `fail_code: "INTERNAL_ERROR"`，同时调用 `error_logger::log_error()` 记录

### 分层架构
- **models 层**：纯数据结构，不含 Serialize/Deserialize，命名为 `UserInfo`、`BackupRuleDetail` 等。通过 `impl From<ModelType> for ResponseType` 实现到响应类型的转换
- **handlers 层**：定义请求结构体（`#[derive(Deserialize)]`，命名 `XxxRequest`）和响应结构体（`#[derive(Serialize)]`，命名 `XxxResponse`）
- 通用工具函数在 `handlers/response.rs`：`get_current_user_id()`、`get_user_root_path()`、`check_admin()`、`internal_error_response()`
- 安全检查函数在 `handlers/security.rs`：`is_path_under_root()`、`is_safe_path()`、`is_safe_name()`

### 模块组织
- `mod.rs` 中声明子模块并使用 `pub use xxx::*` 重新导出
- 路由全部集中在 `routes.rs` 中注册

### 数据库
- SQLite，连接池 r2d2，SQL 使用 `params![]` 宏
- 表结构定义在 `database.rs` 的 `init_tables()` 中
- 修改表结构时必须同步更新 `database/{模块}.md` 文档

### 错误处理
- models 层返回 `Result<T, String>`，使用 `.map_err(|e| e.to_string())` 转换
- handlers 层返回 `HttpResponse::Ok().json(ApiResponse { ... })`，始终 HTTP 200

## 前端代码规范

### 技术栈
Vue 3 + Element Plus + Tailwind CSS 4 + Pinia + Vue Router + Vue I18n + Axios + TypeScript

### 目录结构
```
frontend/src/
├── main.ts              # 入口，注册插件
├── App.vue              # 根组件
├── style.css            # 全局样式
├── api/system.ts        # 所有 API 调用及类型定义
├── views/               # 页面视图
├── components/          # 可复用组件
├── stores/              # Pinia 状态管理
├── router/              # 路由配置
└── i18n/locales/        # 中英文翻译文件
```

### TypeScript
- 严格模式：`strict: true`、`noUnusedLocals`、`noUnusedParameters`
- 路径别名：`@/` → `src/`
- API 类型定义统一在 `api/system.ts` 中

### 样式规范
- 所有页面 `<style>` 必须加 `scoped`
- 禁止使用 `:deep` 覆盖子组件样式，样式只允许全局统一调整
- 连续使用 `el-button`/`el-link` 时，全局样式已有间距，不额外添加
- 充满高度时使用 flex 布局，禁止用绝对尺寸计算

### 组件规范
- 操作列统一使用 `el-link`
- 国际化：所有 UI 文本必须同时提供中文和英文翻译（`i18n/locales/` 下）

### API 调用
- 通过 `request()` 和 `requestWithSuccess()` 封装，自动处理 `fail_code` 和错误提示
- `NOT_LOGGED_IN` 自动跳转登录页

## 测试规范

- 每个测试脚本独立完整：编译 → 创建临时 workdir → 启动后端（workdir 作为工作目录）→ 执行测试 → 关闭后端
- 每个脚本会创建全新的数据库，不依赖外部状态
- 使用 `requests.Session()` 保持会话
- 公共函数在 `tests/common.py`：`build_backend`、`start_backend`、`start_frontend`、`init_system`、`login`、`print_error_log`、`stop_backend`
- 后端测试通过 `tests/backend/test_utils.py` 的 `run_tests()` 运行，前端测试通过 `tests/frontend/test_utils.py` 的 `run_frontend_test()` 运行

## 编码行为准则

### 先思考再编码
- 不要假设，不要隐藏困惑，主动呈现权衡
- 实现前明确陈述假设，不确定就问
- 存在多种理解时，列出选项而非静默选择
- 存在更简单的方案时主动说明，必要时提出异议
- 遇到不明确的地方先停下来，指出困惑点再提问

### 简洁优先
- 只实现被要求的功能，不添加额外特性
- 单次使用的代码不做抽象
- 不主动添加未被要求的"灵活性"或"可配置性"
- 不为不可能发生的场景编写错误处理
- 如果 50 行能解决，不要写 200 行

### 精准改动
- 只修改必须改的部分，不顺手"改进"相邻代码、注释或格式
- 不重构正常工作的代码
- 匹配现有风格，即使自己会写得更不一样
- 发现无关的废弃代码时提到即可，不主动删除
- 自己的改动导致的废弃导入/变量/函数必须清理
- 每一行改动都应能追溯到用户的需求

### 目标驱动
- 将任务转化为可验证的目标：先写失败测试，再让测试通过
- 多步骤任务先简述计划，每步说明验证方式
- 强目标定义允许独立循环推进，弱目标需要反复确认

## 通用规则

- 计划修改后端/前端原有功能（非新增功能）之前，必须先向用户说明改动内容和影响，经用户确认后再执行
- 不要提前实现未来可能用到的方法
- 修改接口时同步更新 `api/{模块}.md` 文档
- 修改数据库表时同步更新 `database/{模块}.md` 文档
- 不写注释（除非明确要求）

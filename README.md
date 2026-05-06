# BrookFile

[English](./README_EN.md) | 中文

一个面向个人/家庭用户的网盘系统，完全由AI开发的实验性项目。

## 功能特色

### 基本功能

- **网盘基本功能** - 文件上传、下载、浏览、重命名、移动、复制、搜索、压缩下载
- **多租户** - 支持多用户独立使用，管理员可管理用户账号，各用户数据完全隔离
- **文件分享** - 支持通过链接分享文件或文件夹，可设置访问密码和过期时间
- **回收站** - 删除的文件进入回收站，支持恢复或彻底删除

### 扩展功能

- **加密云备份** - 安全的文件加密云备份功能
- **云笔记集成** - 便捷的云端笔记管理功能，支持端对端加密
- **密码管理集成** - 端对端加密的密码存储与管理
- **WebDAV支持** - 标准WebDAV协议支持，兼容各类客户端

## 后续开发计划

- **电子书在线阅读** - 支持多种电子书格式的在线阅读
- **音乐库和在线播放** - 在线音乐管理和流媒体播放
- **视频在线转码观看** - 视频文件在线转码与播放
- **照片浏览器** - 支持照片按时间/内容分类查看和快速搜索

## 部署说明

1. 从 [Release](../../releases) 下载对应系统版本（目前仅提供 Windows/Linux x86_64 预编译版本）
2. 解压后直接启动可执行文件
3. 浏览器访问 `http://<IP>:3000`，按提示完成系统初始化即可使用

> **其他平台说明**：macOS、ARM Linux、ARM Windows 等平台理论上均可运行，但作者没有对应设备，无法提供预编译的二进制文件。如有需要，建议使用 AI 辅助在目标平台上自行编译。

### HTTPS 部署（推荐）

正式使用时推荐通过 Nginx 反向代理的方式部署 HTTPS。笔记本、密码库等端对端加密功能依赖于 WebCrypto API，该 API 仅在 HTTPS 环境下可用。即便不考虑安全传输需求，HTTPS 下 WebCrypto 的性能也远高于 HTTP 下的 JavaScript 降级实现。

Nginx 反向代理参考配置：

```nginx
server {
    listen 443 ssl;
    server_name your-domain.com;

    ssl_certificate     /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    client_max_body_size 0;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### 配置说明

`config.json` 仅包含部署时配置（修改后需重启）：

```json
{
    "port": 3000,
    "argon2": {
        "m_cost": 19456,
        "t_cost": 2,
        "p_cost": 1
    }
}
```

| 配置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| `port` | 整数 | `3000` | 服务监听端口 |
| `argon2.m_cost` | 整数 | `19456` | Argon2 内存开销（KB），影响密码哈希计算使用的内存量 |
| `argon2.t_cost` | 整数 | `2` | Argon2 迭代次数，影响密码哈希计算耗时 |
| `argon2.p_cost` | 整数 | `1` | Argon2 并行度，影响密码哈希计算使用的线程数 |

> **注意**：`argon2` 参数影响用户登录密码和备份加密密码的哈希强度。修改这些参数后，已有密码将无法验证，需要删除 `database.db` 重新初始化系统。在低性能设备（如路由器）上，可降低 `m_cost`（如 `4096`）和 `t_cost`（如 `1`）以减少内存占用和登录耗时，但会降低密码抗暴力破解的安全性。

### 系统设置

登录管理员账号后，通过菜单中的「系统设置」可在线修改以下配置：

| 设置项 | 生效方式 | 说明 |
|--------|---------|------|
| 系统名称 | 立即生效 | 显示在登录页、浏览器标题等位置 |
| 系统Logo | 立即生效 | 自定义品牌图标，显示在侧边栏和登录页 |
| 会话超时 | 重启后生效 | 用户无操作多久后自动退出登录（秒） |
| 笔记全文搜索 | 重启后生效 | 关闭后可节省运行内存，笔记搜索仅匹配标题 |
| 重建笔记索引 | 立即执行 | 手动触发所有非加密笔记的全文索引重建 |

## 开发说明

本项目完全由AI（GLM-5/5.1）开发。

### 技术栈

#### 后端
- **Rust** - 高性能系统编程语言
- **Actix Web** - 强大的异步Web框架
- **SQLite (rusqlite)** - 轻量级嵌入式数据库
- **r2d2** - 数据库连接池
- **Serde** - 序列化/反序列化框架
- **HMAC/SHA256** - 安全加密算法

#### 前端
- **Vue 3** - 渐进式JavaScript框架
- **TypeScript** - 类型安全的JavaScript超集
- **Vite** - 下一代前端构建工具
- **Element Plus** - Vue 3 UI组件库
- **Pinia** - Vue 3 状态管理
- **Vue Router** - 官方路由管理器
- **Vue I18n** - 国际化支持
- **Tailwind CSS** - 实用优先的CSS框架
- **Axios** - HTTP客户端

#### 测试
- **Python** - API自动化测试

### 项目结构

```
BrookFile/
├── backend/           # Rust后端服务
│   ├── src/
│   │   ├── handlers/  # 请求处理器
│   │   ├── middleware/ # 中间件
│   │   ├── models/    # 数据模型
│   │   ├── storage/   # 存储功能模块
│   │   ├── backup/    # 备份功能模块
│   │   ├── restore/   # 恢复功能模块
│   │   ├── search/    # 搜索功能模块
│   │   └── ...
│   └── Cargo.toml
├── frontend/          # Vue前端应用
│   ├── src/
│   │   ├── api/       # API接口
│   │   ├── components/# 组件
│   │   ├── views/     # 页面视图
│   │   ├── stores/    # 状态管理
│   │   ├── i18n/      # 国际化
│   │   ├── router/    # 路由配置
│   │   └── ...
│   └── package.json
├── tests/             # Python自动化测试
│   ├── backend/       # 后端接口测试
│   ├── frontend/      # 前端页面测试
│   ├── common.py      # 公共测试函数
│   └── run_all.py     # 一键运行全部测试
├── api/               # 接口文档
├── database/          # 数据库文档
├── build.py           # 构建脚本
```

### 快速开始

#### 环境要求
- Rust 1.70+
- Node.js 18+
- Python 3.8+ (用于测试)

#### 后端启动

```bash
cd backend
cargo run
```

#### 前端启动

```bash
cd frontend
npm install
npm run dev
```

#### 运行测试

```bash
cd tests
pip install -r requirements.txt
python run_all.py
```

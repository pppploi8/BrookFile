# AI模块接口

AI 配置为系统级全局配置（第一版先服务于电子书 AI 问答）。供应商、模型配置为全局共享，任意登录用户均可管理，仅需登录（`NOT_LOGGED_IN` 判定），与账户无关。

**协议**：上游调用基于 **genai**（Rust 多供应商 LLM 库）实现，后端按供应商的 `provider_type`（预设类型）选择协议适配器（OpenAI Chat Completions / OpenAI Responses / Anthropic / Gemini / Ollama / DeepSeek / OpenRouter / Groq 等，见接口 1），消息、工具调用、思考等级、usage 统计均由适配器转换为各厂商的原生协议。`base_url` 为供应商**根地址**（可选，缺省使用预设默认地址，如 `https://api.openai.com/v1/`），适配器在其后拼接协议路径。**模型列表不依赖库更新**：模型名是自由字符串，新模型无需升级后端即可使用，界面可通过接口 6 从供应商拉取模型列表。思考等级 `none|low|high|max` 由适配器映射为各厂商的 reasoning/thinking 参数。

## 1. 获取供应商类型预设

**路径**：POST /api/ai/provider/presets

**功能**：列出后端支持的全部供应商类型预设（与 genai 适配器一一对应），前端配置页据此渲染供应商类型下拉与默认地址占位符。返回顺序即前端下拉的展示顺序：三个协议入口（`openai_completions`、`openai_responses`、`anthropic`）置顶，其余厂商按固定顺序跟随。

**请求参数**：无

**成功响应**：
```json
{
  "success": true,
  "presets": [
    { "type_id": "openai_completions", "name": "OpenAI (Chat Completions)", "default_base_url": "https://api.openai.com/v1/", "requires_api_key": true },
    { "type_id": "ollama", "name": "Ollama", "default_base_url": "http://localhost:11434/", "requires_api_key": false }
  ]
}
```

当前预设全集（按返回顺序）：`openai_completions`、`openai_responses`、`anthropic`、`deepseek`、`openrouter`、`groq`、`xai`、`moonshot`、`kimi`、`zai`、`fireworks`、`together`、`nebius`、`mimo`、`gemini`、`ollama`、`ollama_cloud`、`cohere`。`requires_api_key=false` 的预设（ollama）允许空密钥。

所有厂商的接口基本兼容这三个协议之一，接入非预设厂商时直接选 `openai_completions` 并覆盖 `base_url` 即可（适配器相同，仅默认地址不同）。

**错误编码**：`NOT_LOGGED_IN`

## 2. 获取供应商及模型列表

**路径**：POST /api/ai/provider/list

**功能**：获取当前用户的所有 AI 供应商及其下模型列表。

**请求参数**：无

**成功响应**：
```json
{
  "success": true,
  "providers": [
    {
      "id": "uuid-string",
      "name": "OpenAI",
      "provider_type": "openai_completions",
      "base_url": "https://api.openai.com/v1",
      "has_api_key": true,
      "proxy": "http://127.0.0.1:7890",
      "created_at": "2026-09-09 10:00:00",
      "updated_at": "2026-09-09 10:00:00",
      "models": [
        {
          "id": "uuid-string",
          "model_id": "gpt-4o",
          "supports_vision": true,
          "context_length": 128000,
          "max_output_tokens": 4096,
          "created_at": "2026-09-09 10:00:00",
          "updated_at": "2026-09-09 10:00:00"
        }
      ]
    }
  ]
}
```

**错误编码**：`NOT_LOGGED_IN`

## 3. 创建供应商（可同时添加模型，原子提交）

**路径**：POST /api/ai/provider/create

**功能**：创建供应商并保存类型、访问地址与密钥，可在同一次调用内添加模型。**供应商与模型在同一个数据库事务中提交**：任一步失败（如模型重复）则整体回滚，不会留下只有供应商而无模型的半成品（即用户在界面配置中途关闭弹窗不会产生入库）。创建接口仅用于整个配置完成后一次性提交。

**请求参数**：
```json
{
  "name": "OpenAI",
  "provider_type": "openai_completions",
  "base_url": "https://api.openai.com/v1",
  "api_key": "sk-xxx",
  "proxy": "http://127.0.0.1:7890",
  "models": [
    {
      "model_id": "gpt-4o",
      "supports_vision": true,
      "context_length": 128000,
      "max_output_tokens": 4096
    }
  ]
}
```

**备注**：`provider_type` 必填，须为接口 1 中的 `type_id`。`base_url` 可选，缺省使用预设默认地址；提供时须以 `http(s)://` 开头。`models` 可选；`proxy` 可选，仅支持 `http://` 开头的 HTTP 代理，留空表示直连（访问境外平台时需走代理）。`api_key` 在 `requires_api_key=false` 的预设（ollama）下可省略。`supports_vision`、`context_length`、`max_output_tokens` 也可选，缺省分别为 `false`、`200000`、`65536`。

**成功响应**：
```json
{
  "success": true,
  "provider_id": "uuid-string"
}
```

**错误编码**：`NOT_LOGGED_IN`、`PARAM_INVALID`、`PROVIDER_TYPE_INVALID`、`BASE_URL_INVALID`、`PROXY_INVALID`、`MODEL_DUPLICATE`（同次调用内模型重复，整体回滚）

## 4. 更新供应商（可携带模型列表整体对账，原子提交）

**路径**：POST /api/ai/provider/update

**功能**：更新供应商名称、类型、地址；`api_key` 留空表示保留原密钥，非空则替换。若携带 `models` 数组，则在**同一事务内**对账模型列表：删除被移除的、更新带 `id` 的、新增没有 `id` 的；任一步失败（如模型重复）则整体回滚，供应商信息不变。编辑供应商时前端通过此接口一次性提交全部模型改动（模型不做独立编辑接口）。

**请求参数**：
```json
{
  "id": "uuid-string",
  "name": "OpenAI",
  "provider_type": "openai_completions",
  "base_url": "https://api.openai.com/v1",
  "api_key": "sk-new",
  "proxy": "http://127.0.0.1:7890",
  "models": [
    {
      "id": "已存在的模型id(新增时省略)",
      "model_id": "gpt-4o",
      "supports_vision": true,
      "context_length": 128000,
      "max_output_tokens": 4096
    }
  ]
}
```

**备注**：`provider_type` 必填。`models` 可选；省略则仅更新供应商字段。`base_url` 可选（缺省用预设默认地址）；`api_key` 留空保留原密钥；`proxy` 会直接以请求值（可为空字符串，表示清除代理）覆盖。`supports_vision`、`context_length`、`max_output_tokens` 可选，缺省分别为 `false`、`200000`、`65536`。

**成功响应**：`{ "success": true }`

**错误编码**：`NOT_LOGGED_IN`、`PARAM_INVALID`、`PROVIDER_TYPE_INVALID`、`BASE_URL_INVALID`、`PROXY_INVALID`、`PROVIDER_NOT_FOUND`、`MODEL_DUPLICATE`、`MODEL_NOT_FOUND`

## 5. 删除供应商

**路径**：POST /api/ai/provider/delete

**功能**：删除供应商并级联删除其下所有模型。

**请求参数**：
```json
{ "id": "uuid-string" }
```

**成功响应**：`{ "success": true }`

**错误编码**：`NOT_LOGGED_IN`、`PROVIDER_NOT_FOUND`

## 6. 读取模型列表

**路径**：POST /api/ai/provider/fetch_models

**功能**：按供应商类型调用其模型列表接口（不必已入库，可来自正在配置的表单）。各类型风格不同：OpenAI 兼容族为 `GET {base_url}/models`（Bearer）；`anthropic` 为 `GET {base_url}/models`（`x-api-key` + `anthropic-version` 头）；`gemini` 为 `GET {base_url}/models`（`x-goog-api-key` 头，返回名去掉 `models/` 前缀）；`ollama`/`ollama_cloud` 为 `GET {base_url}/api/tags`；`cohere` 为 `GET {base_url}/models`（Bearer）。请求中的 `proxy` 会在出站时实际使用；留空表示直连。调用失败属于预期情况，前端应引导用户手动添加模型。**上游请求设 30 秒总超时**（含建连与读响应），超时同样返回 `AI_MODEL_LIST_FAILED`，避免供应商「连得上但永不回包」时接口永久挂住。`base_url`、`api_key` 或 `proxy` 为空且提供 `provider_id` 时，回退使用该供应商已存储的值（编辑供应商未改时使用）。

**请求参数**：
```json
{
  "provider_type": "openai_completions",
  "base_url": "https://api.openai.com/v1（可选）",
  "api_key": "sk-xxx（可选）",
  "proxy": "http://127.0.0.1:7890（可选）",
  "provider_id": "uuid-string(可选)"
}
```

**成功响应**：
```json
{
  "success": true,
  "models": ["gpt-4o", "gpt-4o-mini"]
}
```

**错误编码**：`NOT_LOGGED_IN`、`PARAM_INVALID`、`PROVIDER_TYPE_INVALID`、`BASE_URL_INVALID`、`PROXY_INVALID`、`PROVIDER_NOT_FOUND`、`AI_MODEL_LIST_FAILED`（请求失败或解析失败）

## 7. 删除模型

**路径**：POST /api/ai/model/delete

**功能**：删除某个模型配置。

**请求参数**：
```json
{ "id": "uuid-string" }
```

**成功响应**：`{ "success": true }`

**错误编码**：`NOT_LOGGED_IN`、`MODEL_NOT_FOUND`

## AI 会话

AI 会话为公共能力，任何业务通过 `biz_type`（+ `biz_id`）接入，当前电子书阅读器使用 `biz_type=ebook`。会话按用户隔离（存储于各用户 `ai_chat_path` 目录），供应商/模型配置全局共享，见上文供应商接口。

### 会话文件存储格式

```
{ai_chat_path}/
└── {功能名 biz_type}/
    ├── index.json               # 该功能的会话索引（列表数据源）
    ├── {chat_id}.json           # 单会话：元信息 + 全部消息
    └── images/                  # 消息图片，按 {message_id}_{n}.{ext} 命名
```

用户私人根目录本身已按用户隔离，存储层不再叠加用户层级，按功能名（`biz_type`）分目录。

`index.json`：`{ "chats": [{ "id", "biz_type", "biz_id", "title", "created_at", "updated_at", "context_tokens" }] }`（`context_tokens` 为该会话最近一次上游调用返回的上下文 token 总量，压缩阈值判定用，缺省不存在）

`{chat_id}.json`：索引字段 + `"messages": [...]`。消息角色为 `user` / `assistant` / `tool` / `system` / `compact`，字段对齐 OpenAI Chat Completions：

- `user`：`content` 文本；`images` 为相对路径数组（如 `images/m1_0.png`）
- `assistant`：`content`（markdown）；`reasoning` 为该次回复全部轮次合并的思维链文本（启用思考且模型产出时存在，仅最终消息携带）；发起工具调用时含 `tool_calls`（OpenAI 格式）；`model_key` 记录使用的模型配置 id
- `tool`：`tool_call_id` + `name` + `content`（工具执行结果文本）；工具产出截图时 `images` 为相对路径数组
- `system`：首轮落盘的业务系统提示（见接口 15），前端过滤不展示
- `compact`：上下文压缩摘要（见接口 15），重放时以用户消息 + `<summary>` 包装注入，前端过滤不展示

写入策略：一轮对话完成后原子重写（临时文件 + rename）；`index.json` 损坏时扫描功能目录下的 `{chat_id}.json` 即可重建。

## 8. 会话列表

**路径**：POST /api/ai/chat/list

**请求**：
```json
{ "biz_type": "ebook", "biz_id": "书籍id（全局场景传空串或省略）" }
```

**成功响应**：
```json
{
  "success": true,
  "chats": [{ "id": "uuid", "biz_type": "ebook", "biz_id": "book-1", "title": "标题", "created_at": "2026-09-12 10:00:00", "updated_at": "2026-09-12 10:05:00" }]
}
```
按 `updated_at` 倒序返回。

**错误编码**：`NOT_LOGGED_IN`、`PARAM_INVALID`、`AI_CHAT_PATH_NOT_SET`、`NO_ROOT_PATH`

## 9. 创建会话

**路径**：POST /api/ai/chat/create

**请求**：
```json
{ "biz_type": "ebook", "biz_id": "book-1", "title": "可选" }
```

**成功响应**：`{ "success": true, "chat_id": "uuid" }`

**错误编码**：`NOT_LOGGED_IN`、`PARAM_INVALID`、`AI_CHAT_PATH_NOT_SET`

## 10. 重命名会话

**路径**：POST /api/ai/chat/rename

**请求**：`{ "biz_type": "ebook", "chat_id": "uuid", "title": "新标题" }`

**成功响应**：`{ "success": true }`

**错误编码**：`NOT_LOGGED_IN`、`CHAT_NOT_FOUND`、`AI_CHAT_PATH_NOT_SET`

## 11. 删除会话

**路径**：POST /api/ai/chat/delete

**功能**：删除会话文件并从索引移除（消息随会话文件一并删除），同时清理会话内消息引用的图片文件（用户上传与工具产出的 `images/` 文件），不遗留孤儿文件。

**请求**：`{ "biz_type": "ebook", "chat_id": "uuid" }`

**成功响应**：`{ "success": true }`

**错误编码**：`NOT_LOGGED_IN`、`AI_CHAT_PATH_NOT_SET`

## 12. 消息分页

**路径**：POST /api/ai/chat/messages

**功能**：读取会话消息的窗口切片，服务于前端虚拟滚动。`before_id` 取该消息之前的 `limit` 条；`after_id` 取该消息之后的 `limit` 条；两者都缺省时取**最新** `limit` 条。消息按时间正序返回。

**请求**：
```json
{ "biz_type": "ebook", "chat_id": "uuid", "before_id": "可选", "after_id": "可选", "limit": 100 }
```

**成功响应**：
```json
{
  "success": true,
  "messages": [{ "id": "uuid", "role": "user", "content": "…", "images": [], "tool_calls": null, "tool_call_id": null, "name": null, "model_key": null, "created_at": "…" }],
  "has_more_before": true,
  "has_more_after": false
}
```
`limit` 缺省 100，上限 500。`role=tool` 的消息前端默认折叠展示；`role=system`、`role=compact` 前端过滤不展示。

**错误编码**：`NOT_LOGGED_IN`、`CHAT_NOT_FOUND`、`MESSAGE_NOT_FOUND`（游标 id 不存在）、`AI_CHAT_PATH_NOT_SET`

## 13. 发送消息（SSE 流式）

**路径**：POST /api/ai/chat/send

**功能**：保存用户消息后调用模型，以 `text/event-stream` 响应流式返回。带 `tools` 的业务（由后端按 `biz_type` 的工具注册表决定）支持多轮工具调用循环：模型发起工具调用 → 执行工具 → 结果回传模型继续生成，直至产出最终回答或达到轮次上限（每业务的 `AiToolProvider.max_tool_rounds` 决定，电子书为 10）。每轮工具调用与结果实时落盘，客户端断开或上游超时（120s）时已生成内容保留。

**请求**：
```json
{
  "biz_type": "ebook",
  "chat_id": "uuid",
  "model_key": "模型配置id",
  "content": "问题文本（与 images 至少一项非空）",
  "images": ["data:image/png;base64,…（可选，最多4张，单张≤1MB）"],
  "thinking": "none|low|high|max（可选，省略=模型默认行为）"
}
```

`thinking` 由适配器映射为各厂商的思考参数（如 OpenAI Responses 的 `reasoning.effort`）；思维链通过 `reasoning` 事件流式返回，并随该轮 assistant 消息落盘持久化（字段 `reasoning`），**不回传上下文**（不重放进后续上游请求）。

**会话标题生成**：首轮发送且会话无标题时，立即以用户提问前 50 字作为兜底标题，同时后台并行用**同一模型**（思考等级 `none`）根据首轮提问生成标题；主流程结束（含断连/出错）后才写入，避免与对话落盘互相覆盖。生成失败/超时保留兜底标题；用户手动重命名后不再覆盖。标题变化在会话列表刷新时展示。

**业务上下文注入（首轮落盘）**：带工具的业务可声明系统提示（`AiToolProvider.context_prefix`）。会话**首轮发送时**若消息为空，提示作为会话首条消息落盘（`role=system`，在 `chats/{chat_id}.json` 与消息分页接口中均可见），此后作为历史原样重放给模型，**永不更新**——书籍改名等后续变化不回写旧会话，新会话取新值；业务不可用（未配置电子书目录/书籍不存在）时该会话不注入。前端须过滤 `role=system` 不予展示。电子书提示内容：`当前对话上下文：用户正在阅读的书籍是《…》（… 格式）。`

**上下文自动压缩（pi 同款逻辑）**：每次发送构建上游请求前，用 `context_tokens`（上次上游 usage）+ 新增消息估算与模型 `context_length` 判定：`估算 > context_length - 16384`（reserve）时触发压缩。压缩流程：按 pi 的切点算法从最新消息向前累计，保留约 `20000` token 的近期消息（永不切断工具调用与结果的对），将其余历史序列化为文本（`[User]:` / `[Assistant]:` / `[Tool result]:`（截断 2000 字符）格式，工具结果），包在 `<conversation>` 标签中，以 pi 的结构化摘要系统提示（Goal/Constraints/Progress/Key Decisions/Next Steps/Critical Context）调用**同一模型**生成摘要；若此前已有压缩摘要，则以 `<previous-summary>` 标签合并更新。成功后旧历史被替换为一条 `role=compact` 摘要消息落盘（含清理不再引用的图片文件），`context_tokens` 清零待本轮重新计量；摘要作为用户消息（`<summary>` 包装）注入后续上下文。压缩失败不阻断对话（记录 error.log）。模型 `context_length` 为 0 时禁用自动压缩。

**响应**：`Content-Type: text/event-stream`，事件序列：

| 事件 | data | 说明 |
|------|------|------|
| user_message | 消息对象（同分页接口元素，images 为落盘后的相对路径） | 用户消息已保存 |
| compact_start | `{ "tokens_before": 估算token数 }` | 上下文压缩开始 |
| compact_end | `{ "ok": true/false, "tokens_before": …（ok 时） }` | 压缩结束（失败不阻断对话） |
| reasoning | `{ "content": "增量思维链文本" }` | 深度思考增量（仅请求传入 thinking 时） |
| delta | `{ "content": "增量文本" }` | 助手回复增量（markdown 片段） |
| tool_start | `{ "name": "工具名", "arguments": "JSON参数" }` | 开始执行工具 |
| tool_end | `{ "name": "工具名", "ok": true/false }` | 工具执行结束 |
| page_query | `{ "request_id": "uuid", "kind": "查询类型", "params": {…} }` | 工具向页面查询数据，页面按 kind 应答后调用接口 15 回传，20s 超时 |
| done | `{ "message_id": "uuid" }` | 助手消息已落盘，流结束 |
| error | `{ "fail_code": "…", "detail": "上游原始报错（可选）" }` | 出错终止，已生成内容已落盘 |

**工具产出图片**：工具可返回截图（如 `view_pdf_page`），落盘到会话图片目录并挂在 `tool` 消息的 `images` 字段上；模型支持视觉（`supports_vision`）时，图片以视觉消息注入本轮上下文，历史多轮重放时同样还原。

**错误编码**（事件 `error` 或直接 JSON 响应）：`NOT_LOGGED_IN`、`PARAM_INVALID`、`AI_CHAT_PATH_NOT_SET`、`CHAT_NOT_FOUND`、`MODEL_NOT_FOUND`、`PROVIDER_TYPE_INVALID`、`PROXY_INVALID`、`IMAGE_INVALID`、`IMAGE_TOO_LARGE`、`AI_CALL_FAILED`、`AI_TIMEOUT`、`TOOL_ROUNDS_EXCEEDED`、`TOOL_NOT_AVAILABLE`

**上游原始报错**：上游返回失败响应时（如 401 密钥无效、429 限流、5xx 服务异常），`error` 事件在 `fail_code` 之外附带 `detail`，内容为「HTTP 状态码 + 原因 + 上游返回的错误消息」（如 `HTTP 401 Unauthorized: Invalid token`，最长 300 字符），供界面直接展示，便于用户分辨失败原因。仅当拿不到上游响应（本地超时、连接失败）时 `detail` 缺省。请求内容不会出现在 `detail` 中。

## 14. 读取消息图片

**路径**：GET /api/ai/chat/image?biz_type={功能名}&path={relative}

**功能**：读取当前用户会话目录下的图片文件（唯一允许 GET 的例外，供 `<img>` 直接引用；媒体文件同静态资源）。`path` 为消息中的相对路径（如 `images/xxx.png`），服务端校验目录越界与扩展名白名单（png/jpg/jpeg/webp/gif）。

**成功响应**：图片二进制，`Content-Type` 按扩展名。

**错误编码**：`NOT_LOGGED_IN`、`AI_CHAT_PATH_NOT_SET`、`PATH_INVALID`

## 15. 回传页面查询应答

**路径**：POST /api/ai/chat/page_query_result

**功能**：**通用的页面数据查询回传接口**。AI 工具在执行过程中可主动向用户页面查询数据（SSE `page_query` 事件，携带 `kind` 与 `params`），页面按 `kind` 自行决定如何应答，并以任意字符串 `data` 回传（格式由各 kind 自行定义）。仅在流式请求进行中有效，等待方不存在时返回 `PAGE_QUERY_NOT_FOUND`。例如：电子书 `pdf_page` 回传 `JSON {"page": 页码, "image": "data:image/jpeg;base64,…"}`（页码缺省时页面回传其当前阅读页）；未来密码库/云笔记的端到端加密场景可定义各自的 kind（如解密条目、读取笔记内容），由页面持有密钥完成应答。

**请求参数**：
```json
{ "request_id": "uuid", "data": "应答内容（格式由 kind 定义，空串表示应答失败）" }
```

**成功响应**：`{ "success": true }`

**错误编码**：`NOT_LOGGED_IN`、`PARAM_INVALID`、`PAGE_QUERY_NOT_FOUND`

**已定义 kind**：

| kind | params | data 应答格式 | 业务 |
|------|--------|--------------|------|
| pdf_page | `{ "biz_id": "书籍id", "page": 页码（缺省=当前阅读页） }` | `{"page": 实际渲染页码, "image": "data:image/jpeg;base64,…"}` | 电子书 |

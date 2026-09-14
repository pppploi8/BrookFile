# AI模块数据表

AI 配置为系统级全局配置（全功能公用，第一版先服务于电子书 AI 问答），存储多供应商与模型（上游调用基于 genai 适配器）。全局共享一份，任意登录用户均可管理，与账户无关。

**会话与消息不存数据库**：AI 对话（会话、消息、消息图片）以 JSON 文件形式存储在每个用户自选的 `ai_chat_path` 目录下（`users.ai_chat_path`，相对用户 root_path）。用户私人目录本身已隔离，存储层按功能名分目录：`{ai_chat_path}/{biz_type}/`。文件布局与格式见 `api/ai.md`；数据库损坏后可直接打开该目录恢复会话。

## AI供应商表 (ai_providers)

| 字段名 | 数据类型 | 约束 | 描述 |
|-------|---------|------|------|
| id | TEXT | PRIMARY KEY | 供应商ID，UUID格式 |
| name | TEXT | NOT NULL | 供应商名称（用户自定义） |
| provider_type | TEXT | NOT NULL DEFAULT 'openai_responses' | 供应商类型（genai 适配器预设 type_id），见 api/ai.md 接口 1 |
| base_url | TEXT | NOT NULL | 供应商根地址（如 `https://api.openai.com/v1`），适配器在其后拼接协议路径 |
| api_key | TEXT | NOT NULL | 访问密钥 |
| proxy | TEXT | NOT NULL DEFAULT '' | 代理地址，仅支持 HTTP 代理，空串表示直连 |
| created_at | TIMESTAMP | DEFAULT CURRENT_TIMESTAMP | 创建时间 |
| updated_at | TIMESTAMP | DEFAULT CURRENT_TIMESTAMP | 更新时间 |

### AI供应商表说明

- `provider_type` 对应后端预设表（`ai/provider.rs`）：`openai_completions`、`openai_responses`、`anthropic`、`deepseek`、`openrouter`、`groq`、`xai`、`moonshot`、`kimi`、`zai`、`fireworks`、`together`、`nebius`、`mimo`、`gemini`、`ollama`、`ollama_cloud`、`cohere`；适配器决定上游协议与消息/思考参数映射。接入非预设厂商时用 `openai_completions` 覆盖 `base_url` 即可
- `base_url` 为根地址：以 `http://` 或 `https://` 开头，存储时去除末尾 `/`；空串表示使用 `provider_type` 预设的默认地址（调用时兜底）。旧版数据（完整 `/responses` 地址）在调用时自动剥掉后缀兼容
- `proxy` 仅供 HTTP 代理（`http://` 开头的地址），访问境外平台时需配置；空串表示直连
- 删除供应商时级联删除其下所有模型（`ai_models.provider_id ON DELETE CASCADE`）
- `api_key` 明文存储，不在列表接口中回传，仅返回 `has_api_key` 标记；更新时留空表示保留原密钥；`provider_type=ollama` 等免密钥预设允许空串
- `proxy` 在列表接口中返回，供前端编辑时回显；创建/更新时按请求值覆盖，可传空串清除代理

## AI模型表 (ai_models)

| 字段名 | 数据类型 | 约束 | 描述 |
|-------|---------|------|------|
| id | TEXT | PRIMARY KEY | 模型配置ID，UUID格式 |
| provider_id | TEXT | NOT NULL | 所属供应商ID，关联 ai_providers.id，级联删除 |
| model_id | TEXT | NOT NULL | 模型标识，如 `gpt-4o` |
| supports_vision | INTEGER | NOT NULL DEFAULT 0 | 是否支持多模态（0=不支持，1=支持） |
| context_length | INTEGER | NOT NULL DEFAULT 0 | 上下文长度（token），0 表示未知/未配置 |
| max_output_tokens | INTEGER | NOT NULL DEFAULT 0 | 最大输出长度（token），0 表示未知/未配置 |
| created_at | TIMESTAMP | DEFAULT CURRENT_TIMESTAMP | 创建时间 |
| updated_at | TIMESTAMP | DEFAULT CURRENT_TIMESTAMP | 更新时间 |

### 索引

| 索引名 | 字段 | 说明 |
|-------|------|------|
| idx_ai_models_provider_id | provider_id | 按供应商查询模型 |
| idx_ai_models_provider_model | provider_id, model_id (UNIQUE) | 同一供应商下不允许重复模型标识 |

### AI模型表说明

- `model_id` 同一供应商下不允许重复（唯一索引保证）
- `context_length` / `max_output_tokens` 为配置项，默认 0 表示未知，由前端引导用户填写；`context_length > 0` 时启用会话上下文自动压缩（阈值 = context_length - 16384，见 api/ai.md 接口 13），`max_output_tokens` 同时作为压缩摘要生成的输出上限之一
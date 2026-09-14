//! 上下文自动压缩：逻辑与提示词移植自 pi coding agent（pi-mono compaction 模块）。
//! 阈值判定（usage + 估算）、切点选择（保留近期消息、不切断工具调用对）、
//! 序列化格式、摘要提示词均与上游保持一致；文件操作追踪为编码场景专属，未移植。

use super::client::{complete_chat, AiCallConfig};
use super::store::StoredMessage;
use genai::Client;

pub const RESERVE_TOKENS: i64 = 16384;
pub const KEEP_RECENT_TOKENS: i64 = 20000;

const ESTIMATED_IMAGE_CHARS: i64 = 4800;
const TOOL_RESULT_MAX_CHARS: usize = 2000;

const SUMMARIZATION_SYSTEM_PROMPT: &str = "You are a context summarization assistant. Your task is to read a conversation between a user and an AI assistant, then produce a structured summary following the exact format specified.\n\nDo NOT continue the conversation. Do NOT respond to any questions in the conversation. ONLY output the structured summary.";

const SUMMARIZATION_PROMPT: &str = "The messages above are a conversation to summarize. Create a structured context checkpoint summary that another LLM will use to continue the work.\n\nUse this EXACT format:\n\n## Goal\n[What is the user trying to accomplish? Can be multiple items if the session covers different tasks.]\n\n## Constraints & Preferences\n- [Any constraints, preferences, or requirements mentioned by user]\n- [Or \"(none)\" if none were mentioned]\n\n## Progress\n### Done\n- [x] [Completed tasks/changes]\n\n### In Progress\n- [ ] [Current work]\n\n### Blocked\n- [Issues preventing progress, if any]\n\n## Key Decisions\n- **[Decision]**: [Brief rationale]\n\n## Next Steps\n1. [Ordered list of what should happen next]\n\n## Critical Context\n- [Any data, examples, or references needed to continue]\n- [Or \"(none)\" if not applicable]\n\nKeep each section concise. Preserve exact file paths, function names, and error messages.";

const UPDATE_SUMMARIZATION_PROMPT: &str = "The messages above are NEW conversation messages to incorporate into the existing summary provided in <previous-summary> tags.\n\nUpdate the existing structured summary with new information. RULES:\n- PRESERVE all existing information from the previous summary\n- ADD new progress, decisions, and context from the new messages\n- UPDATE the Progress section: move items from \"In Progress\" to \"Done\" when completed\n- UPDATE \"Next Steps\" based on what was accomplished\n- PRESERVE exact file paths, function names, and error messages\n- If something is no longer relevant, you may remove it\n\nUse this EXACT format:\n\n## Goal\n[Preserve existing goals, add new ones if the task expanded]\n\n## Constraints & Preferences\n- [Preserve existing, add new ones discovered]\n\n## Progress\n### Done\n- [x] [Include previously done items AND newly completed items]\n\n### In Progress\n- [ ] [Current work - update based on progress]\n\n### Blocked\n- [Current blockers - remove if resolved]\n\n## Key Decisions\n- **[Decision]**: [Brief rationale] (preserve all previous, add new)\n\n## Next Steps\n1. [Update based on current state]\n\n## Critical Context\n- [Preserve important context, add new if needed]\n\nKeep each section concise. Preserve exact file paths, function names, and error messages.";

const TURN_PREFIX_SUMMARIZATION_PROMPT: &str = "This is the PREFIX of a turn that was too large to keep. The SUFFIX (recent work) is retained.\n\nSummarize the prefix to provide context for the retained suffix:\n\n## Original Request\n[What did the user ask for in this turn?]\n\n## Early Progress\n- [Key decisions and work done in the prefix]\n\n## Context for Suffix\n- [Information needed to understand the retained recent work]\n\nBe concise. Focus on what's needed to understand the kept suffix.";

// ============================================================================
// Token 估算与触发判定
// ============================================================================

/// pi 同款 chars/4 启发式：图片按 4800 字符计
pub fn estimate_tokens(m: &StoredMessage) -> i64 {
    let mut chars = m.content.chars().count() as i64;
    match m.role.as_str() {
        "assistant" => {
            if let Some(r) = &m.reasoning {
                chars += r.chars().count() as i64;
            }
            if let Some(tc) = &m.tool_calls {
                if let Ok(text) = serde_json::to_string(tc) {
                    chars += text.chars().count() as i64;
                }
            }
        }
        _ => {
            chars += m.images.len() as i64 * ESTIMATED_IMAGE_CHARS;
        }
    }
    (chars + 3) / 4
}

/// pi 的 estimateContextTokens：优先使用上次上游返回的 usage（meta.context_tokens，
/// 覆盖截至上次发送结束的全部上下文），加上其后的尾部消息估算；无 usage 时全量估算
pub fn context_token_estimate(meta_tokens: Option<i64>, messages: &[StoredMessage]) -> i64 {
    match meta_tokens {
        Some(t) => {
            let trailing = messages.last().map(estimate_tokens).unwrap_or(0);
            t + trailing
        }
        None => messages.iter().map(estimate_tokens).sum(),
    }
}

pub fn should_compact(context_tokens: i64, context_window: i64) -> bool {
    context_window > 0 && context_tokens > context_window - RESERVE_TOKENS
}

// ============================================================================
// 切点选择
// ============================================================================

fn is_cut_point(role: &str) -> bool {
    matches!(role, "user" | "assistant" | "compact")
}

fn is_turn_start(role: &str) -> bool {
    matches!(role, "user" | "compact")
}

pub struct CutPoint {
    pub first_kept: usize,
    pub turn_start: usize,
    pub is_split_turn: bool,
}

/// pi 的 findCutPoint：从最新消息向前累计估算 token，达到 keepRecentTokens 后在
/// 最近的有效切点处切分；永不切在工具结果上（工具结果必须跟随其调用）
pub fn find_cut_point(messages: &[StoredMessage], start_index: usize) -> CutPoint {
    let cut_points: Vec<usize> = (start_index..messages.len())
        .filter(|i| is_cut_point(messages[*i].role.as_str()))
        .collect();
    if cut_points.is_empty() {
        return CutPoint {
            first_kept: start_index,
            turn_start: usize::MAX,
            is_split_turn: false,
        };
    }

    let mut accumulated: i64 = 0;
    let mut cut_index = cut_points[0];
    for i in (start_index..messages.len()).rev() {
        let tokens = estimate_tokens(&messages[i]);
        if tokens == 0 {
            continue;
        }
        accumulated += tokens;
        if accumulated >= KEEP_RECENT_TOKENS {
            if let Some(cp) = cut_points.iter().find(|c| **c >= i) {
                cut_index = *cp;
            }
            break;
        }
    }

    let starts_turn = is_turn_start(messages[cut_index].role.as_str());
    let mut turn_start = usize::MAX;
    if !starts_turn {
        for i in (start_index..cut_index).rev() {
            if is_turn_start(messages[i].role.as_str()) {
                turn_start = i;
                break;
            }
        }
    }
    CutPoint {
        first_kept: cut_index,
        turn_start,
        is_split_turn: !starts_turn && turn_start != usize::MAX,
    }
}

// ============================================================================
// 会话序列化（pi serializeConversation 格式）
// ============================================================================

fn truncate_for_summary(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    let truncated = count - max_chars;
    let head: String = text.chars().take(max_chars).collect();
    format!("{head}\n\n[... {truncated} more characters truncated]")
}

fn tool_calls_text(m: &StoredMessage) -> Option<String> {
    let calls = m.tool_calls.as_ref()?.as_array()?;
    let parts: Vec<String> = calls
        .iter()
        .filter_map(|call| {
            let name = call["function"]["name"].as_str()?;
            let args = call["function"]["arguments"].as_str().unwrap_or("{}");
            let value: serde_json::Value = serde_json::from_str(args).unwrap_or(serde_json::Value::Null);
            let args_str = match &value {
                serde_json::Value::Object(map) => map
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, serde_json::to_string(v).unwrap_or_default()))
                    .collect::<Vec<_>>()
                    .join(", "),
                other => serde_json::to_string(other).unwrap_or_default(),
            };
            Some(format!("{name}({args_str})"))
        })
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("; "))
    }
}

/// 将消息序列化为文本，防止模型把摘要请求当作对话续写
fn serialize_conversation(messages: &[StoredMessage]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for m in messages {
        match m.role.as_str() {
            "user" => {
                if !m.content.is_empty() {
                    parts.push(format!("[User]: {}", m.content));
                }
            }
            "compact" | "system" => {}
            "assistant" => {
                if let Some(reasoning) = &m.reasoning {
                    if !reasoning.is_empty() {
                        parts.push(format!("[Assistant thinking]: {reasoning}"));
                    }
                }
                if !m.content.is_empty() {
                    parts.push(format!("[Assistant]: {}", m.content));
                }
                if let Some(calls) = tool_calls_text(m) {
                    parts.push(format!("[Assistant tool calls]: {calls}"));
                }
            }
            "tool" => {
                if !m.content.is_empty() {
                    parts.push(format!(
                        "[Tool result]: {}",
                        truncate_for_summary(&m.content, TOOL_RESULT_MAX_CHARS)
                    ));
                }
            }
            _ => {}
        }
    }
    parts.join("\n\n")
}

// ============================================================================
// 摘要生成
// ============================================================================

pub struct Preparation {
    pub first_kept: usize,
    pub is_split_turn: bool,
    pub previous_summary: Option<String>,
    /// 待摘要的历史消息（不含旧 compact 消息与系统提示）
    pub history: Vec<StoredMessage>,
    /// 分裂轮次时被摘要的轮次前缀
    pub turn_prefix: Vec<StoredMessage>,
}

/// pi 的 prepareCompaction：返回 None 表示无需摘要（切点未命中可摘要内容）
pub fn prepare(messages: &[StoredMessage]) -> Option<Preparation> {
    let start_index = if messages.first().map(|m| m.role.as_str()) == Some("system") {
        1
    } else {
        0
    };
    let cut = find_cut_point(messages, start_index);
    let history_end = if cut.is_split_turn {
        cut.turn_start
    } else {
        cut.first_kept
    };
    if history_end <= start_index && !cut.is_split_turn {
        return None;
    }

    let previous_summary = messages
        .get(start_index..history_end)
        .unwrap_or(&[])
        .iter()
        .rev()
        .find(|m| m.role == "compact")
        .map(|m| m.content.clone());

    let history: Vec<StoredMessage> = messages[start_index..history_end]
        .iter()
        .filter(|m| m.role != "compact")
        .cloned()
        .collect();
    let turn_prefix: Vec<StoredMessage> = if cut.is_split_turn {
        messages[cut.turn_start..cut.first_kept]
            .iter()
            .filter(|m| m.role != "compact")
            .cloned()
            .collect()
    } else {
        Vec::new()
    };

    if history.is_empty() && turn_prefix.is_empty() {
        return None;
    }

    Some(Preparation {
        first_kept: cut.first_kept,
        is_split_turn: cut.is_split_turn,
        previous_summary,
        history,
        turn_prefix,
    })
}

async fn summarize_once(
    client: &Client,
    config: &AiCallConfig,
    history_text: String,
    prompt: &str,
    previous: Option<&str>,
    max_tokens: u32,
) -> Result<String, String> {
    let mut prompt_text = format!("<conversation>\n{history_text}\n</conversation>\n\n");
    if let Some(prev) = previous {
        prompt_text.push_str(&format!("<previous-summary>\n{prev}\n</previous-summary>\n\n"));
    }
    prompt_text.push_str(prompt);
    let req = genai::chat::ChatRequest::new(vec![genai::chat::ChatMessage::user(prompt_text)])
        .with_system(SUMMARIZATION_SYSTEM_PROMPT);
    let result = complete_chat(client, config, req, Some(max_tokens)).await?;
    if result.max_tokens_hit {
        return Err("摘要生成达到 token 上限，内容不完整".to_string());
    }
    if result.text.trim().is_empty() {
        return Err("摘要生成为空".to_string());
    }
    Ok(result.text)
}

/// 生成压缩摘要（分裂轮次时两次调用并按 pi 格式合并）；返回 Ok(None) 表示无需压缩
pub async fn run_compaction(
    client: &Client,
    config: &AiCallConfig,
    max_output_tokens: i64,
    messages: &[StoredMessage],
) -> Result<Option<(String, Preparation)>, String> {
    let Some(prep) = prepare(messages) else {
        return Ok(None);
    };

    let mut max_tokens = (RESERVE_TOKENS * 8 / 10).min(i64::from(u32::MAX));
    if max_output_tokens > 0 {
        max_tokens = max_tokens.min(max_output_tokens);
    }
    let max_tokens = max_tokens as u32;

    let history_text = serialize_conversation(&prep.history);
    let base_prompt = if prep.previous_summary.is_some() {
        UPDATE_SUMMARIZATION_PROMPT
    } else {
        SUMMARIZATION_PROMPT
    };
    let summary = if prep.is_split_turn && !prep.turn_prefix.is_empty() {
        let history_text_out = if prep.history.is_empty() {
            "No prior history.".to_string()
        } else {
            summarize_once(
                client,
                config,
                history_text,
                base_prompt,
                prep.previous_summary.as_deref(),
                max_tokens,
            )
            .await?
        };
        let prefix_max = (RESERVE_TOKENS / 2).min(i64::from(u32::MAX)) as u32;
        let prefix_text = summarize_once(
            client,
            config,
            serialize_conversation(&prep.turn_prefix),
            TURN_PREFIX_SUMMARIZATION_PROMPT,
            None,
            prefix_max,
        )
        .await?;
        format!("{history_text_out}\n\n---\n\n**Turn Context (split turn):**\n\n{prefix_text}")
    } else {
        summarize_once(
            client,
            config,
            history_text,
            base_prompt,
            prep.previous_summary.as_deref(),
            max_tokens,
        )
        .await?
    };

    Ok(Some((summary, prep)))
}

// ============================================================================
// 应用压缩结果
// ============================================================================

pub struct ApplyResult {
    pub messages: Vec<StoredMessage>,
    /// 压缩后不再被引用、可删除的图片相对路径
    pub orphan_images: Vec<String>,
}

/// 用摘要消息替换旧历史：保留系统提示与切点之后的消息，返回新消息列表与孤儿图片
pub fn apply(
    messages: Vec<StoredMessage>,
    prep: &Preparation,
    summary: String,
    compact_id: String,
    now: &str,
) -> ApplyResult {
    let start_index = if messages.first().map(|m| m.role.as_str()) == Some("system") {
        1
    } else {
        0
    };
    let mut kept_refs: Vec<String> = Vec::new();
    let mut new_messages: Vec<StoredMessage> = Vec::new();
    if start_index == 1 {
        let sys = messages[0].clone();
        kept_refs.extend(sys.images.iter().cloned());
        new_messages.push(sys);
    }
    new_messages.push(StoredMessage {
        id: compact_id,
        role: "compact".to_string(),
        content: summary,
        reasoning: None,
        images: Vec::new(),
        tool_calls: None,
        tool_call_id: None,
        name: None,
        model_key: None,
        created_at: now.to_string(),
    });
    for m in &messages[prep.first_kept..] {
        kept_refs.extend(m.images.iter().cloned());
    }
    let mut orphan_images = Vec::new();
    for m in &messages[start_index..prep.first_kept] {
        for rel in &m.images {
            if !kept_refs.contains(rel) {
                orphan_images.push(rel.clone());
            }
        }
    }
    new_messages.extend(messages[prep.first_kept..].iter().cloned());
    ApplyResult {
        messages: new_messages,
        orphan_images,
    }
}

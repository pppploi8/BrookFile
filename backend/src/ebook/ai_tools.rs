use std::path::Path;

use async_trait::async_trait;

use crate::ai::{AiToolContext, AiToolProvider, ToolDef, ToolResult};
use crate::database::Pool;
use crate::ebook::ebook_db::{ebook_meta_dir, EbookDb};
use crate::ebook::{epub_parser, txt_parser};
use crate::models::UserModel;

const MAX_CHAPTER_CONTENT_CHARS: usize = 8000;
const MAX_SEARCH_HITS: usize = 20;

pub struct EbookToolProvider {
    user_model: UserModel,
}

impl EbookToolProvider {
    pub fn new(pool: &Pool) -> Self {
        EbookToolProvider {
            user_model: UserModel::new(pool),
        }
    }

    fn resolve_book(
        &self,
        ctx: &AiToolContext,
    ) -> Result<(std::path::PathBuf, String, String), String> {
        let ebook_path = self
            .user_model
            .get_user_full(&ctx.user_id)
            .map_err(|e| e.to_string())?
            .and_then(|u| u.ebook_path)
            .filter(|p| !p.is_empty())
            .ok_or("EBOOK_NOT_CONFIGURED")?;

        let db = EbookDb::open(Path::new(&ctx.root_path), &ebook_path)?;
        let book = db
            .get_book(&ctx.biz_id)
            .map_err(|e| e.to_string())?
            .ok_or("BOOK_NOT_FOUND")?;

        let source = Path::new(&ctx.root_path)
            .join(&ebook_path)
            .join(&book.source_path);
        Ok((source, book.format, ebook_path))
    }

    fn txt_chapter_texts(
        &self,
        source: &Path,
        ctx: &AiToolContext,
    ) -> Result<Vec<(String, String)>, String> {
        let ebook_path = self
            .user_model
            .get_user_full(&ctx.user_id)
            .map_err(|e| e.to_string())?
            .and_then(|u| u.ebook_path)
            .filter(|p| !p.is_empty())
            .ok_or("EBOOK_NOT_CONFIGURED")?;

        let cache_path = ebook_meta_dir(Path::new(&ctx.root_path), &ebook_path)
            .join("cache")
            .join(format!("{}.txt", ctx.biz_id));
        if !cache_path.exists() {
            let encoding = txt_parser::detect_encoding(source)?;
            txt_parser::transcode_to_utf8(source, &encoding, &cache_path)?;
        }
        let content = std::fs::read_to_string(&cache_path).map_err(|e| e.to_string())?;
        let chapters = txt_parser::split_chapters(&content);
        // split_chapters 的偏移是 UTF-16 码元（与前端对齐），切片前需转换为字节偏移
        let targets: Vec<usize> = chapters.iter().map(|(_, off)| *off).collect();
        let byte_offsets = utf16_offsets_to_bytes(&content, targets);
        let mut texts = Vec::with_capacity(chapters.len());
        for (i, (title, _)) in chapters.iter().enumerate() {
            let start = byte_offsets[i];
            let end = byte_offsets.get(i + 1).copied().unwrap_or(content.len());
            texts.push((title.clone().unwrap_or_default(), content[start..end].to_string()));
        }
        Ok(texts)
    }

    fn epub_chapter_texts(source: &Path) -> Result<Vec<(String, String)>, String> {
        let parsed = epub_parser::parse_epub(source)?;
        Ok(parsed
            .chapters
            .into_iter()
            .map(|(title, html)| (title.unwrap_or_default(), strip_html(&html)))
            .collect())
    }

    fn chapter_texts(
        &self,
        source: &Path,
        format: &str,
        ctx: &AiToolContext,
    ) -> Result<Vec<(String, String)>, String> {
        match format {
            "txt" => self.txt_chapter_texts(source, ctx),
            "epub" => Self::epub_chapter_texts(source),
            _ => Err("该格式不支持文本读取，请使用截图引用".to_string()),
        }
    }
}

fn utf16_offsets_to_bytes(content: &str, targets: Vec<usize>) -> Vec<usize> {
    let mut result = vec![content.len(); targets.len()];
    let mut ti = 0usize;
    let mut u16_count = 0usize;
    for (byte_idx, ch) in content.char_indices() {
        while ti < targets.len() && u16_count >= targets[ti] {
            result[ti] = byte_idx;
            ti += 1;
        }
        if ti >= targets.len() {
            break;
        }
        u16_count += ch.len_utf16();
    }
    result
}

fn strip_html(html: &str) -> String {    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => {
                in_tag = true;
                out.push(' ');
            }
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let text = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[async_trait]
impl AiToolProvider for EbookToolProvider {
    fn biz_type(&self) -> &str {
        "ebook"
    }

    fn max_tool_rounds(&self) -> u32 {
        10
    }

    // 告知模型当前阅读书籍的标题与格式，省得它从章节标题猜
    fn context_prefix(&self, ctx: &AiToolContext) -> Option<String> {
        let ebook_path = self
            .user_model
            .get_user_full(&ctx.user_id)
            .ok()?
            .and_then(|u| u.ebook_path)
            .filter(|p| !p.is_empty())?;
        let db = EbookDb::open(Path::new(&ctx.root_path), &ebook_path).ok()?;
        let book = db.get_book(&ctx.biz_id).ok()??;
        Some(format!(
            "当前对话上下文：用户正在阅读的书籍是《{}》（{} 格式）。",
            book.title, book.format
        ))
    }

    fn tool_definitions(&self) -> Vec<ToolDef> {
        vec![            ToolDef {
                name: "list_chapters".to_string(),
                description: "列出当前书籍的章节目录（章节序号从 1 开始）".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            ToolDef {
                name: "get_chapter_content".to_string(),
                description: "读取指定序号章节的正文内容（超长时截断）".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "chapter_no": {
                            "type": "integer",
                            "description": "章节序号，从 1 开始"
                        }
                    },
                    "required": ["chapter_no"]
                }),
            },
            ToolDef {
                name: "search_content".to_string(),
                description: "在书籍全文中搜索关键词，返回命中的章节与上下文片段".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "keyword": {
                            "type": "string",
                            "description": "要搜索的关键词"
                        }
                    },
                    "required": ["keyword"]
                }),
            },
            ToolDef {
                name: "get_reading_position".to_string(),
                description: "获取用户当前阅读位置。返回进度槽记录（按最近保存时间排序，第一条即当前位置）：content_coord 为内容坐标（txt: 章节序号:字符偏移；epub: CFI；pdf: 页码:滚动比例），summary 为可读的位置摘要".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            ToolDef {
                name: "view_pdf_page".to_string(),
                description: "查看 PDF 指定页码的页面截图（仅 PDF 支持视觉查看）。page 省略时渲染用户当前正在阅读的页".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "page": {
                            "type": "integer",
                            "description": "页码，从 1 开始；省略表示当前阅读页"
                        }
                    },
                    "required": []
                }),
            },
        ]
    }

    async fn execute(
        &self,
        ctx: &AiToolContext,
        name: &str,
        arguments: &str,
    ) -> Result<ToolResult, String> {
        let args: serde_json::Value =
            serde_json::from_str(arguments).map_err(|_| "工具参数不是合法 JSON".to_string())?;

        match name {
            "list_chapters" => {
                let (source, format, _) = self.resolve_book(ctx)?;
                let texts = self.chapter_texts(&source, &format, ctx)?;
                let mut lines = Vec::with_capacity(texts.len());
                for (i, (title, content)) in texts.iter().enumerate() {
                    let title = if title.is_empty() {
                        content.chars().take(20).collect::<String>()
                    } else {
                        title.clone()
                    };
                    lines.push(format!("{}. {}", i + 1, title));
                }
                Ok(ToolResult::text(if lines.is_empty() {
                    "该书没有识别到章节".to_string()
                } else {
                    lines.join("\n")
                }))
            }
            "get_chapter_content" => {
                let chapter_no = args
                    .get("chapter_no")
                    .and_then(|v| v.as_i64())
                    .ok_or("缺少 chapter_no 参数")?;
                if chapter_no < 1 {
                    return Err("chapter_no 必须从 1 开始".to_string());
                }
                let (source, format, _) = self.resolve_book(ctx)?;
                let texts = self.chapter_texts(&source, &format, ctx)?;
                let text = texts
                    .get((chapter_no - 1) as usize)
                    .map(|(_, content)| content.clone())
                    .ok_or("章节序号超出范围")?;
                let mut result = text;
                if result.chars().count() > MAX_CHAPTER_CONTENT_CHARS {
                    result = result.chars().take(MAX_CHAPTER_CONTENT_CHARS).collect();
                    result.push_str("\n…（内容过长已截断）");
                }
                Ok(ToolResult::text(result))
            }
            "search_content" => {
                let keyword = args
                    .get("keyword")
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .ok_or("缺少 keyword 参数")?;
                let (source, format, _) = self.resolve_book(ctx)?;
                let texts = self.chapter_texts(&source, &format, ctx)?;
                let mut hits: Vec<String> = Vec::new();
                for (i, (title, content)) in texts.iter().enumerate() {
                    let mut from = 0;
                    while let Some(pos) = content[from..].find(keyword) {
                        let abs = from + pos;
                        let start = content
                            .char_indices()
                            .map(|(b, _)| b)
                            .take_while(|b| *b <= abs)
                            .count();
                        let snippet: String = content
                            .chars()
                            .skip(start.saturating_sub(30))
                            .take(80)
                            .collect();
                        let title = if title.is_empty() {
                            format!("第 {} 章", i + 1)
                        } else {
                            title.clone()
                        };
                        hits.push(format!(
                            "[第 {} 章 {}] …{}…",
                            i + 1,
                            title,
                            snippet.replace('\n', " ")
                        ));
                        if hits.len() >= MAX_SEARCH_HITS {
                            break;
                        }
                        from = abs + keyword.len();
                    }
                    if hits.len() >= MAX_SEARCH_HITS {
                        hits.push("…（命中过多，已截断）".to_string());
                        break;
                    }
                }
                Ok(ToolResult::text(if hits.is_empty() {
                    format!("未找到关键词「{}」", keyword)
                } else {
                    hits.join("\n")
                }))
            }
            "get_reading_position" => {
                let ebook_path = self
                    .user_model
                    .get_user_full(&ctx.user_id)
                    .map_err(|e| e.to_string())?
                    .and_then(|u| u.ebook_path)
                    .filter(|p| !p.is_empty())
                    .ok_or("EBOOK_NOT_CONFIGURED")?;
                let db = EbookDb::open(Path::new(&ctx.root_path), &ebook_path)?;
                let slots = db.get_progress(&ctx.biz_id)?;
                let slots: Vec<serde_json::Value> = slots
                    .iter()
                    .map(|s| {
                        serde_json::json!({
                            "slot": s.slot,
                            "content_coord": s.content_coord,
                            "summary": s.summary,
                            "updated_at": s.updated_at,
                        })
                    })
                    .collect();
                Ok(ToolResult::text(if slots.is_empty() {
                    "没有阅读进度记录，用户尚未开始阅读或未保存过进度".to_string()
                } else {
                    serde_json::to_string(&serde_json::json!({ "progress_slots": slots }))
                        .map_err(|e| e.to_string())?
                }))
            }
            "view_pdf_page" => {
                let (_, format, _) = self.resolve_book(ctx)?;
                if format != "pdf" {
                    return Err("该格式不支持页面截图，仅 PDF 支持".to_string());
                }
                let bridge = ctx
                    .page_query
                    .as_ref()
                    .ok_or("当前界面不支持页面查询")?;
                let mut params = serde_json::json!({ "biz_id": ctx.biz_id });
                if let Some(p) = args.get("page").and_then(|v| v.as_i64()) {
                    if p < 1 {
                        return Err("page 必须为正整数".to_string());
                    }
                    params["page"] = serde_json::json!(p);
                }
                // page 缺省时由页面回传其当前阅读页；应答格式为 JSON {page, image}
                let raw = bridge.query("pdf_page", params).await?;
                let reply: serde_json::Value = serde_json::from_str(&raw)
                    .map_err(|_| "页面回传数据格式错误".to_string())?;
                let page = reply["page"].as_i64().ok_or("页面回传缺少页码")?;
                let image = reply["image"].as_str().ok_or("页面回传缺少截图")?;
                let dir = ctx
                    .chat_store_dir
                    .as_ref()
                    .ok_or("会话存储不可用")?;
                let rel = crate::ai::store::save_image(dir, &ctx.biz_type, &ctx.chat_id, image)?;
                Ok(ToolResult {
                    content: format!("已渲染第 {} 页截图", page),
                    images: vec![rel],
                })
            }
            _ => Err(format!("未知工具: {}", name)),
        }
    }
}

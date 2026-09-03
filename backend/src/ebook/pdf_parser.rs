use std::io::Read;
use std::path::Path;

pub struct PdfParseResult {
    pub title: String,
    pub chapters: Vec<(Option<String>, String)>,
}

pub fn parse_pdf(file_path: &Path) -> Result<PdfParseResult, String> {
    let mut file = std::fs::File::open(file_path).map_err(|e| e.to_string())?;
    let mut content = Vec::new();
    file.read_to_end(&mut content).map_err(|e| e.to_string())?;

    let content_str = String::from_utf8_lossy(&content);
    let page_count = count_pdf_pages(&content_str);

    let title = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();

    let chapters = if page_count > 0 {
        let group_size = 10;
        let mut chs = Vec::new();
        let mut page = 0;
        while page < page_count {
            let end = std::cmp::min(page + group_size, page_count);
            let title = if page == 0 && end == page_count {
                format!("第 1-{} 页", page_count)
            } else {
                format!("第 {}-{} 页", page + 1, end)
            };
            chs.push((Some(title), format!("{}", page)));
            page = end;
        }
        chs
    } else {
        vec![(Some("全文".to_string()), "0".to_string())]
    };

    Ok(PdfParseResult {
        title,
        chapters,
    })
}

fn count_pdf_pages(content: &str) -> usize {
    let mut count = 0;
    let mut search_from = 0;
    while let Some(idx) = content[search_from..].find("/Type") {
        let rest = &content[search_from + idx..];
        if rest.starts_with("/Type /Page") && !rest.starts_with("/Type /Pages") {
            count += 1;
        }
        search_from += idx + 5;
    }
    if count == 0 {
        count = content.matches("/Page").count();
        if count > 0 {
            count = count / 2;
        }
    }
    count.max(1)
}

use std::path::Path;
use std::io::{BufReader, Read, Write, BufWriter};
use chardetng::EncodingDetector;
use encoding_rs::Encoding;
use regex::Regex;

pub fn detect_encoding(file_path: &Path) -> Result<String, String> {
    let file = std::fs::File::open(file_path).map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(file);
    let mut buffer = vec![0u8; 1024 * 1024];
    let n = reader.read(&mut buffer).map_err(|e| e.to_string())?;
    let sample = &buffer[..n];

    let mut detector = EncodingDetector::new();
    detector.feed(sample, true);
    let encoding = detector.guess(None, true);

    let name = if encoding == encoding_rs::UTF_8 {
        "UTF-8".to_string()
    } else if encoding == encoding_rs::GBK {
        "GBK".to_string()
    } else if encoding == encoding_rs::WINDOWS_1252 && looks_like_gb18030(sample) {
        "GB18030".to_string()
    } else {
        encoding.name().to_string()
    };

    Ok(name)
}

// chardetng 对部分 GBK 中文长文本会误判为 windows-1252（单字节编码接受任意字节，
// 统计得分偶发占优）。GB18030 解码对西文文本会迅速遇到非法序列，
// 以「高字节占比 + 低错误率」复核，成立时改判 GB18030。
fn looks_like_gb18030(sample: &[u8]) -> bool {
    let high = sample.iter().filter(|b| **b >= 0x80).count();
    if high * 5 < sample.len() {
        return false;
    }
    let mut decoder = encoding_rs::GB18030.new_decoder();
    let mut dst = [0u16; 1024];
    let mut total = 0usize;
    let mut bad = 0usize;
    let mut offset = 0usize;
    while offset < sample.len() {
        let end = (offset + 4096).min(sample.len());
        let last = end == sample.len();
        let (result, read, _written, had_errors) =
            decoder.decode_to_utf16(&sample[offset..end], &mut dst, last);
        if read == 0 {
            if matches!(result, encoding_rs::CoderResult::InputEmpty) {
                break;
            }
            bad += 1;
            total += 1;
            offset += 1;
            continue;
        }
        total += read;
        if had_errors {
            bad += read;
        }
        offset += read;
    }
    total > 0 && bad * 100 < total
}

pub fn transcode_to_utf8(file_path: &Path, encoding: &str, cache_path: &Path) -> Result<usize, String> {
    let enc = Encoding::for_label(encoding.as_bytes())
        .unwrap_or(encoding_rs::UTF_8);

    let file = std::fs::File::open(file_path).map_err(|e| e.to_string())?;
    let mut reader = BufReader::with_capacity(64 * 1024, file);

    let cache_file = std::fs::File::create(cache_path).map_err(|e| e.to_string())?;
    let mut writer = BufWriter::new(cache_file);

    let mut decoder = enc.new_decoder();
    let mut total_chars = 0usize;
    let mut input_buf = vec![0u8; 64 * 1024];
    let mut output_buf = String::with_capacity(128 * 1024);

    loop {
        let n = reader.read(&mut input_buf).map_err(|e| e.to_string())?;
        if n == 0 {
            output_buf.clear();
            let (_result, written, _had_errors) = decoder.decode_to_string(
                b"",
                &mut output_buf,
                true,
            );
            if written > 0 {
                writer.write_all(output_buf.as_bytes()).map_err(|e| e.to_string())?;
                total_chars += written;
            }
            break;
        }

        output_buf.clear();
        let (_result, written, _had_errors) = decoder.decode_to_string(
            &input_buf[..n],
            &mut output_buf,
            false,
        );

        if written > 0 {
            writer.write_all(output_buf.as_bytes()).map_err(|e| e.to_string())?;
            total_chars += written;
        }
    }

    writer.flush().map_err(|e| e.to_string())?;
    Ok(total_chars)
}

pub fn split_chapters(utf8_content: &str) -> Vec<(Option<String>, usize)> {
    let patterns = [
        Regex::new(r"(?m)^[ \t]*第[零一二三四五六七八九十百千万0-9]+[章回节卷][\s:：、]").unwrap(),
        Regex::new(r"(?m)^[ \t]*Chapter\s+\d+").unwrap(),
        Regex::new(r"(?m)^[ \t]*CHAPTER\s+\d+").unwrap(),
    ];

    // 直接在原文上找行首匹配：str::lines() 会把 \r\n / \r\r\n 的 \r 吞掉，
    // 按行累加的偏移会逐章漂移。location 统一为 UTF-16 码元偏移，
    // 与前端 JS 字符串索引严格一致。
    let mut starts: Vec<usize> = Vec::new();
    for pattern in &patterns {
        for m in pattern.find_iter(utf8_content) {
            starts.push(m.start());
        }
    }
    starts.sort_unstable();
    starts.dedup();

    let mut matches: Vec<(Option<String>, usize)> = Vec::new();
    let mut prev = 0usize;
    let mut utf16 = 0usize;
    for &s in &starts {
        utf16 += utf8_content[prev..s].encode_utf16().count();
        prev = s;
        let rest = &utf8_content[s..];
        let end = rest
            .find(|c| c == '\n' || c == '\r')
            .map(|i| s + i)
            .unwrap_or(utf8_content.len());
        matches.push((Some(utf8_content[s..end].trim().to_string()), utf16));
    }

    if matches.is_empty() {
        let chunk_size = 6000;
        let total = utf8_content.chars().count();
        let mut char_i = 0usize;
        let mut utf16 = 0usize;
        let mut next_boundary = 0usize;
        for c in utf8_content.chars() {
            if char_i == next_boundary {
                let end = std::cmp::min(char_i + chunk_size, total);
                let title = if char_i == 0 {
                    Some(format!("段落 1-{}", end))
                } else {
                    Some(format!("段落 {}-{}", char_i + 1, end))
                };
                matches.push((title, utf16));
                next_boundary += chunk_size;
            }
            utf16 += c.len_utf16();
            char_i += 1;
        }
    }

    if matches.is_empty() {
        matches.push((Some("全文".to_string()), 0));
    }

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_sample(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("brookfile_txt_detect_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn split_offsets_index_raw_text_with_crlf() {
        let text = "序\r\r\n第一章 开始\r\n\r\n正文甲\r\r\n第二章 继续\r\n正文乙";
        let chapters = split_chapters(text);
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].0.as_deref(), Some("第一章 开始"));
        assert_eq!(chapters[1].0.as_deref(), Some("第二章 继续"));
        assert_eq!(chapters[0].1, 4);
        assert_eq!(chapters[1].1, 20);
    }

    #[test]
    fn split_indented_heading_after_blank_lines() {
        let text = "《书》\n\r\n\r\n    第一章 甲\r\n\r\n    正文内容\r\n\r\n    第二章 乙\r\n    内容";
        let chapters = split_chapters(text);
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].0.as_deref(), Some("第一章 甲"));
        assert_eq!(chapters[1].0.as_deref(), Some("第二章 乙"));
        let u16s: Vec<u16> = text.encode_utf16().collect();
        for (title, off) in &chapters {
            let rest = String::from_utf16(&u16s[*off..]).unwrap();
            assert!(rest.starts_with("    "), "offset 应指向标题行行首（含缩进）");
            let line_end = rest.find('\n').unwrap();
            assert_eq!(rest[..line_end].trim(), title.clone().unwrap());
        }
    }

    #[test]
    fn split_offsets_are_utf16_units() {
        let text = "🎉\r\n第一章 一\r\n正文";
        let chapters = split_chapters(text);
        assert_eq!(chapters.len(), 1);
        assert_eq!(chapters[0].1, "🎉\r\n".encode_utf16().count());
    }

    #[test]
    fn split_forced_chunks_cover_whole_text() {
        let text = "甲".repeat(15000);
        let chapters = split_chapters(&text);
        assert_eq!(chapters.len(), 3);
        assert_eq!(chapters[0].0.as_deref(), Some("段落 1-6000"));
        assert_eq!(chapters[1].0.as_deref(), Some("段落 6001-12000"));
        assert_eq!(chapters[2].0.as_deref(), Some("段落 12001-15000"));
        assert_eq!(chapters[0].1, 0);
        let u16s: Vec<u16> = text.encode_utf16().collect();
        for (_, off) in &chapters {
            assert!(*off < u16s.len());
        }
        assert_eq!(chapters[1].1, 6000);
        assert_eq!(chapters[2].1, 12000);
    }

    #[test]
    fn detect_gbk_chinese_novel() {
        let text = "《大奉打更人》\r\n\r\n第一章 牢狱之灾\r\n\r\n    大奉京城许七安，监牢之中彻夜难眠。\r\n    次日天还没亮，狱卒便送来了一碗热腾腾的牢饭。\r\n".repeat(300);
        let (bytes, _, _) = encoding_rs::GBK.encode(&text);
        let path = write_sample("gbk_sample.txt", &bytes);
        let enc = detect_encoding(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert!(
            enc.eq_ignore_ascii_case("gbk") || enc.eq_ignore_ascii_case("gb18030"),
            "GBK 中文被检测为: {}",
            enc
        );
    }

    #[test]
    fn detect_utf8() {
        let text = "《大奉打更人》\r\n第一章 牢狱之灾\r\n".repeat(100);
        let path = write_sample("utf8_sample.txt", text.as_bytes());
        let enc = detect_encoding(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(enc.to_ascii_lowercase(), "utf-8");
    }

    #[test]
    fn detect_windows1252_not_rescued_as_gb() {
        let text = "Café déjà vu — naïve résumé à côté. ".repeat(500);
        let (bytes, _, _) = encoding_rs::WINDOWS_1252.encode(&text);
        let path = write_sample("w1252_sample.txt", &bytes);
        let enc = detect_encoding(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert!(
            !enc.eq_ignore_ascii_case("gb18030") && !enc.eq_ignore_ascii_case("gbk"),
            "西文被误判为 GB: {}",
            enc
        );
    }

    #[test]
    fn looks_like_gb18030_discriminates() {
        let gbk_text = "《大奉打更人》第一章 牢狱之灾，许七安在监牢之中彻夜难眠。".repeat(50);
        let (gbk, _, _) = encoding_rs::GBK.encode(&gbk_text);
        assert!(looks_like_gb18030(&gbk));
        let w1252_text = "Café déjà vu, naïve résumé à côté. ".repeat(50);
        let (w1252, _, _) = encoding_rs::WINDOWS_1252.encode(&w1252_text);
        assert!(!looks_like_gb18030(&w1252));
        assert!(!looks_like_gb18030(b"plain ascii text only"));
    }
}

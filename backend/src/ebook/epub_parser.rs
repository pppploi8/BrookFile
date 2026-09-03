use std::path::Path;
use std::io::Read;
use quick_xml::Reader;
use quick_xml::events::Event;

pub struct EpubParseResult {
    pub title: String,
    pub chapters: Vec<(Option<String>, String)>,
    pub cover_image: Option<Vec<u8>>,
}

fn attr_to_string(val: &[u8]) -> String {
    String::from_utf8_lossy(val).to_string()
}

pub fn parse_epub(file_path: &Path) -> Result<EpubParseResult, String> {
    let file = std::fs::File::open(file_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

    let mut container_xml = String::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        if name == "META-INF/container.xml" {
            entry.read_to_string(&mut container_xml).map_err(|e| e.to_string())?;
            break;
        }
    }

    let opf_path = extract_opf_path(&container_xml)?;

    let mut opf_content = String::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.name() == opf_path {
            entry.read_to_string(&mut opf_content).map_err(|e| e.to_string())?;
            break;
        }
    }

    let (title, _author, spine_ids, _manifest_id_to_href, cover_href) =
        parse_opf(&opf_content)?;

    let mut chapters = Vec::new();
    for (i, spine_id) in spine_ids.iter().enumerate() {
        let title = if i == 0 {
            Some(title.clone())
        } else {
            None
        };
        chapters.push((title, spine_id.clone()));
    }

    let cover_image = if let Some(ref cover_href) = cover_href {
        let cover_path = if opf_path.contains('/') {
            let opf_dir = &opf_path[..opf_path.rfind('/').unwrap_or(0)];
            format!("{}/{}", opf_dir, cover_href)
        } else {
            cover_href.clone()
        };

        let mut image_data = None;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
            if entry.name() == cover_path || entry.name().ends_with(&format!("/{}", cover_href)) {
                let mut data = Vec::new();
                entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
                image_data = Some(data);
                break;
            }
        }
        image_data
    } else {
        None
    };

    Ok(EpubParseResult {
        title: if title.is_empty() {
            file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string()
        } else {
            title
        },
        chapters,
        cover_image,
    })
}

fn extract_opf_path(container_xml: &str) -> Result<String, String> {
    let mut reader = Reader::from_str(container_xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut opf_path = String::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                if e.name().as_ref() == b"rootfile" {
                    for attr in e.attributes() {
                        if let Ok(attr) = attr {
                            if attr.key.as_ref() == b"full-path" {
                                opf_path = attr_to_string(&attr.value);
                            }
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(e.to_string()),
            _ => {}
        }
    }
    if opf_path.is_empty() {
        Err("OPF path not found in container.xml".to_string())
    } else {
        Ok(opf_path)
    }
}

fn parse_opf(opf_content: &str) -> Result<
    (
        String,
        Option<String>,
        Vec<String>,
        std::collections::HashMap<String, String>,
        Option<String>,
    ),
    String,
> {
    let mut reader = Reader::from_str(opf_content);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut title = String::new();
    let mut author = None;
    let mut spine_ids: Vec<String> = Vec::new();
    let mut manifest_id_to_href: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut cover_href: Option<String> = None;

    let mut in_metadata = false;
    let mut current_element = String::new();
    let mut cover_id: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "metadata" {
                    in_metadata = true;
                }
                if name == "item" {
                    let mut id = String::new();
                    let mut href = String::new();
                    let mut properties = String::new();
                    let mut media_type = String::new();
                    for attr in e.attributes() {
                        if let Ok(attr) = attr {
                            match attr.key.as_ref() {
                                b"id" => id = attr_to_string(&attr.value),
                                b"href" => href = attr_to_string(&attr.value),
                                b"properties" => properties = attr_to_string(&attr.value),
                                b"media-type" => media_type = attr_to_string(&attr.value),
                                _ => {}
                            }
                        }
                    }
                    if !id.is_empty() && !href.is_empty() {
                        manifest_id_to_href.insert(id.clone(), href.clone());
                        if properties.contains("cover-image") {
                            cover_href = Some(href.clone());
                        }
                        if media_type.starts_with("image/") && (id == "cover" || id.contains("cover")) {
                            if cover_href.is_none() {
                                cover_href = Some(href);
                            }
                        }
                    }
                }
                if name == "itemref" {
                    let mut idref = String::new();
                    for attr in e.attributes() {
                        if let Ok(attr) = attr {
                            if attr.key.as_ref() == b"idref" {
                                idref = attr_to_string(&attr.value);
                            }
                        }
                    }
                    if !idref.is_empty() {
                        spine_ids.push(idref);
                    }
                }
                current_element = name;
            }
            Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "meta" {
                    let mut name_attr = String::new();
                    let mut content_attr = String::new();
                    for attr in e.attributes() {
                        if let Ok(attr) = attr {
                            match attr.key.as_ref() {
                                b"name" => name_attr = attr_to_string(&attr.value),
                                b"content" => content_attr = attr_to_string(&attr.value),
                                _ => {}
                            }
                        }
                    }
                    if name_attr == "cover" {
                        cover_id = Some(content_attr);
                    }
                }
                if name == "item" {
                    let mut id = String::new();
                    let mut href = String::new();
                    let mut properties = String::new();
                    let mut media_type = String::new();
                    for attr in e.attributes() {
                        if let Ok(attr) = attr {
                            match attr.key.as_ref() {
                                b"id" => id = attr_to_string(&attr.value),
                                b"href" => href = attr_to_string(&attr.value),
                                b"properties" => properties = attr_to_string(&attr.value),
                                b"media-type" => media_type = attr_to_string(&attr.value),
                                _ => {}
                            }
                        }
                    }
                    if !id.is_empty() && !href.is_empty() {
                        manifest_id_to_href.insert(id.clone(), href.clone());
                        if properties.contains("cover-image") {
                            cover_href = Some(href.clone());
                        }
                        if media_type.starts_with("image/") && (id == "cover" || id.contains("cover")) {
                            if cover_href.is_none() {
                                cover_href = Some(href);
                            }
                        }
                    }
                }
                if name == "itemref" {
                    let mut idref = String::new();
                    for attr in e.attributes() {
                        if let Ok(attr) = attr {
                            if attr.key.as_ref() == b"idref" {
                                idref = attr_to_string(&attr.value);
                            }
                        }
                    }
                    if !idref.is_empty() {
                        spine_ids.push(idref);
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_metadata {
                    let text = e.unescape().map_err(|e| e.to_string())?.to_string();
                    match current_element.as_str() {
                        "dc:title" | "title" => {
                            if !text.trim().is_empty() {
                                title = text.trim().to_string();
                            }
                        }
                        "dc:creator" | "creator" => {
                            if !text.trim().is_empty() {
                                author = Some(text.trim().to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "metadata" {
                    in_metadata = false;
                }
                current_element.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(e.to_string()),
            _ => {}
        }
    }

    if let Some(ref cid) = cover_id {
        if let Some(href) = manifest_id_to_href.get(cid) {
            if cover_href.is_none() {
                cover_href = Some(href.clone());
            }
        }
    }

    Ok((title, author, spine_ids, manifest_id_to_href, cover_href))
}

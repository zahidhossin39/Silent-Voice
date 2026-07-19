use reqwest::Client;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::time::Duration;

#[derive(Serialize)]
pub struct HfSearchItem {
    pub id: String,
    pub downloads: u64,
    pub likes: u64,
    pub last_modified: String,
    pub tags: Vec<String>,
    pub pipeline_tag: Option<String>,
    pub gated: bool,
    pub params_b: Option<f64>,
    pub siblings_gguf: u32,
}

fn deserialize_gated<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value: Value = Deserialize::deserialize(deserializer)?;
    match value {
        Value::Bool(b) => Ok(b),
        Value::String(s) => Ok(s.as_str() != "false"),
        _ => Ok(false),
    }
}

#[derive(Deserialize)]
struct HfSearchItemRaw {
    // No `_id` alias: HF sends both `id` and `_id`, and aliasing them to one
    // field makes serde reject the item as a duplicate.
    id: String,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    likes: u64,
    #[serde(rename = "lastModified", default)]
    last_modified: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(rename = "pipeline_tag")]
    pipeline_tag: Option<String>,
    #[serde(default, deserialize_with = "deserialize_gated")]
    gated: bool,
}

#[derive(Serialize)]
pub struct HfModelDetails {
    pub id: String,
    pub downloads: u64,
    pub likes: u64,
    pub last_modified: String,
    pub tags: Vec<String>,
    pub pipeline_tag: Option<String>,
    pub gated: bool,
    pub arch: Option<String>,
    pub params_b: Option<f64>,
    pub context_length: Option<u64>,
    /// gguf.chat_template mentions tools → the model supports tool calling.
    pub has_tools: bool,
    pub files: Vec<HfFile>,
    pub readme: String,
}

#[derive(Serialize)]
pub struct HfFile {
    pub name: String,
    pub size_bytes: u64,
}

fn get_client() -> Result<Client, String> {
    Client::builder()
        .user_agent("SilentVoice/0.1.7")
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| "Failed to build HTTP client".to_string())
}

fn map_hf_error(err: reqwest::Error) -> String {
    if err.is_timeout() {
        "Hugging Face is unreachable (timeout)".into()
    } else if err.is_status() {
        if err.status() == Some(reqwest::StatusCode::NOT_FOUND) {
            "Model not found".into()
        } else {
            format!("Hugging Face error: {}", err.status().unwrap_or(reqwest::StatusCode::BAD_REQUEST))
        }
    } else {
        "Hugging Face is unreachable".into()
    }
}

#[derive(Serialize)]
pub struct PiperVoice {
    pub key: String,
    pub name: String,
    pub language_code: String,
    pub language_english: String,
    pub country_english: String,
    pub quality: String,
    pub num_speakers: u32,
    pub onnx_path: String,
    pub onnx_size_bytes: u64,
    pub json_path: String,
}

#[tauri::command]
pub async fn hf_search_models(
    query: String,
    sort: String,
    limit: u32,
    track: String,
) -> Result<Vec<HfSearchItem>, String> {
    let sort_val = match sort.as_str() {
        "likes" => "likes",
        "lastModified" => "lastModified",
        "trending" => "trendingScore",
        _ => "downloads",
    };

    let is_empty = query.trim().is_empty();

    let url = if track == "stt" {
        let search_q = if is_empty {
            "whisper.cpp".to_string()
        } else {
            format!("{} whisper.cpp", query)
        };
        format!(
            "https://huggingface.co/api/models?search={}&sort={}&direction=-1&limit={}&full=true",
            urlencoding::encode(&search_q), sort_val, limit
        )
    } else {
        if is_empty {
            format!(
                "https://huggingface.co/api/models?filter=gguf&sort={}&direction=-1&limit={}&full=true",
                sort_val, limit
            )
        } else {
            format!(
                "https://huggingface.co/api/models?search={}&filter=gguf&sort={}&direction=-1&limit={}&full=true",
                urlencoding::encode(&query), sort_val, limit
            )
        }
    };

    let client = get_client()?;
    let res = client.get(&url).send().await.map_err(map_hf_error)?;
    let raw_items: Vec<Value> = res.json().await.map_err(map_hf_error)?;

    let mut items = Vec::new();
    for val in raw_items {
        let mut params_b = None;
        if let Some(gguf) = val.get("gguf") {
            if let Some(p) = gguf.get("parameters").or_else(|| gguf.get("total")).and_then(|v| v.as_f64()) {
                params_b = Some((p / 1_000_000_000.0 * 10.0).round() / 10.0);
            }
        }
        if params_b.is_none() {
            if let Some(st) = val.get("safetensors") {
                if let Some(p) = st.get("total").and_then(|v| v.as_f64()) {
                    params_b = Some((p / 1_000_000_000.0 * 10.0).round() / 10.0);
                }
            }
        }

        let mut siblings_gguf = 0;
        if let Some(siblings) = val.get("siblings").and_then(|v| v.as_array()) {
            for sib in siblings {
                if let Some(rfilename) = sib.get("rfilename").and_then(|v| v.as_str()) {
                    let basename = rfilename.rsplit('/').next().unwrap_or(rfilename);
                    if track == "stt" {
                        if basename.starts_with("ggml-") && basename.ends_with(".bin") {
                            siblings_gguf += 1;
                        }
                    } else {
                        if rfilename.ends_with(".gguf") {
                            siblings_gguf += 1;
                        }
                    }
                }
            }
        }

        if siblings_gguf == 0 {
            continue;
        }

        if let Ok(raw) = serde_json::from_value::<HfSearchItemRaw>(val.clone()) {
            items.push(HfSearchItem {
                id: raw.id,
                downloads: raw.downloads,
                likes: raw.likes,
                last_modified: raw.last_modified,
                tags: raw.tags,
                pipeline_tag: raw.pipeline_tag,
                gated: raw.gated,
                params_b,
                siblings_gguf,
            });
        }
    }

    Ok(items)
}

/// The id goes into a URL path — only "owner/name" with safe chars allowed.
fn validate_repo_id(repo_id: &str) -> bool {
    let mut parts = repo_id.split('/');
    let (Some(owner), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let ok = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
    };
    ok(owner) && ok(name)
}

pub fn strip_frontmatter(content: &str) -> String {
    if content.starts_with("---\n") || content.starts_with("---\r\n") {
        if let Some(end) = content[3..].find("\n---\n").or_else(|| content[3..].find("\n---\r\n")) {
            let offset = if content[3..].as_bytes().get(end + 4) == Some(&b'\r') { 6 } else { 5 };
            return content[3 + end + offset..].trim_start().to_string();
        }
    }
    content.to_string()
}

#[tauri::command]
pub async fn hf_model_details(repo_id: String, track: String) -> Result<HfModelDetails, String> {
    if !validate_repo_id(&repo_id) {
        return Err("Invalid repository ID".into());
    }

    let url = format!("https://huggingface.co/api/models/{}?blobs=true", repo_id);
    let client = get_client()?;
    
    let res = client.get(&url).send().await.map_err(map_hf_error)?;
    let data: Value = res.json().await.map_err(map_hf_error)?;

    if data.get("error").is_some() {
        return Err("Model not found".into());
    }

    let id = data["id"].as_str().unwrap_or("").to_string();
    let downloads = data["downloads"].as_u64().unwrap_or(0);
    let likes = data["likes"].as_u64().unwrap_or(0);
    let last_modified = data["lastModified"].as_str().unwrap_or("").to_string();
    
    let tags = data["tags"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
        
    let pipeline_tag = data["pipeline_tag"].as_str().map(|s| s.to_string());
    let gated = match &data["gated"] {
        Value::Bool(b) => *b,
        Value::String(s) => s != "false",
        _ => false,
    };

    let mut arch = None;
    let mut params_b = None;
    let mut context_length = None;
    let mut has_tools = false;

    if let Some(gguf) = data.get("gguf") {
        if let Some(a) = gguf.get("architecture").and_then(|v| v.as_str()) {
            arch = Some(a.to_string());
        }
        if let Some(p) = gguf.get("parameters").or_else(|| gguf.get("total")).and_then(|v| v.as_f64()) {
            params_b = Some((p / 1_000_000_000.0 * 10.0).round() / 10.0);
        }
        context_length = gguf.get("context_length").and_then(|v| v.as_u64());
        if let Some(tpl) = gguf.get("chat_template").and_then(|v| v.as_str()) {
            has_tools = tpl.contains("tool");
        }
    }
    
    if params_b.is_none() {
        if let Some(st) = data.get("safetensors") {
            if let Some(p) = st.get("total").and_then(|v| v.as_f64()) {
                params_b = Some((p / 1_000_000_000.0 * 10.0).round() / 10.0);
            }
        }
    }

    let mut files = Vec::new();
    if let Some(siblings) = data.get("siblings").and_then(|v| v.as_array()) {
        for sib in siblings {
            if let Some(rfilename) = sib.get("rfilename").and_then(|v| v.as_str()) {
                let basename = rfilename.rsplit('/').next().unwrap_or(rfilename);
                let matches = if track == "stt" {
                    basename.starts_with("ggml-") && basename.ends_with(".bin")
                } else {
                    rfilename.ends_with(".gguf")
                };
                if matches {
                    if let Some(size) = sib.get("size").and_then(|v| v.as_u64()) {
                        files.push(HfFile {
                            name: rfilename.to_string(),
                            size_bytes: size,
                        });
                    }
                }
            }
        }
    }

    let readme_url = format!("https://huggingface.co/{}/raw/main/README.md", repo_id);
    let readme_res = client.get(&readme_url).send().await;
    let mut readme = String::new();
    if let Ok(res) = readme_res {
        if res.status().is_success() {
            if let Ok(text) = res.text().await {
                // Truncate on a char boundary — String::truncate panics
                // mid-UTF-8, and READMEs are full of emoji/CJK.
                readme = strip_frontmatter(&text).chars().take(20000).collect();
            }
        }
    }

    Ok(HfModelDetails {
        id,
        downloads,
        likes,
        last_modified,
        tags,
        pipeline_tag,
        gated,
        arch,
        params_b,
        context_length,
        has_tools,
        files,
        readme,
    })
}

#[tauri::command]
pub async fn hf_piper_voices() -> Result<Vec<PiperVoice>, String> {
    let url = "https://huggingface.co/rhasspy/piper-voices/raw/main/voices.json";
    let client = get_client()?;
    let res = client.get(url).send().await.map_err(map_hf_error)?;
    let data: Value = res.json().await.map_err(map_hf_error)?;
    let obj = data.as_object().ok_or("Invalid voices.json format")?;
    
    let mut voices = Vec::new();
    for (key, val) in obj {
        let name = val["name"].as_str().unwrap_or("").to_string();
        let language_code = val["language"]["code"].as_str().unwrap_or("").to_string();
        let language_english = val["language"]["name_english"].as_str().unwrap_or("").to_string();
        let country_english = val["language"]["country_english"].as_str().unwrap_or("").to_string();
        let quality = val["quality"].as_str().unwrap_or("").to_string();
        let num_speakers = val["num_speakers"].as_u64().unwrap_or(1) as u32;
        
        let files = val.get("files").and_then(|f| f.as_object());
        let mut onnx_path = None;
        let mut onnx_size_bytes = 0;
        let mut json_path = None;
        
        if let Some(files_obj) = files {
            for (fpath, fval) in files_obj {
                if fpath.ends_with(".onnx") {
                    onnx_path = Some(fpath.to_string());
                    onnx_size_bytes = fval["size_bytes"].as_u64().unwrap_or(0);
                } else if fpath.ends_with(".onnx.json") {
                    json_path = Some(fpath.to_string());
                }
            }
        }
        
        if let (Some(onnx), Some(json)) = (onnx_path, json_path) {
            voices.push(PiperVoice {
                key: key.to_string(),
                name,
                language_code,
                language_english,
                country_english,
                quality,
                num_speakers,
                onnx_path: onnx,
                onnx_size_bytes,
                json_path: json,
            });
        }
    }
    
    voices.sort_by(|a, b| {
        a.language_english.cmp(&b.language_english)
            .then(a.name.cmp(&b.name))
            .then(a.quality.cmp(&b.quality))
    });
    
    Ok(voices)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Live network test: exercises the real commands end-to-end. Passes
    // vacuously offline (network errors are not assertion failures).
    #[tokio::test]
    async fn live_search_and_details() {
        let items = match hf_search_models("qwen".into(), "downloads".into(), 5, "llm".into()).await {
            Ok(i) => i,
            Err(e) => {
                println!("live_search skip (offline?): {e}");
                return;
            }
        };
        assert!(!items.is_empty(), "search returned no GGUF repos");
        assert!(items[0].downloads > 0, "downloads missing: {:?}", items[0].id);
        assert!(!items[0].last_modified.is_empty(), "lastModified missing");

        let browse_items = hf_search_models("".into(), "trending".into(), 5, "llm".into()).await.expect("browse failed");
        assert!(!browse_items.is_empty(), "browse returned no GGUF repos");
        assert!(browse_items[0].siblings_gguf > 0);

        let details = hf_model_details(items[0].id.clone(), "llm".into()).await.expect("details failed");
        assert!(!details.files.is_empty(), "no gguf files with sizes for {}", details.id);
        assert!(details.files.iter().all(|f| f.size_bytes > 0));
        println!(
            "OK {}: {} files, arch={:?}, params_b={:?}, ctx={:?}, tools={}, readme_len={}",
            details.id, details.files.len(), details.arch, details.params_b,
            details.context_length, details.has_tools, details.readme.len()
        );
    }

    #[test]
    fn test_validate_repo_id() {
        assert!(validate_repo_id("meta-llama/Llama-2-7b-chat-hf"));
        assert!(validate_repo_id("TheBloke/Mistral-7B-Instruct-v0.2-GGUF"));
        assert!(!validate_repo_id("invalid_repo_name"));
        assert!(!validate_repo_id("meta/llama/Llama-2-7b"));
        assert!(!validate_repo_id("meta-llama/"));
    }

    #[test]
    fn test_strip_frontmatter() {
        let content_with_frontmatter = "---\nlanguage: en\n---\n# Readme Title\nSome content";
        assert_eq!(strip_frontmatter(content_with_frontmatter), "# Readme Title\nSome content");

        let content_with_crlf = "---\r\nlanguage: en\r\n---\r\n# Readme Title";
        assert_eq!(strip_frontmatter(content_with_crlf), "# Readme Title");

        let content_without_frontmatter = "# Readme Title\nSome content";
        assert_eq!(strip_frontmatter(content_without_frontmatter), "# Readme Title\nSome content");

        let malformed_frontmatter = "---\nlanguage: en\n# Readme Title";
        assert_eq!(strip_frontmatter(malformed_frontmatter), "---\nlanguage: en\n# Readme Title");
    }

    #[test]
    fn test_deserialize_gated() {
        let json1 = r#"{"id": "a/b", "gated": false}"#;
        let item1: HfSearchItemRaw = serde_json::from_str(json1).unwrap();
        assert!(!item1.gated);

        let json2 = r#"{"id": "a/b", "gated": "auto"}"#;
        let item2: HfSearchItemRaw = serde_json::from_str(json2).unwrap();
        assert!(item2.gated);

        let json3 = r#"{"id": "a/b", "gated": true}"#;
        let item3: HfSearchItemRaw = serde_json::from_str(json3).unwrap();
        assert!(item3.gated);

        let json4 = r#"{"id": "a/b"}"#;
        let item4: HfSearchItemRaw = serde_json::from_str(json4).unwrap();
        assert!(!item4.gated);

        let json5 = r#"{"id": "a/b", "gated": "false"}"#;
        let item5: HfSearchItemRaw = serde_json::from_str(json5).unwrap();
        assert!(!item5.gated);
    }

    #[tokio::test]
    async fn live_piper_voices() {
        let voices = match hf_piper_voices().await {
            Ok(v) => v,
            Err(e) => {
                println!("live_piper_voices skip (offline?): {e}");
                return;
            }
        };
        assert!(!voices.is_empty(), "voices missing");
        assert!(!voices[0].name.is_empty());
        assert!(!voices[0].onnx_path.is_empty());
        assert!(voices[0].onnx_size_bytes > 0);
    }
}

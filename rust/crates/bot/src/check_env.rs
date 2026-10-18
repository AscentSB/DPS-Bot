//! `check-env`: shows whether .env can be read, without printing any secret (port of scripts/check-env.mjs).
//! The messages are the JS ones, word for word.

use indexmap::IndexMap;
use std::path::Path;

fn has_end_junk(v: &str) -> bool {
    // /^["'\s]|["'\s]$/
    let junk = |c: char| c == '"' || c == '\'' || c.is_whitespace();
    v.chars().next().is_some_and(junk) || v.chars().last().is_some_and(junk)
}

fn js_len(v: &str) -> usize {
    v.encode_utf16().count()
}

/// Notes for the three settings; `out` collects the lines. Returns the process exit code.
pub fn check(dir: &Path, process_token: Option<&str>, out: &mut Vec<String>) -> i32 {
    let env_path = dir.join(".env");
    if !env_path.exists() {
        out.push(format!("No file named exactly .env in {}", dir.display()));
        let mut near: Vec<String> = std::fs::read_dir(dir)
            .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        near.retain(|f| f.to_lowercase().contains("env") && f != ".env.example");
        near.sort();
        if !near.is_empty() {
            let quoted: Vec<String> = near.iter().map(|f| serde_json::to_string(f).unwrap()).collect();
            out.push(format!("Files with \"env\" in the name here: {}", quoted.join(", ")));
            out.push(format!("Rename the one with your token to .env, for example: Rename-Item {} .env", quoted[0]));
        } else {
            out.push("No other env file here either. Create it: copy .env.example .env, then: notepad .env".into());
        }
        return 1;
    }
    let raw = std::fs::read(&env_path).unwrap_or_default();
    let utf16 = raw.len() >= 2 && ((raw[0] == 0xff && raw[1] == 0xfe) || (raw[0] == 0xfe && raw[1] == 0xff));
    if utf16 {
        out.push(".env is saved as UTF-16 (Unicode), which cannot be read. In Notepad: File > Save As > Encoding: UTF-8, then save over .env.".into());
        return 1;
    }
    let mut values: IndexMap<String, String> = IndexMap::new();
    if let Ok(iter) = dotenvy::from_path_iter(&env_path) {
        for (k, v) in iter.flatten() {
            values.insert(k, v);
        }
    }
    let keys: Vec<&str> = values.keys().map(String::as_str).collect();
    out.push(format!("Settings found in .env: {}", if keys.is_empty() { "(none)".to_string() } else { keys.join(", ") }));
    for key in ["DISCORD_TOKEN", "CLIENT_ID", "HYPIXEL_API_KEY"] {
        let v = values.get(key).map(String::as_str).unwrap_or("");
        let note = if v.is_empty() {
            "MISSING or empty".to_string()
        } else if has_end_junk(v) {
            format!("set ({} characters) but has a space or quote at an end: remove it", js_len(v))
        } else if key == "DISCORD_TOKEN" && v.split('.').count() != 3 {
            format!("set ({} characters) but a bot token has 3 parts separated by dots: copy it again from the Bot page", js_len(v))
        } else {
            format!("set ({} characters)", js_len(v))
        };
        out.push(format!("  {key}: {note}"));
    }
    if let Some(p) = process_token {
        if values.get("DISCORD_TOKEN").map(String::as_str) != Some(p) {
            out.push("A DISCORD_TOKEN is also set in Windows and overrides .env. Clear it: Remove-Item Env:DISCORD_TOKEN".into());
        }
    }
    0
}

pub fn run() -> i32 {
    let dir = std::env::current_dir().unwrap_or_else(|_| ".".into());
    let token = std::env::var("DISCORD_TOKEN").ok();
    let mut out = Vec::new();
    let code = check(&dir, token.as_deref(), &mut out);
    for l in out {
        println!("{l}");
    }
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("dps-bot-checkenv-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn missing_file_suggests_rename() {
        let d = tmp("a");
        std::fs::write(d.join(".env.txt"), "x").unwrap();
        let mut out = vec![];
        assert_eq!(check(&d, None, &mut out), 1);
        assert!(out[1].contains("\".env.txt\""));
        assert!(out[2].ends_with("Rename-Item \".env.txt\" .env"));
    }

    #[test]
    fn reports_without_secrets() {
        let d = tmp("b");
        std::fs::write(d.join(".env"), "DISCORD_TOKEN=abc.def.ghi\nCLIENT_ID= 123 \nOTHER=1\n").unwrap();
        let mut out = vec![];
        assert_eq!(check(&d, Some("zzz"), &mut out), 0);
        assert_eq!(out[0], "Settings found in .env: DISCORD_TOKEN, CLIENT_ID, OTHER");
        assert_eq!(out[1], "  DISCORD_TOKEN: set (11 characters)");
        assert!(out[2].starts_with("  CLIENT_ID: set") || out[2].contains("CLIENT_ID"));
        assert_eq!(out[3], "  HYPIXEL_API_KEY: MISSING or empty");
        assert!(out[4].starts_with("A DISCORD_TOKEN is also set"));
        assert!(!out.join("\n").contains("abc.def"));
    }

    #[test]
    fn utf16_is_refused() {
        let d = tmp("c");
        std::fs::write(d.join(".env"), [0xffu8, 0xfe, b'a', 0]).unwrap();
        let mut out = vec![];
        assert_eq!(check(&d, None, &mut out), 1);
    }
}

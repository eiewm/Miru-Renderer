use std::collections::HashMap;
pub struct SkinIniData {
    pub sections: HashMap<String, HashMap<String, String>>,
}
impl SkinIniData {
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.sections.get(section)?.get(key).map(|s| s.as_str())
    }
    pub fn get_int(&self, section: &str, key: &str) -> Option<i32> {
        self.get(section, key)?.parse().ok()
    }
    pub fn get_float(&self, section: &str, key: &str) -> Option<f32> {
        self.get(section, key)?.parse().ok()
    }
    pub fn get_bool(&self, section: &str, key: &str) -> Option<bool> {
        let val = self.get(section, key)?;
        match val.to_lowercase().as_str() {
            "1" | "true" | "yes" => Some(true),
            "0" | "false" | "no" => Some(false),
            _ => None,
        }
    }
    pub fn get_color(&self, section: &str, key: &str) -> Option<(u8, u8, u8, u8)> {
        let val = self.get(section, key)?;
        let parts: Vec<&str> = val.split(',').collect();
        if parts.len() < 3 {
            return None;
        }
        let r = parts[0].trim().parse().ok()?;
        let g = parts[1].trim().parse().ok()?;
        let b = parts[2].trim().parse().ok()?;
        let a = parts
            .get(3)
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(255);
        // Skin.ini colors usually omit alpha; treat missing alpha as fully opaque.
        Some((r, g, b, a))
    }
}
pub fn parse_skin_ini(content: &str) -> SkinIniData {
    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut current_section = String::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].to_string();
            sections.entry(current_section.clone()).or_default();
            continue;
        }
        // osu! skin files commonly mix key:value and key=value syntax.
        let sep = if line.contains(':') {
            ':'
        } else if line.contains('=') {
            '='
        } else {
            continue;
        };
        if let Some((key, val)) = line.split_once(sep) {
            let key = key.trim().to_string();
            let val = val.trim().to_string();
            if !current_section.is_empty() {
                sections
                    .entry(current_section.clone())
                    .or_default()
                    .insert(key, val);
            }
        }
    }
    SkinIniData { sections }
}

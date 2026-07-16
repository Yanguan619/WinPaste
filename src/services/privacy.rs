/// Privacy protection: regex-based detection and encryption of sensitive content.
use std::sync::atomic::Ordering;

/// Check if content contains sensitive data based on enabled privacy kinds.
/// Returns the original content if no sensitive data found, or encrypted content.
pub fn maybe_encrypt_content(content: &str, content_type: &str) -> Option<String> {
    if content_type == "image" || content_type == "file" {
        return None;
    }

    let settings = if let Some(guard) = crate::APP_CTX.get() {
        let ctx = guard.lock().unwrap();
        ctx.settings.clone()
    } else {
        return None;
    };

    if !settings.privacy_protection.load(Ordering::Relaxed) {
        return None;
    }

    let kinds = settings.privacy_protection_kinds.lock().unwrap();
    let custom_rules = settings.privacy_protection_custom_rules.lock().unwrap();

    let mut has_sensitive = false;

    for kind in kinds.iter() {
        match kind.as_str() {
            "phone" => {
                if contains_phone(content) {
                    has_sensitive = true;
                    break;
                }
            }
            "idcard" => {
                if contains_id_card(content) {
                    has_sensitive = true;
                    break;
                }
            }
            "email" => {
                if contains_email(content) {
                    has_sensitive = true;
                    break;
                }
            }
            "secret" | "password" => {
                if contains_secret(content) {
                    has_sensitive = true;
                    break;
                }
            }
            _ => {}
        }
    }

    // Check custom rules
    if !has_sensitive && !custom_rules.is_empty() {
        for rule in custom_rules.split('\n') {
            let rule = rule.trim();
            if rule.is_empty() {
                continue;
            }
            if let Ok(re) = regex::Regex::new(rule) {
                if re.is_match(content) {
                    has_sensitive = true;
                    break;
                }
            }
        }
    }

    if has_sensitive {
        Some(content.to_string())
    } else {
        None
    }
}

/// Check if content matches a Chinese phone number pattern.
fn contains_phone(content: &str) -> bool {
    // Chinese mobile: 1[3-9]\d{9}
    // Landline: 0\d{2,3}-?\d{7,8}
    let mobile_re = regex::Regex::new(r"1[3-9]\d{9}").unwrap();
    let landline_re = regex::Regex::new(r"0\d{2,3}-?\d{7,8}").unwrap();
    mobile_re.is_match(content) || landline_re.is_match(content)
}

/// Check if content matches a Chinese ID card number.
fn contains_id_card(content: &str) -> bool {
    // 18-digit ID: [1-9]\d{5}(19|20)\d{2}(0[1-9]|1[0-2])(0[1-9]|[12]\d|3[01])\d{3}[\dXx]
    let re = regex::Regex::new(r"[1-9]\d{5}(19|20)\d{2}(0[1-9]|1[0-2])(0[1-9]|[12]\d|3[01])\d{3}[\dXx]").unwrap();
    re.is_match(content)
}

/// Check if content contains an email address.
fn contains_email(content: &str) -> bool {
    let re = regex::Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap();
    re.is_match(content)
}

/// Check if content looks like a secret/key/password.
fn contains_secret(content: &str) -> bool {
    let lower = content.to_lowercase();
    // Check for common secret patterns
    let secret_patterns = [
        "api_key", "apikey", "api-key", "secret_key", "secretkey",
        "access_key", "accesskey", "private_key", "privatekey",
        "password", "passwd", "token", "bearer",
        "ghp_", "gho_", "github_pat_",
        "sk-", "sk_live_",
        "AKIA", // AWS
    ];
    for pattern in &secret_patterns {
        if lower.contains(pattern) {
            return true;
        }
    }
    // Check for base64-encoded keys (long alphanumeric strings)
    let key_re = regex::Regex::new(r"[A-Za-z0-9+/]{32,}={0,2}").unwrap();
    if key_re.is_match(content) && content.len() > 40 {
        return true;
    }
    false
}

/// Sanitize content for preview display (mask sensitive parts).
pub fn sanitize_preview(content: &str) -> String {
    let mut result = content.to_string();

    // Mask phone numbers
    if let Ok(re) = regex::Regex::new(r"(1[3-9]\d)(\d{4})(\d{4})") {
        result = re.replace_all(&result, "$1****$3").to_string();
    }

    // Mask ID cards
    if let Ok(re) = regex::Regex::new(r"([1-9]\d{5}(?:19|20)\d{2}(?:0[1-9]|1[0-2])(?:0[1-9]|[12]\d|3[01]))(\d{4})([\dXx])") {
        result = re.replace_all(&result, "$1****$3").to_string();
    }

    // Mask emails
    if let Ok(re) = regex::Regex::new(r"([a-zA-Z0-9._%+-]{2})([a-zA-Z0-9._%+-]*)(@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,})") {
        result = re.replace_all(&result, "$1***$3").to_string();
    }

    result
}

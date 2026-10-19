use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};

pub type Result<T> = std::result::Result<T, String>;
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
fn provider_date(s: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(s)
        .or_else(|_| chrono::DateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f%#z"))
        .map(|d| {
            d.with_timezone(&chrono::Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        })
        .unwrap_or_else(|_| s.to_string())
}
pub fn uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}
pub fn valid_id(id: &str) -> Result<()> {
    uuid::Uuid::parse_str(id)
        .map(|_| ())
        .map_err(|_| "Invalid identifier.".into())
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Identity {
    pub id: String,
    pub email: String,
    pub name: String,
    pub signature: String,
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Attachment {
    pub id: String,
    pub filename: String,
    pub content_type: String,
    pub size: u64,
    pub cached: bool,
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Message {
    pub id: String,
    pub provider_id: String,
    pub direction: String,
    pub from: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub reply_to: Vec<String>,
    pub subject: String,
    pub text: String,
    pub html: String,
    pub created_at: String,
    pub message_id: String,
    pub headers: BTreeMap<String, String>,
    pub attachments: Vec<Attachment>,
    pub status: String,
    pub folder: String,
    pub is_read: bool,
    pub starred: bool,
    pub labels: Vec<String>,
    pub hydrated: bool,
    pub links: Vec<String>,
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Draft {
    pub id: String,
    pub revision: i64,
    pub identity_id: String,
    pub to: String,
    pub cc: String,
    pub bcc: String,
    pub subject: String,
    pub text: String,
    pub html: String,
    pub attachments: Vec<Attachment>,
    pub headers: BTreeMap<String, String>,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Intent {
    pub id: String,
    pub draft_id: String,
    pub subject: String,
    pub to: Vec<String>,
    pub state: String,
    pub error: Option<String>,
    pub first_attempt: Option<i64>,
    pub provider_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub connected: bool,
    pub identities: Vec<Identity>,
    pub settings: Value,
    pub last_sync: Option<String>,
}
#[derive(Serialize)]
pub struct Snapshot {
    pub profile: Profile,
    pub messages: Vec<Message>,
    pub drafts: Vec<Draft>,
    pub outbox: Vec<Intent>,
    pub pending_bodies: i64,
    pub history_complete: bool,
    pub sync_error: Option<String>,
    pub inbox_unread: i64,
    pub labels: Vec<String>,
}
#[derive(Default, Deserialize)]
#[serde(default)]
pub struct MessageQuery {
    pub domain: String,
    pub include_subdomains: bool,
    pub folder: String,
    pub identity: String,
    pub search: String,
    pub limit: u32,
    pub offset: u32,
}
#[derive(Serialize)]
pub struct Probe {
    pub incoming: bool,
    pub outgoing: bool,
    pub incoming_error: Option<String>,
    pub outgoing_error: Option<String>,
}

pub fn clean_html(html: &str) -> String {
    // No URL-bearing attributes, styles, SVG, images, links, or active content are admitted.
    let tags: HashSet<&str> = [
        "p",
        "br",
        "div",
        "span",
        "strong",
        "b",
        "em",
        "i",
        "u",
        "s",
        "ul",
        "ol",
        "li",
        "blockquote",
        "pre",
        "code",
        "h1",
        "h2",
        "h3",
        "h4",
        "hr",
        "table",
        "thead",
        "tbody",
        "tr",
        "td",
        "th",
    ]
    .into_iter()
    .collect();
    ammonia::Builder::new()
        .tags(tags)
        .generic_attributes(HashSet::new())
        .tag_attributes(std::collections::HashMap::new())
        .link_rel(None)
        .clean(html)
        .to_string()
}
pub fn extract_links(html: &str) -> Vec<String> {
    let links = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let collected = links.clone();
    ammonia::Builder::new()
        .attribute_filter(move |tag, attr, value| {
            if tag == "a" && attr == "href" {
                if let Ok(url) = reqwest::Url::parse(value) {
                    if ["https", "http"].contains(&url.scheme())
                        && url.host_str().is_some()
                        && url.username().is_empty()
                        && url.password().is_none()
                    {
                        let mut out = collected.lock().unwrap();
                        if out.len() < 100 && !out.contains(&url.to_string()) {
                            out.push(url.to_string());
                        }
                    }
                }
            }
            None
        })
        .clean(html);
    let result = links.lock().unwrap().clone();
    result
}
pub fn address(s: &str) -> Result<String> {
    if s.contains(['\r', '\n', '\0']) || s.len() > 320 {
        return Err("Invalid email address or header.".into());
    }
    let list = mailparse::addrparse(s).map_err(|_| "Use an address such as name@example.com.")?;
    if list.len() != 1 {
        return Err("Enter one email address.".into());
    }
    match &list[0] {
        mailparse::MailAddr::Single(a) => {
            let parts: Vec<_> = a.addr.split('@').collect();
            if parts.len() != 2
                || parts.iter().any(|p| p.is_empty())
                || !a.addr.is_ascii()
                || a.addr.contains(char::is_whitespace)
            {
                return Err(
                    "Use a valid ASCII email address. Unicode display names are supported.".into(),
                );
            }
            Ok(a.addr.clone())
        }
        _ => Err("Address groups are not supported.".into()),
    }
}
pub fn recipients(s: &str) -> Result<Vec<String>> {
    if s.trim().is_empty() {
        return Ok(vec![]);
    }
    if s.contains(['\r', '\n', '\0']) {
        return Err("Recipient headers cannot contain line breaks.".into());
    }
    let list =
        mailparse::addrparse(&s.replace(';', ",")).map_err(|_| "Check recipient addresses.")?;
    let mut out = vec![];
    let mut seen = HashSet::new();
    for item in list.iter() {
        if let mailparse::MailAddr::Single(a) = item {
            address(&a.addr)?;
            let key = address_key(&a.addr);
            if seen.insert(key) {
                out.push(a.to_string());
            }
        } else {
            return Err("Address groups are not supported.".into());
        }
    }
    if out.len() > 50 {
        return Err("A message may have at most 50 recipients.".into());
    }
    Ok(out)
}
pub fn address_key(s: &str) -> String {
    let a = address(s).unwrap_or_else(|_| s.to_string());
    match a.rsplit_once('@') {
        Some((local, domain)) => format!("{}@{}", local, domain.to_lowercase()),
        None => a,
    }
}
pub fn validate_draft(d: &Draft, identities: &[Identity]) -> Result<Value> {
    let identity = identities
        .iter()
        .find(|i| i.id == d.identity_id)
        .ok_or("Choose a sender identity in Settings.")?;
    address(&identity.email)?;
    if identity.name.contains(['\r', '\n', '\0']) || d.subject.contains(['\r', '\n', '\0']) {
        return Err("Headers cannot contain line breaks.".into());
    }
    let to = recipients(&d.to)?;
    let cc = recipients(&d.cc)?;
    let bcc = recipients(&d.bcc)?;
    if to.is_empty() || to.len() + cc.len() + bcc.len() > 50 {
        return Err("Add a To recipient; at most 50 total recipients are supported.".into());
    }
    if d.html.len() + d.text.len() > 2_000_000 {
        return Err("The message body is too large (2 MB maximum).".into());
    }
    let mut headers = BTreeMap::new();
    for (k, v) in &d.headers {
        if !["In-Reply-To", "References"].contains(&k.as_str())
            || v.contains(['\r', '\n', '\0'])
            || v.len() > 8192
        {
            return Err("Invalid conversation header.".into());
        }
        headers.insert(k.clone(), v.clone());
    }
    let from = if identity.name.is_empty() {
        identity.email.clone()
    } else {
        format!(
            "\"{}\" <{}>",
            identity.name.replace('\\', "\\\\").replace('"', "\\\""),
            identity.email
        )
    };
    let signature = if identity.signature.is_empty() {
        String::new()
    } else {
        format!("\n\n{}", identity.signature)
    };
    let text = format!("{}{}", d.text, signature);
    let html = format!(
        "{}<p>{}</p>",
        clean_html(&d.html),
        escape_html(&signature).replace('\n', "<br>")
    );
    Ok(
        json!({"from":from,"to":to,"cc":cc,"bcc":bcc,"subject":d.subject,"text":text,"html":html,"headers":headers}),
    )
}
pub fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
pub fn list_strings(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => vec![s.clone()],
        Value::Array(a) => a
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect(),
        _ => vec![],
    }
}
pub fn provider_message(v: Value, direction: &str, hydrated: bool) -> Result<Message> {
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let provider_id = s("id");
    valid_id(&provider_id)?;
    let html = s("html");
    if html.len() > 2_000_000 || s("text").len() > 2_000_000 {
        return Err("Message exceeds the safe reader size limit.".into());
    }
    let headers = v["headers"]
        .as_object()
        .map(|h| {
            h.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.to_lowercase(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();
    let attachments = v["attachments"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|a| Attachment {
                    id: a["id"].as_str().unwrap_or("").into(),
                    filename: a["filename"].as_str().unwrap_or("attachment").into(),
                    content_type: a["content_type"]
                        .as_str()
                        .unwrap_or("application/octet-stream")
                        .into(),
                    size: a["size"].as_u64().unwrap_or(0),
                    cached: false,
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Message {
        id: format!("{direction}:{provider_id}"),
        provider_id,
        direction: direction.into(),
        from: s("from"),
        to: list_strings(&v["to"]),
        cc: list_strings(&v["cc"]),
        bcc: list_strings(&v["bcc"]),
        reply_to: list_strings(&v["reply_to"]),
        subject: s("subject"),
        text: s("text"),
        html: clean_html(&html),
        created_at: provider_date(&s("created_at")),
        message_id: s("message_id"),
        headers,
        attachments,
        status: if direction == "incoming" {
            "received".into()
        } else {
            v["last_event"].as_str().unwrap_or("accepted").into()
        },
        folder: if direction == "incoming" {
            "inbox".into()
        } else {
            "sent".into()
        },
        is_read: direction == "outgoing",
        starred: false,
        labels: vec![],
        hydrated,
        links: extract_links(&html),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hostile_html() {
        let html=clean_html("<script>fetch('ipc:')</script><img src='https://tracker/x'><svg onload='bad()'/><form><input></form><p style='background:url(https://tracker)'>Hello <b>世界</b><a href='javascript:bad()'>link</a></p>");
        for bad in [
            "script", "img", "svg", "form", "input", "style=", "href=", "src=", "onload",
        ] {
            assert!(!html.contains(bad), "{html}");
        }
        assert!(html.contains("世界"));
    }
    #[test]
    fn recipients_and_injection() {
        assert_eq!(
            recipients("\"你好, Friend\" <one@example.com>, one@example.com;two@example.com")
                .unwrap()
                .len(),
            2
        );
        assert!(recipients("a@example.com\r\nBcc: other@example.com").is_err());
        assert!(address("wrong@").is_err());
        assert!(address("x@例.com").is_err());
    }
    #[test]
    fn provider_nulls_and_unknown_status() {
        let m = provider_message(
            json!({"id":uuid(),"html":null,"text":null,"cc":null,"last_event":"future-event"}),
            "outgoing",
            true,
        )
        .unwrap();
        assert_eq!(m.status, "future-event");
        assert!(m.cc.is_empty());
    }
}

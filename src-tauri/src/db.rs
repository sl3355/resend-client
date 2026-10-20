use crate::model::*;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::path::Path;

pub struct Store {
    pub db: Connection,
}
fn err(e: impl std::fmt::Display) -> String {
    format!("Local storage error: {e}")
}
fn workspace_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 60 || name.chars().any(char::is_control) {
        return Err("Use a workspace name of 1–60 characters without control characters.".into());
    }
    Ok(name)
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let db = Connection::open(path).map_err(err)?;
        db.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(err)?;
        let version: i64 = db
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(err)?;
        if version == 0 {
            db.execute_batch(include_str!("../migrations/001.sql"))
                .map_err(err)?;
        } else if version != 1 && version != 2 {
            return Err("This profile uses a newer schema. Open it with a compatible version of Resend Client.".into());
        }
        if version < 2 {
            db.execute_batch(include_str!("../migrations/002.sql"))
                .map_err(err)?;
        }
        db.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;",
        )
        .map_err(err)?;
        db.execute("UPDATE intents SET state='unknown',error='Submission was interrupted. Checking with the same attempt identity.' WHERE state='submitting'",[]).map_err(err)?;
        let mut s = Self { db };
        if s.db
            .query_row("SELECT COUNT(*) FROM profiles WHERE active=1", [], |r| {
                r.get::<_, i64>(0)
            })
            .map_err(err)?
            == 0
        {
            s.new_profile()?;
        }
        Ok(s)
    }
    pub fn new_profile(&mut self) -> Result<String> {
        self.new_workspace("My workspace")
    }
    pub fn new_workspace(&mut self, name: &str) -> Result<String> {
        let name = workspace_name(name)?;
        let id = uuid();
        let tx = self.db.transaction().map_err(err)?;
        tx.execute("UPDATE profiles SET active=0", [])
            .map_err(err)?;
        tx.execute(
            "INSERT INTO profiles(id,active,name) VALUES(?1,1,?2)",
            params![id, name],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(id)
    }
    pub fn profile(&self) -> Result<Profile> {
        self.db
            .query_row(
                "SELECT id,connected,identities,settings,last_sync,name FROM profiles WHERE active=1",
                [],
                |r| {
                    let identities: String = r.get(2)?;
                    let settings: String = r.get(3)?;
                    Ok(Profile {
                        id: r.get(0)?,
                        name: r.get(5)?,
                        connected: r.get(1)?,
                        identities: serde_json::from_str(&identities).unwrap_or_default(),
                        settings: serde_json::from_str(&settings).unwrap_or(json!({})),
                        last_sync: r.get(4)?,
                    })
                },
            )
            .map_err(err)
    }
    pub fn require_active(&self, id: &str) -> Result<()> {
        if self.profile()?.id != id {
            return Err(
                "The active workspace changed. Return to the original workspace and try again."
                    .into(),
            );
        }
        Ok(())
    }
    pub fn workspaces(&self) -> Result<Vec<Workspace>> {
        let mut stmt = self.db.prepare("SELECT id,name,connected,(SELECT COUNT(*) FROM intents WHERE profile=profiles.id AND state NOT IN('accepted','cancelled')) FROM profiles ORDER BY rowid").map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Workspace {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    connected: r.get(2)?,
                    pending_sends: r.get(3)?,
                })
            })
            .map_err(err)?;
        rows.collect::<std::result::Result<_, _>>().map_err(err)
    }
    pub fn rename_workspace(&self, id: &str, name: &str) -> Result<()> {
        let name = workspace_name(name)?;
        self.db
            .execute("UPDATE profiles SET name=?1 WHERE id=?2", params![name, id])
            .map_err(err)?;
        Ok(())
    }
    pub fn switch_workspace(&mut self, id: &str) -> Result<()> {
        let tx = self.db.transaction().map_err(err)?;
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
                [id],
                |r| r.get(0),
            )
            .map_err(err)?;
        if !exists {
            return Err("Workspace no longer exists.".into());
        }
        tx.execute("UPDATE profiles SET active=0", [])
            .map_err(err)?;
        tx.execute("UPDATE profiles SET active=1 WHERE id=?1", [id])
            .map_err(err)?;
        tx.commit().map_err(err)
    }
    pub fn update_profile(&self, identities: &[Identity], settings: &Value) -> Result<()> {
        if identities.len() > 100 {
            return Err("At most 100 identities are supported.".into());
        }
        let mut seen = std::collections::HashSet::new();
        for i in identities {
            valid_id(&i.id)?;
            address(&i.email)?;
            if !seen.insert(address_key(&i.email)) {
                return Err("This address already has an identity.".into());
            }
            if i.name.contains(['\r', '\n', '\0'])
                || i.name.len() > 200
                || i.signature.len() > 10_000
            {
                return Err("Identity name or signature is invalid.".into());
            }
        }
        self.db
            .execute(
                "UPDATE profiles SET identities=?1,settings=?2 WHERE active=1",
                params![
                    serde_json::to_string(identities).map_err(err)?,
                    settings.to_string()
                ],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn snapshot(&self) -> Result<Snapshot> {
        let profile = self.profile()?;
        let pending_bodies = self
            .db
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE profile=?1 AND hydrated=0",
                [&profile.id],
                |r| r.get(0),
            )
            .map_err(err)?;
        let complete: i64 = self
            .db
            .query_row(
                "SELECT COUNT(*) FROM sync_state WHERE profile=?1 AND complete=1",
                [&profile.id],
                |r| r.get(0),
            )
            .map_err(err)?;
        let drafts = self.drafts(&profile.id)?;
        let outbox = self.intents(&profile.id)?;
        let sync_error = self
            .db
            .query_row(
                "SELECT sync_error FROM profiles WHERE id=?1",
                [&profile.id],
                |r| r.get(0),
            )
            .map_err(err)?;
        let inbox_unread = self
            .db
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE profile=?1 AND folder='inbox' AND is_read=0",
                [&profile.id],
                |r| r.get(0),
            )
            .map_err(err)?;
        let mut statement=self.db.prepare("SELECT DISTINCT j.value FROM messages,json_each(messages.labels) j WHERE profile=?1 ORDER BY j.value").map_err(err)?;
        let labels = statement
            .query_map([&profile.id], |r| r.get(0))
            .map_err(err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(Snapshot {
            profile,
            messages: self.list(&MessageQuery {
                folder: "all".into(),
                limit: 250,
                ..Default::default()
            })?,
            drafts,
            outbox,
            pending_bodies,
            history_complete: complete == 2,
            sync_error,
            inbox_unread,
            labels,
        })
    }
    pub fn list(&self, q: &MessageQuery) -> Result<Vec<Message>> {
        let p = self.profile()?;
        let mut sql = String::from(
            "SELECT document,folder,is_read,starred,labels,hydrated FROM messages WHERE profile=?",
        );
        let mut args: Vec<rusqlite::types::Value> = vec![p.id.into()];
        match q.folder.as_str() {
            "all" => sql.push_str(" AND folder!='trash'"),
            "starred" => sql.push_str(" AND starred=1 AND folder!='trash'"),
            "unmatched" => {
                sql.push_str(" AND direction='incoming' AND folder!='trash'");
                for i in &p.identities {
                    sql.push_str(" AND NOT EXISTS(SELECT 1 FROM json_each(document,'$.recipient_keys') WHERE value=?)");
                    args.push(address_key(&i.email).into());
                }
            }
            "" => {}
            folder => {
                sql.push_str(" AND folder=?");
                args.push(folder.to_string().into());
            }
        }
        if !q.identity.is_empty() || !q.domain.is_empty() {
            // Select the account side of the message, including Bcc in older cached documents.
            let addresses = "SELECT json_extract(document,'$.from') AS raw WHERE direction='outgoing' UNION ALL SELECT value FROM json_each(document,'$.to') WHERE direction='incoming' UNION ALL SELECT value FROM json_each(document,'$.cc') WHERE direction='incoming' UNION ALL SELECT value FROM json_each(document,'$.bcc') WHERE direction='incoming'";
            let parsed = format!("SELECT trim(CASE WHEN instr(raw,'<')>0 THEN substr(raw,instr(raw,'<')+1,instr(raw,'>')-instr(raw,'<')-1) ELSE raw END) AS addr FROM ({addresses})");
            if !q.identity.is_empty() {
                sql.push_str(&format!(" AND EXISTS(SELECT 1 FROM ({parsed}) WHERE substr(addr,1,instr(addr,'@')) || lower(substr(addr,instr(addr,'@')+1))=?)"));
                args.push(address_key(&q.identity).into());
            } else {
                let domain = q.domain.trim().to_lowercase();
                sql.push_str(&format!(" AND EXISTS(SELECT 1 FROM ({parsed}) WHERE lower(substr(addr,instr(addr,'@')+1))=?"));
                args.push(domain.clone().into());
                if q.include_subdomains {
                    sql.push_str(" OR lower(substr(addr,instr(addr,'@')+1)) LIKE ? ESCAPE '\\'");
                    args.push(format!("%.{}", like(&domain)).into());
                }
                sql.push(')');
            }
        }
        for term in search_terms(&q.search) {
            let pair = term.split_once(':');
            match pair {
                Some(("from", v)) | Some(("subject", v)) => {
                    let field = pair.unwrap().0;
                    sql.push_str(&format!(
                        " AND json_extract(document,'$.{field}') LIKE ? ESCAPE '\\'"
                    ));
                    args.push(format!("%{}%", like(v)).into());
                }
                Some(("to", v)) => {
                    sql.push_str(" AND (EXISTS(SELECT 1 FROM json_each(document,'$.to') WHERE value LIKE ? ESCAPE '\\') OR EXISTS(SELECT 1 FROM json_each(document,'$.cc') WHERE value LIKE ? ESCAPE '\\'))");
                    let v = format!("%{}%", like(v));
                    args.push(v.clone().into());
                    args.push(v.into());
                }
                Some(("label", v)) => {
                    sql.push_str(
                        " AND EXISTS(SELECT 1 FROM json_each(labels) WHERE value=? COLLATE NOCASE)",
                    );
                    args.push(v.to_string().into());
                }
                Some(("after", v)) | Some(("before", v)) => {
                    if chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d").is_err() {
                        return Err("Use dates like after:2026-10-01.".into());
                    }
                    sql.push_str(if pair.unwrap().0 == "after" {
                        " AND created_at>=?"
                    } else {
                        " AND created_at<?"
                    });
                    args.push(v.to_string().into());
                }
                Some(("has", "attachment")) => {
                    sql.push_str(" AND json_array_length(document,'$.attachments')>0")
                }
                Some(("is", "unread")) => sql.push_str(" AND is_read=0"),
                Some(("is", "starred")) => sql.push_str(" AND starred=1"),
                _ => {
                    sql.push_str(" AND id IN(SELECT id FROM message_fts WHERE message_fts MATCH ? AND profile=?)");
                    args.push(format!("\"{}\"", term.replace('"', "\"\"")).into());
                    args.push(self.profile()?.id.into());
                }
            }
        }
        sql.push_str(" ORDER BY created_at DESC,id DESC LIMIT ? OFFSET ?");
        args.push(q.limit.clamp(1, 500).into());
        args.push(q.offset.min(1_000_000).into());
        let mut stmt = self.db.prepare(&sql).map_err(err)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(args), read_row)
            .map_err(err)?;
        let mut out = vec![];
        for row in rows {
            let mut m = row.map_err(err)?;
            m.html.clear();
            m.text = m.text.chars().take(180).collect();
            out.push(m);
        }
        Ok(out)
    }
    pub fn message(&self, profile: &str, id: &str) -> Result<Message> {
        self.db.query_row("SELECT document,folder,is_read,starred,labels,hydrated FROM messages WHERE profile=?1 AND id=?2",params![profile,id],read_row).map_err(|_|"Message is no longer available locally.".into())
    }
    pub fn upsert_page(
        &mut self,
        profile: &str,
        direction: &str,
        messages: &[Message],
        cursor: Option<&str>,
        complete: bool,
        advance: bool,
    ) -> Result<()> {
        let tx = self.db.transaction().map_err(err)?;
        for m in messages {
            upsert(&tx, profile, m)?;
        }
        if advance {
            tx.execute("INSERT INTO sync_state(profile,direction,cursor,complete) VALUES(?1,?2,?3,?4) ON CONFLICT(profile,direction) DO UPDATE SET cursor=excluded.cursor,complete=MAX(complete,excluded.complete)",params![profile,direction,cursor,complete]).map_err(err)?;
        }
        tx.commit().map_err(err)
    }
    pub fn hydrate(&mut self, profile: &str, m: &Message) -> Result<()> {
        let tx = self.db.transaction().map_err(err)?;
        upsert(&tx, profile, m)?;
        tx.commit().map_err(err)
    }
    pub fn flags(
        &mut self,
        id: &str,
        folder: Option<String>,
        is_read: Option<bool>,
        starred: Option<bool>,
        labels: Option<Vec<String>>,
    ) -> Result<()> {
        let p = self.profile()?.id;
        if let Some(ref f) = folder {
            if !["inbox", "sent", "archive", "trash"].contains(&f.as_str()) {
                return Err("Invalid folder.".into());
            }
        }
        if let Some(ref l) = labels {
            if l.len() > 30 || l.iter().any(|x| x.len() > 50 || x.trim().is_empty()) {
                return Err("Use up to 30 labels of 1–50 characters.".into());
            }
        }
        self.db.execute("UPDATE messages SET folder=COALESCE(?1,folder),is_read=COALESCE(?2,is_read),starred=COALESCE(?3,starred),labels=COALESCE(?4,labels) WHERE profile=?5 AND id=?6",params![folder,is_read,starred,labels.map(|v|json!(v).to_string()),p,id]).map_err(err)?;
        Ok(())
    }
    pub fn remove_message(&mut self, id: &str) -> Result<()> {
        let p = self.profile()?.id;
        let m = self.message(&p, id)?;
        if m.folder != "trash" {
            return Err("Move the message to Trash first.".into());
        }
        let tx = self.db.transaction().map_err(err)?;
        tx.execute(
            "INSERT OR IGNORE INTO tombstones VALUES(?1,?2)",
            params![p, id],
        )
        .map_err(err)?;
        tx.execute(
            "DELETE FROM messages WHERE profile=?1 AND id=?2",
            params![p, id],
        )
        .map_err(err)?;
        tx.execute(
            "DELETE FROM message_fts WHERE profile=?1 AND id=?2",
            params![p, id],
        )
        .map_err(err)?;
        tx.commit().map_err(err)
    }
    pub fn drafts(&self, p: &str) -> Result<Vec<Draft>> {
        let mut s = self
            .db
            .prepare("SELECT document FROM drafts WHERE profile=?1 ORDER BY updated_at DESC")
            .map_err(err)?;
        let docs = s.query_map([p], |r| r.get::<_, String>(0)).map_err(err)?;
        docs.map(|r| serde_json::from_str(&r.map_err(err)?).map_err(err))
            .collect()
    }
    pub fn save_draft(&self, d: &Draft) -> Result<()> {
        valid_id(&d.id)?;
        let p = self.profile()?.id;
        if d.revision < 1
            || d.text.len() + d.html.len() + d.quoted_text.len() + d.quoted_html.len() > 2_000_000
            || d.attachments.len() > 100
        {
            return Err("Draft exceeds supported limits.".into());
        }
        let locked:bool=self.db.query_row("SELECT EXISTS(SELECT 1 FROM intents WHERE profile=?1 AND draft_id=?2 AND state!='cancelled')",params![p,d.id],|r|r.get(0)).map_err(err)?;
        if locked {
            return Err("This draft has an immutable send intent. Cancel the queued item or create a copy before editing.".into());
        }
        self.db.execute("INSERT INTO drafts VALUES(?1,?2,?3,?4,?5) ON CONFLICT(profile,id) DO UPDATE SET revision=excluded.revision,document=excluded.document,updated_at=excluded.updated_at WHERE excluded.revision>drafts.revision",params![d.id,p,d.revision,serde_json::to_string(d).map_err(err)?,now()]).map_err(err)?;
        Ok(())
    }
    pub fn delete_draft(&self, id: &str) -> Result<()> {
        let p = self.profile()?.id;
        self.db.execute("DELETE FROM drafts WHERE profile=?1 AND id=?2 AND NOT EXISTS(SELECT 1 FROM intents WHERE profile=?1 AND draft_id=?2 AND state NOT IN('cancelled','accepted'))",params![p,id]).map_err(err)?;
        Ok(())
    }
    pub fn enqueue(&mut self, d: &Draft, payload: &Value) -> Result<String> {
        let p = self.profile()?.id;
        if let Some(id) = self
            .db
            .query_row(
                "SELECT id FROM intents WHERE profile=?1 AND draft_id=?2 AND revision=?3",
                params![p, d.id, d.revision],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?
        {
            return Ok(id);
        }
        self.save_draft(d)?;
        let stored: i64 = self
            .db
            .query_row(
                "SELECT revision FROM drafts WHERE profile=?1 AND id=?2",
                params![p, d.id],
                |r| r.get(0),
            )
            .map_err(err)?;
        if stored != d.revision {
            return Err("The draft changed. Save and try again.".into());
        }
        let id = uuid();
        self.db.execute("INSERT INTO intents(id,profile,draft_id,revision,payload,state) VALUES(?1,?2,?3,?4,?5,'queued')",params![id,p,d.id,d.revision,payload.to_string()]).map_err(err)?;
        Ok(id)
    }
    pub fn intents(&self, p: &str) -> Result<Vec<Intent>> {
        let mut s=self.db.prepare("SELECT id,draft_id,payload,state,error,first_attempt,provider_id FROM intents WHERE profile=?1 AND state NOT IN('cancelled','accepted') ORDER BY rowid DESC").map_err(err)?;
        let rows = s
            .query_map([p], |r| {
                let v: Value = serde_json::from_str(&r.get::<_, String>(2)?).unwrap_or_default();
                Ok(Intent {
                    id: r.get(0)?,
                    draft_id: r.get(1)?,
                    subject: v["subject"].as_str().unwrap_or("").into(),
                    to: list_strings(&v["to"]),
                    state: r.get(3)?,
                    error: r.get(4)?,
                    first_attempt: r.get(5)?,
                    provider_id: r.get(6)?,
                })
            })
            .map_err(err)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)
    }
    pub fn cursor(&self, p: &str, direction: &str) -> Result<Option<String>> {
        self.db
            .query_row(
                "SELECT cursor FROM sync_state WHERE profile=?1 AND direction=?2",
                params![p, direction],
                |r| r.get(0),
            )
            .optional()
            .map(|x| x.flatten())
            .map_err(err)
    }
}
fn read_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    let doc: String = r.get(0)?;
    let mut m: Message = serde_json::from_str(&doc).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    m.folder = r.get(1)?;
    m.is_read = r.get(2)?;
    m.starred = r.get(3)?;
    m.labels = serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or_default();
    m.hydrated = r.get(5)?;
    Ok(m)
}
pub fn upsert(tx: &rusqlite::Transaction<'_>, p: &str, m: &Message) -> Result<()> {
    let previous: Option<String> = tx
        .query_row(
            "SELECT document FROM messages WHERE profile=?1 AND id=?2",
            params![p, m.id],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?;
    let mut merged = m.clone();
    if let Some(raw) = previous {
        if let Ok(old) = serde_json::from_str::<Message>(&raw) {
            if m.hydrated {
                // Provider detail responses may omit immutable fields that we already know.
                if merged.attachments.is_empty() {
                    merged.attachments = old.attachments;
                }
                if merged.message_id.is_empty() {
                    merged.message_id = old.message_id;
                }
                for (k, v) in old.headers {
                    merged.headers.entry(k).or_insert(v);
                }
                if merged.html.is_empty() && merged.text.is_empty() {
                    merged.html = old.html;
                    merged.text = old.text;
                }
            }
        }
    }
    let m = &merged;
    let mut document = serde_json::to_value(m).map_err(err)?;
    document["from_key"] = json!(address_key(&m.from));
    document["recipient_keys"] = json!(m
        .to
        .iter()
        .chain(m.cc.iter())
        .map(|s| address_key(s))
        .collect::<Vec<_>>());
    let removed: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM tombstones WHERE profile=?1 AND id=?2)",
            params![p, m.id],
            |r| r.get(0),
        )
        .map_err(err)?;
    if removed {
        return Ok(());
    }
    tx.execute("INSERT INTO messages(id,profile,direction,provider_id,document,folder,is_read,hydrated,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(profile,id) DO UPDATE SET document=CASE WHEN excluded.hydrated=1 OR messages.hydrated=0 THEN excluded.document ELSE json_set(messages.document,'$.status',json_extract(excluded.document,'$.status')) END,hydrated=MAX(messages.hydrated,excluded.hydrated)",params![m.id,p,m.direction,m.provider_id,document.to_string(),m.folder,m.is_read,m.hydrated,m.created_at]).map_err(err)?;
    let stored: String = tx
        .query_row(
            "SELECT document FROM messages WHERE profile=?1 AND id=?2",
            params![p, m.id],
            |r| r.get(0),
        )
        .map_err(err)?;
    let stored: Message = serde_json::from_str(&stored).map_err(err)?;
    let body = if !stored.text.is_empty() {
        stored.text.clone()
    } else {
        html_text(&stored.html)
    };
    let content = format!(
        "{} {} {} {} {}",
        stored.subject,
        stored.from,
        stored.to.join(" "),
        stored.cc.join(" "),
        body
    );
    tx.execute(
        "DELETE FROM message_fts WHERE profile=?1 AND id=?2",
        params![p, m.id],
    )
    .map_err(err)?;
    tx.execute(
        "INSERT INTO message_fts(profile,id,content) VALUES(?1,?2,?3)",
        params![p, m.id, content],
    )
    .map_err(err)?;
    Ok(())
}
pub fn html_text(s: &str) -> String {
    let tags = std::collections::HashSet::new();
    ammonia::Builder::new().tags(tags).clean(s).to_string()
}
fn like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}
fn search_terms(s: &str) -> Vec<String> {
    let mut out = vec![];
    let mut token = String::new();
    let mut quoted = false;
    for c in s.chars().take(2000) {
        if c == '"' {
            quoted = !quoted;
        } else if c.is_whitespace() && !quoted {
            if !token.is_empty() {
                out.push(std::mem::take(&mut token));
            }
        } else {
            token.push(c);
        }
    }
    if !token.is_empty() {
        out.push(token);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn store() -> Store {
        Store::open(Path::new(":memory:")).unwrap()
    }
    fn mail() -> Message {
        provider_message(json!({"id":uuid(),"from":"friend@example.com","to":["me@example.com"],"subject":"你好 project","text":"orchid draft","created_at":now()}),"incoming",true).unwrap()
    }
    #[test]
    fn workspace_switch_roundtrip_retains_data_and_checks_stale_writers() {
        let mut s = store();
        let a = s.profile().unwrap().id;
        s.rename_workspace(&a, "Studio").unwrap();
        let m = mail();
        s.hydrate(&a, &m).unwrap();
        let draft = Draft {
            id: uuid(),
            revision: 1,
            subject: "Private draft".into(),
            ..Default::default()
        };
        s.enqueue(&draft, &json!({"subject":"A send"})).unwrap();
        let b = s.new_workspace("Personal").unwrap();
        assert!(s.snapshot().unwrap().messages.is_empty());
        assert!(s.drafts(&b).unwrap().is_empty());
        assert!(s.intents(&b).unwrap().is_empty());
        assert!(s.require_active(&a).is_err());
        assert_eq!(s.workspaces().unwrap()[0].pending_sends, 1);
        // Even identical draft IDs are isolated.
        s.save_draft(&Draft {
            subject: "B draft".into(),
            ..draft.clone()
        })
        .unwrap();
        s.switch_workspace(&a).unwrap();
        assert_eq!(s.profile().unwrap().name, "Studio");
        assert_eq!(s.snapshot().unwrap().messages.len(), 1);
        assert_eq!(s.drafts(&a).unwrap()[0].subject, "Private draft");
        assert_eq!(s.drafts(&b).unwrap()[0].subject, "B draft");
        assert!(s.switch_workspace("missing").is_err());
        assert!(s.new_workspace("  ").is_err());
        assert_eq!(s.profile().unwrap().id, a);
    }
    #[test]
    fn v1_migration_preserves_existing_profiles_and_last_active_after_restart() {
        let path = std::env::temp_dir().join(format!("resend-migration-{}.sqlite", uuid()));
        let a = uuid();
        let b = uuid();
        {
            let db = Connection::open(&path).unwrap();
            db.execute_batch(include_str!("../migrations/001.sql"))
                .unwrap();
            db.execute("INSERT INTO profiles(id,active,connected,identities) VALUES(?1,1,1,'[]'),(?2,0,0,'[]')",params![a,b]).unwrap();
            db.execute("INSERT INTO sync_state(profile,direction,cursor) VALUES(?1,'incoming','old-cursor')",[&a]).unwrap();
        }
        {
            let mut s = Store::open(&path).unwrap();
            assert_eq!(s.workspaces().unwrap().len(), 2);
            assert!(s.profile().unwrap().connected);
            assert_eq!(s.profile().unwrap().id, a);
            assert_eq!(s.cursor(&a, "incoming").unwrap(), Some("old-cursor".into()));
            s.rename_workspace(&b, "Restored workspace").unwrap();
            s.switch_workspace(&b).unwrap();
            // The database enforces at most one active workspace.
            assert!(s
                .db
                .execute("UPDATE profiles SET active=1 WHERE id=?1", [&a])
                .is_err());
        }
        {
            let s = Store::open(&path).unwrap();
            assert_eq!(s.profile().unwrap().id, b);
            assert_eq!(s.profile().unwrap().name, "Restored workspace");
            assert_eq!(
                s.db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                    .unwrap(),
                2
            );
        }
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn replay_preserves_flags_and_tombstones() {
        let mut s = store();
        let p = s.profile().unwrap().id;
        let m = mail();
        s.upsert_page(
            &p,
            "incoming",
            std::slice::from_ref(&m),
            Some(&m.provider_id),
            false,
            true,
        )
        .unwrap();
        s.flags(&m.id, Some("archive".into()), Some(true), Some(true), None)
            .unwrap();
        s.upsert_page(&p, "incoming", std::slice::from_ref(&m), None, true, true)
            .unwrap();
        let actual = s.message(&p, &m.id).unwrap();
        assert!(actual.is_read && actual.starred);
        assert_eq!(actual.folder, "archive");
        s.flags(&m.id, Some("trash".into()), None, None, None)
            .unwrap();
        s.remove_message(&m.id).unwrap();
        s.upsert_page(&p, "incoming", std::slice::from_ref(&m), None, true, true)
            .unwrap();
        assert!(s.message(&p, &m.id).is_err());
    }
    #[test]
    fn composed_search_and_profile_isolation() {
        let mut s = store();
        let p = s.profile().unwrap().id;
        let m = mail();
        s.hydrate(&p, &m).unwrap();
        s.flags(&m.id, None, None, None, Some(vec!["work".into()]))
            .unwrap();
        let q = MessageQuery {
            search: "orchid from:friend label:work".into(),
            limit: 100,
            ..Default::default()
        };
        assert_eq!(s.list(&q).unwrap().len(), 1);
        assert!(s
            .list(&MessageQuery {
                search: "%x' OR 1=1".into(),
                limit: 100,
                ..Default::default()
            })
            .unwrap()
            .is_empty());
        s.new_profile().unwrap();
        assert!(s.list(&q).unwrap().is_empty());
    }
    #[test]
    fn stale_draft_cannot_overwrite_and_enqueue_is_unique() {
        let mut s = store();
        let mut d = Draft {
            id: uuid(),
            revision: 2,
            text: "new".into(),
            ..Default::default()
        };
        s.save_draft(&d).unwrap();
        d.revision = 1;
        d.text = "old".into();
        s.save_draft(&d).unwrap();
        let p = s.profile().unwrap().id;
        assert_eq!(s.drafts(&p).unwrap()[0].text, "new");
        d.revision = 2;
        let payload = json!({"subject":"example","to":["a@b.com"]});
        let a = s.enqueue(&d, &payload).unwrap();
        assert_eq!(a, s.enqueue(&d, &payload).unwrap());
        d.revision = 3;
        assert!(s.save_draft(&d).is_err());
    }
    #[test]
    fn submitting_recovers_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mail.db");
        let mut s = Store::open(&path).unwrap();
        let d = Draft {
            id: uuid(),
            revision: 1,
            ..Default::default()
        };
        let id = s.enqueue(&d, &json!({})).unwrap();
        s.db.execute("UPDATE intents SET state='submitting' WHERE id=?1", [id])
            .unwrap();
        drop(s);
        let s = Store::open(&path).unwrap();
        assert_eq!(
            s.intents(&s.profile().unwrap().id).unwrap()[0].state,
            "unknown"
        );
    }
    #[test]
    fn exact_identity_filter_uses_addresses_and_preserves_local_case() {
        let mut s = store();
        let p = s.profile().unwrap().id;
        let mut m = mail();
        m.from = "Not me <notme@example.com>".into();
        m.to = vec!["Me <Me@EXAMPLE.COM>".into()];
        s.hydrate(&p, &m).unwrap();
        let query = |identity: &str| MessageQuery {
            identity: identity.into(),
            limit: 10,
            ..Default::default()
        };
        assert_eq!(s.list(&query("Me@example.com")).unwrap().len(), 1);
        assert!(s.list(&query("me@example.com")).unwrap().is_empty());
    }
    #[test]
    fn scope_combines_folder_direction_and_domain_boundaries() {
        let mut s = store();
        let p = s.profile().unwrap().id;
        let mut incoming = mail();
        incoming.from = "me@email.sample.com".into();
        incoming.to = vec!["other@example.com".into()];
        incoming.bcc = vec!["Hidden <hidden@EMAIL.SAMPLE.COM>".into()];
        s.hydrate(&p, &incoming).unwrap();
        let mut outgoing = mail();
        outgoing.direction = "outgoing".into();
        outgoing.folder = "sent".into();
        outgoing.from = "Sender <me@apac.email.sample.com>".into();
        s.hydrate(&p, &outgoing).unwrap();
        let mut q = MessageQuery {
            folder: "inbox".into(),
            identity: "hidden@email.sample.com".into(),
            limit: 100,
            ..Default::default()
        };
        assert_eq!(s.list(&q).unwrap().len(), 1);
        q.identity = "me@email.sample.com".into();
        assert!(s.list(&q).unwrap().is_empty());
        q.identity.clear();
        q.folder = "sent".into();
        q.domain = "email.sample.com".into();
        assert!(s.list(&q).unwrap().is_empty());
        q.include_subdomains = true;
        assert_eq!(s.list(&q).unwrap().len(), 1);
        q.domain = "sample.com".into();
        q.include_subdomains = false;
        assert!(s.list(&q).unwrap().is_empty());
        q.domain = "apac.email.sample.com".into();
        assert_eq!(s.list(&q).unwrap().len(), 1);
        q.domain = "pac.email.sample.com".into();
        q.include_subdomains = true;
        assert!(s.list(&q).unwrap().is_empty());
    }
    #[test]
    fn detail_hydration_keeps_known_attachments_and_thread_headers() {
        let mut s = store();
        let p = s.profile().unwrap().id;
        let mut m = mail();
        m.headers
            .insert("in-reply-to".into(), "<parent@example.com>".into());
        m.attachments.push(Attachment {
            id: uuid(),
            filename: "note.txt".into(),
            ..Default::default()
        });
        s.hydrate(&p, &m).unwrap();
        let mut updated = m.clone();
        updated.headers.clear();
        updated.attachments.clear();
        updated.status = "delivered".into();
        s.hydrate(&p, &updated).unwrap();
        let read = s.message(&p, &m.id).unwrap();
        assert_eq!(read.attachments.len(), 1);
        assert_eq!(read.headers["in-reply-to"], "<parent@example.com>");
        assert_eq!(read.status, "delivered");
    }
}

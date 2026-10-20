use crate::{
    commands::{atomic_write, safe_filename},
    db,
    model::*,
    AppState,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::State;
use tauri_plugin_dialog::DialogExt;

#[derive(Serialize, Deserialize)]
struct BackupFile {
    id: String,
    filename: String,
    content_type: String,
    content: String,
}
#[derive(Serialize, Deserialize)]
struct BackupIntent {
    id: String,
    draft_id: String,
    revision: i64,
    payload: Value,
    state: String,
    first_attempt: Option<i64>,
    provider_id: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct BackupData {
    version: u32,
    identities: Vec<Identity>,
    settings: Value,
    messages: Vec<Message>,
    drafts: Vec<Draft>,
    intents: Vec<BackupIntent>,
    tombstones: Vec<String>,
    files: Vec<BackupFile>,
}
#[derive(Serialize, Deserialize)]
struct Envelope {
    format: String,
    sha256: String,
    data: String,
}
fn checksum(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

#[tauri::command]
pub async fn export_profile(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    workspace_id: String,
) -> Result<bool> {
    let _job = state.jobs.lock().await;
    let mut data = {
        let db = state.workspace_db(&workspace_id)?;
        let p = db.profile()?;
        let mut stmt = db
            .db
            .prepare("SELECT id FROM messages WHERE profile=?1")
            .map_err(|_| "Cannot snapshot mail.")?;
        let ids = stmt
            .query_map([&p.id], |r| r.get::<_, String>(0))
            .map_err(|_| "Cannot snapshot mail.")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| "Cannot snapshot mail.")?;
        let messages = ids
            .iter()
            .map(|id| db.message(&p.id, id))
            .collect::<Result<_>>()?;
        let mut stmt=db.db.prepare("SELECT id,draft_id,revision,payload,state,first_attempt,provider_id FROM intents WHERE profile=?1").map_err(|_|"Cannot snapshot Outbox.")?;
        let intents = stmt
            .query_map([&p.id], |r| {
                Ok(BackupIntent {
                    id: r.get(0)?,
                    draft_id: r.get(1)?,
                    revision: r.get(2)?,
                    payload: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or_default(),
                    state: r.get(4)?,
                    first_attempt: r.get(5)?,
                    provider_id: r.get(6)?,
                })
            })
            .map_err(|_| "Cannot snapshot Outbox.")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| "Cannot snapshot Outbox.")?;
        let mut stmt = db
            .db
            .prepare("SELECT id FROM tombstones WHERE profile=?1")
            .map_err(|_| "Cannot snapshot removal history.")?;
        let tombstones = stmt
            .query_map([&p.id], |r| r.get(0))
            .map_err(|_| "Cannot snapshot removal history.")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| "Cannot snapshot removal history.")?;
        BackupData {
            version: 1,
            identities: p.identities,
            settings: p.settings,
            messages,
            drafts: db.drafts(&p.id)?,
            intents,
            tombstones,
            files: vec![],
        }
    };
    let p = workspace_id;
    let referenced: std::collections::HashSet<String> = data
        .messages
        .iter()
        .flat_map(|m| m.attachments.iter().map(|a| a.id.clone()))
        .chain(
            data.drafts
                .iter()
                .flat_map(|d| d.attachments.iter().map(|a| a.id.clone())),
        )
        .collect();
    let files: Vec<(String, String, String)> = {
        let db = state.db()?;
        let mut stmt = db
            .db
            .prepare("SELECT id,filename,content_type FROM files WHERE profile=?1")
            .map_err(|_| "Cannot snapshot attachments.")?;
        let rows = stmt
            .query_map([&p], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|_| "Cannot snapshot attachments.")?;
        rows.collect::<std::result::Result<_, _>>()
            .map_err(|_| "Cannot snapshot attachments.")?
    };
    let mut size = 0;
    for (id, filename, content_type) in files {
        if !referenced.contains(&id) {
            continue;
        }
        let bytes = std::fs::read(state.file_path(&p, &id)?)
            .map_err(|_| "A referenced attachment is missing. Backup was not written.")?;
        size += bytes.len();
        if size > 250_000_000 {
            return Err("This preview supports backups up to 250 MB of attachment data.".into());
        }
        data.files.push(BackupFile {
            id,
            filename,
            content_type,
            content: STANDARD.encode(bytes),
        });
    }
    let raw = serde_json::to_string(&data).map_err(|_| "Cannot encode backup.")?;
    let envelope = Envelope {
        format: "resend-client-backup-v1".into(),
        sha256: checksum(raw.as_bytes()),
        data: raw,
    };
    let bytes = serde_json::to_vec(&envelope).map_err(|_| "Cannot encode backup.")?;
    let Some(path) = app
        .dialog()
        .file()
        .set_title("Export sensitive local mail backup (credentials excluded)")
        .set_file_name(format!(
            "resend-backup-{}.json",
            chrono::Utc::now().format("%Y-%m-%d")
        ))
        .blocking_save_file()
    else {
        return Ok(false);
    };
    atomic_write(
        &path
            .into_path()
            .map_err(|_| "Unsupported backup destination.")?,
        &bytes,
    )?;
    Ok(true)
}
fn decode(bytes: &[u8]) -> Result<BackupData> {
    if bytes.len() > 512_000_000 {
        return Err("Backup exceeds the 512 MB import limit.".into());
    }
    let envelope: Envelope =
        serde_json::from_slice(bytes).map_err(|_| "This is not a valid Resend Client backup.")?;
    if envelope.format != "resend-client-backup-v1"
        || checksum(envelope.data.as_bytes()) != envelope.sha256
    {
        return Err("Backup format or checksum validation failed. Nothing was imported.".into());
    }
    let data: BackupData =
        serde_json::from_str(&envelope.data).map_err(|_| "Backup contents are invalid.")?;
    if data.version != 1 {
        return Err("Unsupported backup version.".into());
    }
    for i in &data.identities {
        valid_id(&i.id)?;
        address(&i.email)?;
    }
    for m in &data.messages {
        valid_id(&m.provider_id)?;
        if !["incoming", "outgoing"].contains(&m.direction.as_str())
            || m.id != format!("{}:{}", m.direction, m.provider_id)
            || m.html.len() + m.text.len() > 4_000_000
        {
            return Err("Backup has invalid message data.".into());
        }
    }
    for d in &data.drafts {
        valid_id(&d.id)?;
        if d.html.len() + d.text.len() > 2_000_000 {
            return Err("Backup draft is too large.".into());
        }
    }
    let mut seen = std::collections::HashSet::new();
    for f in &data.files {
        valid_id(&f.id)?;
        if !seen.insert(&f.id) || f.content.len() > 54_000_000 {
            return Err("Backup has invalid attachment data.".into());
        }
        STANDARD
            .decode(&f.content)
            .map_err(|_| "Backup attachment data is corrupt.")?;
    }
    for d in &data.drafts {
        for a in &d.attachments {
            if !seen.contains(&a.id) {
                return Err("Backup is missing a draft attachment.".into());
            }
        }
    }
    for i in &data.intents {
        valid_id(&i.id)?;
        valid_id(&i.draft_id)?;
    }
    Ok(data)
}
#[tauri::command]
pub async fn restore_profile(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    workspace_id: String,
) -> Result<bool> {
    let _job = state.jobs.lock().await;
    state.db()?.require_active(&workspace_id)?;
    let Some(path) = app
        .dialog()
        .file()
        .set_title("Restore backup into a separate disconnected profile")
        .add_filter("Resend Client backup", &["json"])
        .blocking_pick_file()
    else {
        return Ok(false);
    };
    let path = path
        .into_path()
        .map_err(|_| "Unsupported backup location.")?;
    if std::fs::metadata(&path)
        .map_err(|_| "Cannot read backup.")?
        .len()
        > 512_000_000
    {
        return Err("Backup exceeds the 512 MB import limit.".into());
    }
    let bytes = std::fs::read(path).map_err(|_| "Cannot read backup.")?;
    let data = decode(&bytes)?;
    restore_data_for(&state, data, Some(&workspace_id))?;
    Ok(true)
}
#[cfg(test)]
fn restore_data(state: &AppState, data: BackupData) -> Result<()> {
    restore_data_for(state, data, None)
}
fn restore_data_for(
    state: &AppState,
    mut data: BackupData,
    expected_workspace: Option<&str>,
) -> Result<()> {
    let profile = uuid();
    for m in &mut data.messages {
        m.html = clean_html(&m.html);
    }
    for d in &mut data.drafts {
        d.html = clean_html(&d.html);
    }
    let dir = state.root.join("attachments").join(&profile);
    std::fs::create_dir_all(&dir).map_err(|_| "Cannot create restored attachment storage.")?;
    let result: Result<()> = (|| {
        for f in &data.files {
            let bytes = STANDARD
                .decode(&f.content)
                .map_err(|_| "Invalid attachment data.")?;
            atomic_write(&state.file_path(&profile, &f.id)?, &bytes)?;
        }
        let mut db = state.db()?;
        if let Some(expected) = expected_workspace {
            db.require_active(expected)?;
        }
        let tx = db.db.transaction().map_err(|_| "Cannot begin restore.")?;
        tx.execute(
            "INSERT INTO profiles(id,identities,settings) VALUES(?1,?2,?3)",
            params![
                profile,
                serde_json::to_string(&data.identities).unwrap(),
                data.settings.to_string()
            ],
        )
        .map_err(|_| "Cannot import profile.")?;
        for m in &data.messages {
            db::upsert(&tx, &profile, m)?;
            tx.execute("UPDATE messages SET folder=?1,is_read=?2,starred=?3,labels=?4 WHERE profile=?5 AND id=?6",params![m.folder,m.is_read,m.starred,serde_json::to_string(&m.labels).unwrap(),profile,m.id]).map_err(|_|"Cannot import local state.")?;
        }
        for d in &data.drafts {
            tx.execute(
                "INSERT INTO drafts VALUES(?1,?2,?3,?4,?5)",
                params![
                    d.id,
                    profile,
                    d.revision,
                    serde_json::to_string(d).unwrap(),
                    d.updated_at
                ],
            )
            .map_err(|_| "Cannot import drafts.")?;
        }
        for i in &data.intents {
            let status = if ["accepted", "cancelled"].contains(&i.state.as_str()) {
                i.state.as_str()
            } else {
                "needs_review"
            };
            tx.execute("INSERT INTO intents(id,profile,draft_id,revision,payload,state,first_attempt,provider_id,error) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![i.id,profile,i.draft_id,i.revision,i.payload.to_string(),status,i.first_attempt,i.provider_id,"Restored attempts require review. Check Resend before making a new send; a duplicate is possible."]).map_err(|_|"Cannot import send history.")?;
        }
        for id in &data.tombstones {
            tx.execute("INSERT INTO tombstones VALUES(?1,?2)", params![profile, id])
                .map_err(|_| "Cannot import removal history.")?;
        }
        for f in &data.files {
            let size = STANDARD.decode(&f.content).unwrap().len();
            tx.execute(
                "INSERT INTO files VALUES(?1,?2,?3,?4,?5)",
                params![
                    profile,
                    f.id,
                    safe_filename(&f.filename),
                    f.content_type,
                    size as i64
                ],
            )
            .map_err(|_| "Cannot import attachments.")?;
        }
        tx.execute("UPDATE profiles SET active=0", [])
            .map_err(|_| "Cannot activate restored profile.")?;
        tx.execute("UPDATE profiles SET active=1 WHERE id=?1", [&profile])
            .map_err(|_| "Cannot activate restored profile.")?;
        tx.commit().map_err(|_| {
            "Restore could not commit. Original profile remains active.".to_string()
        })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(dir);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> BackupData {
        let mut m=provider_message(serde_json::json!({"id":uuid(),"from":"test@example.com","to":["me@example.com"],"subject":"Restored","text":"Saved mail","html":"<p>Safe</p><script>bad()</script>","created_at":now()}),"incoming",true).unwrap();
        m.folder = "archive".into();
        m.starred = true;
        let draft = Draft {
            id: uuid(),
            revision: 1,
            text: "Recovered draft".into(),
            ..Default::default()
        };
        let intent = BackupIntent {
            id: uuid(),
            draft_id: draft.id.clone(),
            revision: 1,
            payload: serde_json::json!({"to":["test@example.com"]}),
            state: "queued".into(),
            first_attempt: None,
            provider_id: None,
        };
        BackupData {
            version: 1,
            identities: vec![],
            settings: serde_json::json!({"theme":"dark"}),
            messages: vec![m],
            drafts: vec![draft],
            intents: vec![intent],
            tombstones: vec![],
            files: vec![],
        }
    }
    #[test]
    fn restore_is_isolated_and_never_automatically_replays_sends() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState {
            store: std::sync::Mutex::new(
                crate::db::Store::open(&directory.path().join("mail.sqlite")).unwrap(),
            ),
            provider: crate::provider::Provider::new(),
            root: directory.path().to_path_buf(),
            jobs: tokio::sync::Mutex::new(()),
        };
        let old = state.db().unwrap().profile().unwrap().id;
        let data = fixture();
        let mail_id = data.messages[0].id.clone();
        let raw = serde_json::to_string(&data).unwrap();
        let envelope = Envelope {
            format: "resend-client-backup-v1".into(),
            sha256: checksum(raw.as_bytes()),
            data: raw,
        };
        restore_data(
            &state,
            decode(&serde_json::to_vec(&envelope).unwrap()).unwrap(),
        )
        .unwrap();
        let db = state.db().unwrap();
        let p = db.profile().unwrap();
        assert_ne!(old, p.id);
        assert!(!p.connected);
        assert!(db.message(&old, &mail_id).is_err());
        let m = db.message(&p.id, &mail_id).unwrap();
        assert!(m.starred);
        assert_eq!(m.folder, "archive");
        assert!(!m.html.contains("script"));
        assert_eq!(db.intents(&p.id).unwrap()[0].state, "needs_review");
        assert_eq!(db.drafts(&p.id).unwrap()[0].text, "Recovered draft");
    }
    #[test]
    fn failed_restore_keeps_original_profile_active() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState {
            store: std::sync::Mutex::new(
                crate::db::Store::open(&directory.path().join("mail.sqlite")).unwrap(),
            ),
            provider: crate::provider::Provider::new(),
            root: directory.path().to_path_buf(),
            jobs: tokio::sync::Mutex::new(()),
        };
        let original = state.db().unwrap().profile().unwrap().id;
        let mut data = fixture();
        data.drafts.push(data.drafts[0].clone());
        assert!(restore_data(&state, data).is_err());
        assert_eq!(state.db().unwrap().profile().unwrap().id, original);
    }
    #[test]
    fn checksum_and_format_are_required() {
        let bad = serde_json::to_vec(&Envelope {
            format: "resend-client-backup-v1".into(),
            sha256: "incorrect".into(),
            data: "{}".into(),
        })
        .unwrap();
        assert!(decode(&bad).is_err());
        assert!(decode(b"not json").is_err());
    }
    #[test]
    fn filename_cannot_escape() {
        assert_eq!(safe_filename("../../evil\\payload.exe"), "evilpayload.exe");
    }
}

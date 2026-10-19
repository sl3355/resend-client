use crate::{db, model::*, AppState};
use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::Method;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use tauri::State;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub async fn confirm_action(app: tauri::AppHandle, message: String) -> Result<bool> {
    if message.len() > 10_000 {
        return Err("Confirmation message is too long.".into());
    }
    Ok(app
        .dialog()
        .message(message)
        .title("Resend Client")
        .kind(tauri_plugin_dialog::MessageDialogKind::Warning)
        .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancel)
        .blocking_show())
}

#[tauri::command]
pub fn open_external(app: tauri::AppHandle, url: String) -> Result<()> {
    let parsed = reqwest::Url::parse(&url).map_err(|_| "Invalid link destination.")?;
    if !["https", "http"].contains(&parsed.scheme())
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || url.len() > 8192
    {
        return Err("Only explicit HTTP or HTTPS destinations can be opened.".into());
    }
    app.opener()
        .open_url(parsed.to_string(), None::<&str>)
        .map_err(|_| "Could not open the link in your browser.".into())
}

#[tauri::command]
pub fn get_snapshot(state: State<AppState>) -> Result<Snapshot> {
    state.db()?.snapshot()
}
#[tauri::command]
pub fn list_messages(state: State<AppState>, query: MessageQuery) -> Result<Vec<Message>> {
    state.db()?.list(&query)
}
#[tauri::command]
pub async fn connect_account(state: State<'_, AppState>, credential: String) -> Result<Probe> {
    let _job = state.jobs.lock().await;
    let p = state.db()?.profile()?;
    if p.connected {
        return Err(
            "Disconnect and create a separate profile before connecting another credential.".into(),
        );
    }
    let credential = credential.trim();
    if !credential.starts_with("re_")
        || credential.len() > 512
        || credential.contains(char::is_whitespace)
    {
        return Err("Enter a valid Resend API key beginning with re_.".into());
    }
    let incoming = state
        .provider
        .get(credential, "/emails/receiving?limit=1")
        .await;
    let outgoing = state.provider.get(credential, "/emails?limit=1").await;
    let probe = Probe {
        incoming: incoming.is_ok(),
        outgoing: outgoing.is_ok(),
        incoming_error: incoming.err(),
        outgoing_error: outgoing.err(),
    };
    if probe.incoming && probe.outgoing {
        keyring::Entry::new("com.resendclient.desktop",&p.id).and_then(|e|e.set_password(credential)).map_err(|_|"Secure storage is unavailable. Unlock your OS credential store and try again. The key was not saved to a file.")?;
        state
            .db()?
            .db
            .execute("UPDATE profiles SET connected=1 WHERE id=?1", [p.id])
            .map_err(|_| "Could not save connection.")?;
    }
    Ok(probe)
}
#[tauri::command]
pub async fn disconnect_account(state: State<'_, AppState>) -> Result<()> {
    let _job = state.jobs.lock().await;
    let p = state.db()?.profile()?;
    if p.connected {
        let entry = keyring::Entry::new("com.resendclient.desktop", &p.id)
            .map_err(|_| "Secure storage is unavailable.")?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(_) => {
                return Err(
                    "Could not remove the credential. Unlock secure storage and try again.".into(),
                )
            }
        }
    }
    let mut db = state.db()?;
    db.db
        .execute("UPDATE profiles SET connected=0 WHERE id=?1", [&p.id])
        .map_err(|_| "Could not disconnect.")?;
    db.new_profile()?;
    Ok(())
}
#[tauri::command]
pub fn save_settings(
    state: State<AppState>,
    identities: Vec<Identity>,
    settings: Value,
) -> Result<()> {
    // Only explicit product preferences are persisted; this never accepts secrets.
    let theme = settings["theme"].as_str().unwrap_or("system");
    if !["system", "light", "dark"].contains(&theme) {
        return Err("Invalid appearance setting.".into());
    }
    state
        .db()?
        .update_profile(&identities, &json!({"theme":theme}))
}
#[tauri::command]
pub async fn refresh_connection(state: State<'_, AppState>) -> Result<()> {
    sync(&state).await
}

pub async fn sync(state: &AppState) -> Result<()> {
    let _job = state.jobs.lock().await;
    let p = state.db()?.profile()?;
    if !p.connected {
        return Ok(());
    }
    let result = match state.secret(&p.id) {
        Ok(key) => sync_profile(state, &p, &key).await,
        Err(error) => Err(error),
    };
    state
        .db()?
        .db
        .execute(
            "UPDATE profiles SET sync_error=?1 WHERE id=?2",
            params![result.as_ref().err(), p.id],
        )
        .map_err(|_| "Could not save synchronization status.")?;
    result
}
async fn sync_profile(state: &AppState, p: &Profile, key: &str) -> Result<()> {
    let mut completed = 0;
    for direction in ["incoming", "outgoing"] {
        let base = if direction == "incoming" {
            "/emails/receiving"
        } else {
            "/emails"
        };
        // A fresh head scan runs independently of the durable historical cursor.
        let head = state
            .provider
            .get(key, &format!("{base}?limit=100"))
            .await?;
        let mut page = head;
        let existing = state.db()?.cursor(&p.id, direction)?;
        for index in 0..3 {
            let data = page["data"]
                .as_array()
                .ok_or("Resend returned an invalid message list.")?;
            let messages: Vec<Message> = data
                .iter()
                .map(|v| provider_message(v.clone(), direction, false))
                .collect::<Result<_>>()?;
            let more = page["has_more"].as_bool().unwrap_or(false);
            let cursor = if more {
                messages.last().map(|m| m.provider_id.clone())
            } else {
                None
            };
            if more && cursor.is_none() {
                return Err(
                    "Resend returned an empty page with more history. Retry refresh.".into(),
                );
            }
            let advance = index > 0 || existing.is_none();
            state.db()?.upsert_page(
                &p.id,
                direction,
                &messages,
                cursor.as_deref(),
                !more,
                advance,
            )?;
            if !more && (advance || existing.is_none()) {
                completed += 1;
                break;
            }
            if index == 2 {
                break;
            }
            let next = if index == 0 {
                existing.clone().or(cursor)
            } else {
                cursor
            };
            let Some(next) = next else {
                completed += 1;
                break;
            };
            valid_id(&next)?;
            page = match state
                .provider
                .get(key, &format!("{base}?limit=100&after={next}"))
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    // A replay from the head is safe if a stale cursor disappears.
                    state
                        .db()?
                        .db
                        .execute(
                            "UPDATE sync_state SET cursor=NULL WHERE profile=?1 AND direction=?2",
                            params![p.id, direction],
                        )
                        .map_err(|_| "Could not reset history cursor.")?;
                    return Err(e);
                }
            };
        }
    }
    // Record a completed scan separately from body coverage.
    if completed == 2 {
        state
            .db()?
            .db
            .execute(
                "UPDATE profiles SET last_sync=?1 WHERE id=?2",
                params![now(), p.id],
            )
            .map_err(|_| "Could not save refresh marker.")?;
    }
    // Bounded background hydration leaves capacity for user actions on the next tick.
    let pending: Vec<(String, String)> = {
        let db = state.db()?;
        let mut stmt=db.db.prepare("SELECT provider_id,direction FROM messages WHERE profile=?1 AND hydrated=0 ORDER BY created_at DESC LIMIT 8").map_err(|_|"Could not load download queue.")?;
        let rows = stmt
            .query_map([&p.id], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|_| "Could not load download queue.")?;
        rows.collect::<std::result::Result<_, _>>()
            .map_err(|_| "Could not load download queue.")?
    };
    for (id, direction) in pending {
        let path = if direction == "incoming" {
            format!("/emails/receiving/{id}?html_format=cid")
        } else {
            format!("/emails/{id}")
        };
        if let Ok(v) = state.provider.get(key, &path).await {
            if let Ok(m) = provider_message(v, &direction, true) {
                state.db()?.hydrate(&p.id, &m)?;
            }
        }
    }
    let recent: Vec<String> = {
        let db = state.db()?;
        let mut stmt=db.db.prepare("SELECT provider_id FROM messages WHERE profile=?1 AND direction='outgoing' AND hydrated=1 AND json_extract(document,'$.status') IN ('accepted','sent','delivery_delayed') ORDER BY created_at DESC LIMIT 5").map_err(|_|"Could not inspect delivery status.")?;
        let rows = stmt
            .query_map([&p.id], |r| r.get(0))
            .map_err(|_| "Could not inspect delivery status.")?;
        rows.collect::<std::result::Result<_, _>>()
            .map_err(|_| "Could not inspect delivery status.")?
    };
    for id in recent {
        if let Ok(v) = state.provider.get(key, &format!("/emails/{id}")).await {
            if let Ok(m) = provider_message(v, "outgoing", true) {
                state.db()?.hydrate(&p.id, &m)?;
            }
        }
    }
    Ok(())
}
#[tauri::command]
pub async fn read_message(state: State<'_, AppState>, id: String) -> Result<Message> {
    let p = state.db()?.profile()?;
    let mut m = state.db()?.message(&p.id, &id)?;
    if p.connected && (!m.hydrated || m.direction == "outgoing") {
        let key = state.secret(&p.id)?;
        let path = if m.direction == "incoming" {
            format!("/emails/receiving/{}?html_format=cid", m.provider_id)
        } else {
            format!("/emails/{}", m.provider_id)
        };
        match state.provider.get(&key, &path).await {
            Ok(v) => {
                m = provider_message(v, &m.direction, true)?;
                state.db()?.hydrate(&p.id, &m)?;
            }
            Err(e) if !m.hydrated => return Err(format!("Body is not downloaded. {e}")),
            Err(_) => {}
        }
    }
    if !m.hydrated {
        return Err("This body is not downloaded. Reconnect to download it.".into());
    }
    // Never return another profile's data if connection setup changed during the request.
    if state.db()?.profile()?.id != p.id {
        return Err("The active profile changed.".into());
    }
    state.db()?.flags(&id, None, Some(true), None, None)?;
    m = state.db()?.message(&p.id, &id)?;
    for a in &mut m.attachments {
        a.cached = state
            .file_path(&p.id, &a.id)
            .map(|p| p.is_file())
            .unwrap_or(false);
    }
    Ok(m)
}
#[tauri::command]
pub fn save_draft(state: State<AppState>, draft: Draft) -> Result<()> {
    state.db()?.save_draft(&draft)
}
#[tauri::command]
pub fn delete_draft(state: State<AppState>, id: String) -> Result<()> {
    state.db()?.delete_draft(&id)
}
#[tauri::command]
pub fn enqueue_send(state: State<AppState>, draft: Draft) -> Result<String> {
    let p = state.db()?.profile()?;
    if !p.connected {
        return Err("Connect your account before queueing mail.".into());
    }
    let mut payload = validate_draft(&draft, &p.identities)?;
    let mut attachments = vec![];
    let mut encoded_size = payload.to_string().len();
    for a in &draft.attachments {
        valid_id(&a.id)?;
        let known: bool = state
            .db()?
            .db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM files WHERE profile=?1 AND id=?2)",
                params![p.id, a.id],
                |r| r.get(0),
            )
            .map_err(|_| "Could not check attachment.")?;
        if !known {
            return Err("An attachment is no longer available.".into());
        }
        let bytes = std::fs::read(state.file_path(&p.id, &a.id)?)
            .map_err(|_| "An attachment is unavailable on this device.")?;
        encoded_size += bytes.len().div_ceil(3) * 4 + a.filename.len() + 200;
        if encoded_size > 39_000_000 {
            return Err(
                "Message exceeds the 40 MB encoded limit. Remove attachments and try again.".into(),
            );
        }
        attachments
            .push(json!({"filename":safe_filename(&a.filename),"content":STANDARD.encode(bytes)}));
    }
    payload["attachments"] = json!(attachments);
    state.db()?.enqueue(&draft, &payload)
}
pub async fn process_outbox(state: &AppState) -> Result<()> {
    let _job = state.jobs.lock().await;
    let p = state.db()?.profile()?;
    if !p.connected {
        return Ok(());
    }
    let has_work:bool=state.db()?.db.query_row("SELECT EXISTS(SELECT 1 FROM intents WHERE profile=?1 AND state IN('queued','unknown','submitting'))",[&p.id],|r|r.get(0)).map_err(|_|"Could not inspect Outbox.")?;
    if !has_work {
        return Ok(());
    }
    let key = state.secret(&p.id)?;
    submit_next(state, &p, &key).await
}
async fn submit_next(state: &AppState, p: &Profile, key: &str) -> Result<()> {
    state.db()?.db.execute("UPDATE intents SET state='unknown',error='The previous submission did not finish committing. Checking with the original attempt identity.' WHERE profile=?1 AND state='submitting'",[&p.id]).map_err(|_|"Could not recover the previous attempt.")?;
    let next: Option<(String, String, Option<i64>, String)> = {
        let db = state.db()?;
        db.db.query_row("SELECT id,payload,first_attempt,state FROM intents WHERE profile=?1 AND state IN('queued','unknown') AND next_attempt<=?2 ORDER BY rowid LIMIT 1",params![p.id,chrono::Utc::now().timestamp()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(|_|"Could not read Outbox.")?
    };
    let Some((id, raw, first, previous_state)) = next else {
        return Ok(());
    };
    let timestamp = chrono::Utc::now().timestamp();
    if first.is_some_and(|t| timestamp < t || timestamp - t > 23 * 3600) {
        state.db()?.db.execute("UPDATE intents SET state='needs_review',error='The safe retry window has expired. Check Resend before explicitly creating another send; a duplicate is possible.' WHERE profile=?1 AND id=?2",params![p.id,id]).map_err(|_|"Could not update Outbox.")?;
        return Ok(());
    }
    let payload: Value =
        serde_json::from_str(&raw).map_err(|_| "Stored send payload is invalid.")?;
    let claimed=state.db()?.db.execute("UPDATE intents SET state='submitting',first_attempt=COALESCE(first_attempt,?1),error=NULL WHERE profile=?2 AND id=?3 AND state IN('queued','unknown')",params![timestamp,p.id,id]).map_err(|_|"Could not commit send attempt. Nothing was submitted.")?;
    if claimed == 0 {
        return Ok(());
    }
    let response = state
        .provider
        .request(key, Method::POST, "/emails", Some(&payload), Some(&id))
        .await;
    match response {
        Ok(v) if v["id"].as_str().is_some() => {
            let provider_id = v["id"].as_str().unwrap();
            valid_id(provider_id)?;
            let mut doc = payload.clone();
            doc["id"] = json!(provider_id);
            doc["created_at"] = json!(now());
            doc["last_event"] = json!("accepted");
            doc["attachments"] = json!([]);
            // Persist attachment metadata, never the encoded request in the message document.
            let draft: Option<Draft> = {
                let db = state.db()?;
                let draft_id: String = db
                    .db
                    .query_row(
                        "SELECT draft_id FROM intents WHERE profile=?1 AND id=?2",
                        params![p.id, id],
                        |r| r.get(0),
                    )
                    .map_err(|_| "Could not read send context.")?;
                db.drafts(&p.id)?.into_iter().find(|d| d.id == draft_id)
            };
            let mut m = provider_message(doc, "outgoing", true)?;
            if let Some(d) = draft {
                m.attachments = d.attachments;
            }
            let mut db = state.db()?;
            let tx = db
                .db
                .transaction()
                .map_err(|_| "Could not begin acceptance commit.")?;
            db::upsert(&tx, &p.id, &m)?;
            tx.execute("UPDATE intents SET state='accepted',provider_id=?1,error=NULL WHERE profile=?2 AND id=?3",params![provider_id,p.id,id]).map_err(|_|"Could not save acceptance.")?;
            tx.execute("DELETE FROM drafts WHERE profile=?1 AND id=(SELECT draft_id FROM intents WHERE profile=?1 AND id=?2)",params![p.id,id]).map_err(|_|"Could not retire sent draft.")?;
            tx.commit().map_err(|_| {
                "Acceptance could not be committed. Restart to reconcile the original attempt."
                    .to_string()
            })?;
        }
        other => {
            let (status,message,delay)=match other {
                Err(e) if e.ambiguous=>("unknown",e.message,e.retry_after),
                Err(e) if previous_state=="unknown" && !e.retryable=>("needs_review",format!("The original outcome is still unknown. {} Check Resend before creating a new send.",e.message),e.retry_after),
                Err(e) if previous_state=="unknown"=>("unknown",e.message,e.retry_after),
                Err(e) if e.retryable=>("queued",e.message,e.retry_after),
                Err(e)=>("failed",e.message,3600),
                Ok(_)=>("unknown","Acceptance response did not include an email ID. The original attempt will be reconciled.".into(),60),
            };
            state.db()?.db.execute("UPDATE intents SET state=?1,error=?2,next_attempt=?3 WHERE profile=?4 AND id=?5",params![status,message,timestamp+delay as i64,p.id,id]).map_err(|_|"Could not record send outcome.")?;
        }
    }
    Ok(())
}
#[tauri::command]
pub fn cancel_intent(state: State<AppState>, id: String) -> Result<()> {
    let db = state.db()?;
    let p = db.profile()?.id;
    let changed=db.db.execute("UPDATE intents SET state='cancelled' WHERE profile=?1 AND id=?2 AND state IN('queued','failed') AND first_attempt IS NULL",params![p,id]).map_err(|_|"Could not cancel queued mail.")?;
    if changed == 0 {
        return Err(
            "This request has started. Cancellation cannot recall a submitted message.".into(),
        );
    }
    Ok(())
}
#[tauri::command]
pub fn retry_intent(state: State<AppState>, id: String) -> Result<()> {
    let db = state.db()?;
    let p = db.profile()?.id;
    let changed=db.db.execute("UPDATE intents SET state='queued',next_attempt=0,error=NULL WHERE profile=?1 AND id=?2 AND state IN('failed','unknown') AND (first_attempt IS NULL OR first_attempt>?3)",params![p,id,chrono::Utc::now().timestamp()-23*3600]).map_err(|_|"Could not resume attempt.")?;
    if changed == 0 {
        return Err("This attempt needs review. Check Resend before making a new draft; another send could duplicate the original.".into());
    }
    Ok(())
}
#[tauri::command]
pub fn set_local_flags(
    state: State<AppState>,
    id: String,
    folder: Option<String>,
    is_read: Option<bool>,
    starred: Option<bool>,
    labels: Option<Vec<String>>,
) -> Result<()> {
    state.db()?.flags(&id, folder, is_read, starred, labels)
}
#[tauri::command]
pub async fn make_intent_copy(state: State<'_, AppState>, id: String) -> Result<Draft> {
    let _job = state.jobs.lock().await;
    let db = state.db()?;
    let p = db.profile()?.id;
    let draft_id:String=db.db.query_row("SELECT draft_id FROM intents WHERE profile=?1 AND id=?2 AND state IN('failed','unknown','needs_review')",params![p,id],|r|r.get(0)).map_err(|_|"This send is no longer eligible to copy. Refresh Outbox first.")?;
    let mut draft = db
        .drafts(&p)?
        .into_iter()
        .find(|d| d.id == draft_id)
        .ok_or(
            "Original draft is no longer available. Check Sent before writing another message.",
        )?;
    db.db.execute("UPDATE intents SET state='needs_review',error='An explicit draft copy was created. This original attempt is paused; another send may duplicate it.' WHERE profile=?1 AND id=?2",params![p,id]).map_err(|_|"Could not pause the original attempt.")?;
    draft.id = uuid();
    draft.revision = 1;
    draft.updated_at = now();
    Ok(draft)
}
#[tauri::command]
pub fn permanently_remove(state: State<AppState>, id: String) -> Result<()> {
    state.db()?.remove_message(&id)
}

pub fn safe_filename(name: &str) -> String {
    let s: String = name
        .chars()
        .filter(|c| !c.is_control() && !['/', '\\', ':', '*', '?', '"', '<', '>', '|'].contains(c))
        .take(180)
        .collect();
    let s = s.trim().trim_matches('.');
    if s.is_empty() {
        "attachment".into()
    } else {
        s.to_string()
    }
}
pub fn atomic_write(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_file_name(format!(".resend-{}.tmp", uuid()));
    let result = (|| {
        use std::io::Write;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut file = opts
            .open(&tmp)
            .map_err(|_| "Could not create the file. Check free space and permissions.")?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| "Could not finish saving. Check free space.")?;
        std::fs::rename(&tmp, path)
            .map_err(|_| "Could not replace the destination file. Choose another name.")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(tmp);
    }
    result
}
#[tauri::command]
pub async fn attach_files(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<Attachment>> {
    let profile = state.db()?.profile()?.id;
    let Some(files) = app
        .dialog()
        .file()
        .set_title("Attach files")
        .blocking_pick_files()
    else {
        return Ok(vec![]);
    };
    if files.len() > 100 {
        return Err("Choose at most 100 attachments.".into());
    }
    let mut attachments = vec![];
    let mut total = 0_u64;
    for file in files {
        let path = file.into_path().map_err(|_| "Unsupported file location.")?;
        let size = std::fs::metadata(&path)
            .map_err(|_| "Cannot read attachment metadata.")?
            .len();
        total += size;
        if total > 29_000_000 {
            return Err("Selected files exceed the encoded message limit.".into());
        }
        let mut bytes = Vec::new();
        {
            use std::io::Read;
            std::fs::File::open(&path)
                .map_err(|_| "Could not read the selected attachment.")?
                .take(29_000_001)
                .read_to_end(&mut bytes)
                .map_err(|_| "Attachment read was interrupted.")?;
        }
        if bytes.len() > 29_000_000 {
            return Err("Selected attachment exceeds the encoded message limit.".into());
        }
        let id = uuid();
        let filename = safe_filename(&path.file_name().unwrap_or_default().to_string_lossy());
        let dst = state.file_path(&profile, &id)?;
        std::fs::create_dir_all(dst.parent().unwrap())
            .map_err(|_| "Could not create private attachment storage.")?;
        atomic_write(&dst, &bytes)?;
        state
            .db()?
            .db
            .execute(
                "INSERT INTO files VALUES(?1,?2,?3,?4,?5)",
                params![
                    profile,
                    id,
                    filename,
                    "application/octet-stream",
                    bytes.len() as i64
                ],
            )
            .map_err(|_| "Could not commit attachment.")?;
        attachments.push(Attachment {
            id,
            filename,
            content_type: "application/octet-stream".into(),
            size: bytes.len() as u64,
            cached: true,
        });
    }
    if state.db()?.profile()?.id != profile {
        return Err("The active profile changed. Attach the files again.".into());
    }
    Ok(attachments)
}
#[tauri::command]
pub async fn save_attachment(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    message_id: String,
    attachment_id: String,
) -> Result<bool> {
    let p = state.db()?.profile()?;
    let m = state.db()?.message(&p.id, &message_id)?;
    let a = m
        .attachments
        .iter()
        .find(|a| a.id == attachment_id)
        .ok_or("Attachment is not part of this message.")?;
    let cache = state.file_path(&p.id, &a.id)?;
    let bytes = if cache.is_file() {
        std::fs::read(&cache).map_err(|_| "Could not read cached attachment.")?
    } else {
        if !p.connected {
            return Err("This attachment is not downloaded. Reconnect first.".into());
        }
        valid_id(&m.provider_id)?;
        valid_id(&a.id)?;
        let base = if m.direction == "incoming" {
            "/emails/receiving"
        } else {
            "/emails"
        };
        let key = state.secret(&p.id)?;
        let metadata = state
            .provider
            .get(
                &key,
                &format!("{base}/{}/attachments/{}", m.provider_id, a.id),
            )
            .await?;
        let url = metadata["download_url"]
            .as_str()
            .ok_or("No attachment download is available.")?;
        let bytes = crate::provider::download(url).await?;
        std::fs::create_dir_all(cache.parent().unwrap())
            .map_err(|_| "Could not create attachment cache.")?;
        atomic_write(&cache, &bytes)?;
        state
            .db()?
            .db
            .execute(
                "INSERT OR REPLACE INTO files VALUES(?1,?2,?3,?4,?5)",
                params![p.id, a.id, a.filename, a.content_type, bytes.len() as i64],
            )
            .map_err(|_| "Could not register attachment.")?;
        bytes
    };
    if state.db()?.profile()?.id != p.id {
        return Err("The active profile changed.".into());
    }
    let Some(path) = app
        .dialog()
        .file()
        .set_file_name(safe_filename(&a.filename))
        .blocking_save_file()
    else {
        return Ok(false);
    };
    atomic_write(
        &path
            .into_path()
            .map_err(|_| "Unsupported save destination.")?,
        &bytes,
    )?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::Store,
        provider::Provider,
        test_server::{serve, Reply},
    };
    use std::{path::Path, sync::Mutex};
    fn state(base: String) -> AppState {
        AppState {
            store: Mutex::new(Store::open(Path::new(":memory:")).unwrap()),
            provider: Provider::mock(base),
            root: std::env::temp_dir().join(uuid()),
            jobs: tokio::sync::Mutex::new(()),
        }
    }
    fn queued(s: &AppState) -> (Profile, String) {
        let p = s.db().unwrap().profile().unwrap();
        let d = Draft {
            id: uuid(),
            revision: 1,
            ..Default::default()
        };
        let id=s.db().unwrap().enqueue(&d,&json!({"from":"me@example.com","to":["test@example.com"],"subject":"immutable","text":"body","html":"<p>body</p>"})).unwrap();
        (p, id)
    }
    #[tokio::test]
    async fn paginated_sync_replays_safely_and_discovers_new_arrivals() {
        let first = r#"{"id":"00000000-0000-4000-8000-000000000101","from":"sender@example.com","to":["me@example.com"],"subject":"First","created_at":"2026-10-19T12:01:00Z"}"#;
        let second = r#"{"id":"00000000-0000-4000-8000-000000000102","from":"me@example.com","to":["sender@example.com"],"subject":"Sent elsewhere","created_at":"2026-10-19T12:00:00Z","last_event":"delivered"}"#;
        let arrival = r#"{"id":"00000000-0000-4000-8000-000000000103","from":"new@example.com","to":["me@example.com"],"subject":"Arrived during import","created_at":"2026-10-19T12:02:00Z"}"#;
        let page = |data: &str, more: bool| -> &'static str {
            Box::leak(format!("{{\"has_more\":{more},\"data\":[{data}]}}").into_boxed_str())
        };
        let detail = |data: &str| -> &'static str { Box::leak(data.to_string().into_boxed_str()) };
        let (base, requests, thread) = serve(vec![
            Reply {
                status: 200,
                body: page(first, true),
                headers: "",
            },
            Reply {
                status: 200,
                body: page("", false),
                headers: "",
            },
            Reply {
                status: 200,
                body: page(second, false),
                headers: "",
            },
            Reply {
                status: 200,
                body: detail(first),
                headers: "",
            },
            Reply {
                status: 200,
                body: detail(second),
                headers: "",
            },
            Reply {
                status: 200,
                body: page(&format!("{arrival},{first}"), false),
                headers: "",
            },
            Reply {
                status: 200,
                body: page(second, false),
                headers: "",
            },
            Reply {
                status: 200,
                body: detail(arrival),
                headers: "",
            },
        ]);
        let s = state(base);
        let p = s.db().unwrap().profile().unwrap();
        sync_profile(&s, &p, "fake-test-key").await.unwrap();
        let id = "incoming:00000000-0000-4000-8000-000000000101";
        s.db()
            .unwrap()
            .flags(id, Some("archive".into()), Some(true), Some(true), None)
            .unwrap();
        sync_profile(&s, &p, "fake-test-key").await.unwrap();
        thread.join().unwrap();
        let db = s.db().unwrap();
        let snapshot = db.snapshot().unwrap();
        assert_eq!(snapshot.messages.len(), 3);
        assert_eq!(snapshot.pending_bodies, 0);
        assert!(snapshot.history_complete);
        assert!(snapshot.profile.last_sync.is_some());
        let original = db.message(&p.id, id).unwrap();
        assert!(original.starred && original.is_read);
        assert_eq!(original.folder, "archive");
        let req = requests.lock().unwrap();
        assert!(req[1].starts_with("GET /emails/receiving?limit=100&after="));
        assert!(req[5].starts_with("GET /emails/receiving?limit=100 "));
    }
    #[tokio::test]
    async fn ambiguous_acceptance_retries_same_payload_and_key() {
        let (base, requests, thread) = serve(vec![
            Reply {
                status: 0,
                body: "",
                headers: "",
            },
            Reply {
                status: 200,
                body: r#"{"id":"00000000-0000-4000-8000-000000000001"}"#,
                headers: "",
            },
        ]);
        let s = state(base);
        let (p, id) = queued(&s);
        submit_next(&s, &p, "fake-test-credential").await.unwrap();
        assert_eq!(s.db().unwrap().intents(&p.id).unwrap()[0].state, "unknown");
        s.db()
            .unwrap()
            .db
            .execute("UPDATE intents SET next_attempt=0 WHERE id=?1", [&id])
            .unwrap();
        submit_next(&s, &p, "fake-test-credential").await.unwrap();
        thread.join().unwrap();
        let req = requests.lock().unwrap();
        assert_eq!(req.len(), 2);
        for r in req.iter() {
            assert!(r
                .to_lowercase()
                .contains(&format!("idempotency-key: resend-client/{id}")));
        }
        assert_eq!(
            req[0].split("\r\n\r\n").nth(1),
            req[1].split("\r\n\r\n").nth(1)
        );
        assert!(s.db().unwrap().intents(&p.id).unwrap().is_empty());
        assert!(s.db().unwrap().drafts(&p.id).unwrap().is_empty());
        assert_eq!(
            s.db()
                .unwrap()
                .message(&p.id, "outgoing:00000000-0000-4000-8000-000000000001")
                .unwrap()
                .status,
            "accepted"
        );
    }
    #[tokio::test]
    async fn expired_and_future_attempt_clocks_require_review() {
        for first in [
            chrono::Utc::now().timestamp() - 24 * 3600,
            chrono::Utc::now().timestamp() + 60,
        ] {
            let s = state("http://127.0.0.1:1".into());
            let (p, id) = queued(&s);
            s.db()
                .unwrap()
                .db
                .execute(
                    "UPDATE intents SET state='unknown',first_attempt=?1 WHERE id=?2",
                    params![first, id],
                )
                .unwrap();
            submit_next(&s, &p, "fake").await.unwrap();
            assert_eq!(
                s.db().unwrap().intents(&p.id).unwrap()[0].state,
                "needs_review"
            );
        }
    }
    #[tokio::test]
    async fn definite_rejection_retains_draft_and_does_not_retry() {
        let (base, requests, thread) = serve(vec![Reply {
            status: 422,
            body: r#"{"name":"validation_error","message":"private recipient detail"}"#,
            headers: "",
        }]);
        let s = state(base);
        let (p, _) = queued(&s);
        submit_next(&s, &p, "fake").await.unwrap();
        submit_next(&s, &p, "fake").await.unwrap();
        thread.join().unwrap();
        let i = &s.db().unwrap().intents(&p.id).unwrap()[0];
        assert_eq!(i.state, "failed");
        assert!(!i.error.as_ref().unwrap().contains("private recipient"));
        assert_eq!(s.db().unwrap().drafts(&p.id).unwrap().len(), 1);
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
    #[tokio::test]
    async fn rate_limit_preserves_identity_and_obeys_server_timing() {
        let (base, _, thread) = serve(vec![Reply {
            status: 429,
            body: r#"{"name":"rate_limit_exceeded"}"#,
            headers: "Retry-After: 47\r\n",
        }]);
        let s = state(base);
        let (p, id) = queued(&s);
        let before = chrono::Utc::now().timestamp();
        submit_next(&s, &p, "fake").await.unwrap();
        thread.join().unwrap();
        let db = s.db().unwrap();
        let (state, next): (String, i64) = db
            .db
            .query_row(
                "SELECT state,next_attempt FROM intents WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(state, "queued");
        assert!(next >= before + 47);
    }
    #[tokio::test]
    async fn revoked_permission_does_not_turn_uncertain_acceptance_into_failure() {
        let (base, _, thread) = serve(vec![Reply {
            status: 401,
            body: r#"{"name":"unauthorized"}"#,
            headers: "",
        }]);
        let s = state(base);
        let (p, id) = queued(&s);
        s.db()
            .unwrap()
            .db
            .execute(
                "UPDATE intents SET state='unknown',first_attempt=?1 WHERE id=?2",
                params![chrono::Utc::now().timestamp(), id],
            )
            .unwrap();
        submit_next(&s, &p, "fake").await.unwrap();
        thread.join().unwrap();
        let i = &s.db().unwrap().intents(&p.id).unwrap()[0];
        assert_eq!(i.state, "needs_review");
        assert!(i.error.as_ref().unwrap().contains("still unknown"));
    }
}

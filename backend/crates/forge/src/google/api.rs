//! Google Calendar + Gmail REST access with a user access token.

use serde_json::{json, Value};

use crate::error::{Error, Result};

pub async fn calendar_events(access_token: &str, max_results: i64) -> Result<Value> {
    let http = reqwest::Client::new();
    let v: Value = http
        .get("https://www.googleapis.com/calendar/v3/calendars/primary/events")
        .query(&[
            ("maxResults", max_results.to_string()),
            ("orderBy", "startTime".to_string()),
            ("singleEvents", "true".to_string()),
            ("timeMin", chrono::Utc::now().to_rfc3339()),
        ])
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| Error::Internal(format!("google calendar: {e}")))?
        .json()
        .await
        .map_err(|e| Error::Internal(format!("google calendar body: {e}")))?;
    Ok(json!({
        "connected": true,
        "items": v["items"].as_array().cloned().unwrap_or_default().iter().map(|e| json!({
            "id": e["id"], "summary": e["summary"], "start": e["start"], "end": e["end"]
        })).collect::<Vec<_>>()
    }))
}

pub async fn gmail_send(access_token: &str, to: &str, subject: &str, body: &str) -> Result<Value> {
    if to.is_empty() || !to.contains('@') {
        return Err(Error::BadRequest("a valid `to` address is required".into()));
    }
    let raw = format!(
        "To: {to}\r\nSubject: {subject}\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\n{body}"
    );
    let http = reqwest::Client::new();
    let resp = http
        .post("https://gmail.googleapis.com/gmail/v1/users/me/messages/send")
        .bearer_auth(access_token)
        .json(&json!({ "raw": crate::google::oauth::b64url(raw.as_bytes()) }))
        .send()
        .await
        .map_err(|e| Error::Internal(format!("gmail send: {e}")))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(Error::Internal(format!("gmail send failed ({status}): {text}")));
    }
    Ok(json!({ "sent": true, "to": to, "subject": subject }))
}

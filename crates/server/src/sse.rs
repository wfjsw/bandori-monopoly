//! `GET /api/rooms/{id}/stream` -- one Server-Sent Events stream per player.
//!
//! Frames:
//!
//! | `event:`   | `id:`               | `data:` |
//! |------------|---------------------|---------|
//! | `hello`    |                     | `{you, room}` |
//! | `room`     |                     | `RoomInfo` (when it changes) |
//! | `event`    | `<matchId>:<eventId>` | `MatchEvent` |
//! | `match`    |                     | `{state, hand, handNotes, you, player_id}` (when it changes) |
//! | `dissolve` |                     | `{reason}` |
//!
//! Only `event` frames carry an id, so a browser's automatic reconnect sends the
//! last event it saw as `Last-Event-ID` and the stream resumes right after it. The
//! match id in front makes a new match start from its first event.

use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::{Stream, StreamExt};
use tokio::sync::{broadcast, mpsc};
use tokio_stream::wrappers::ReceiverStream;

use crate::api::{match_view, member_room};
use crate::auth::Auth;
use crate::error::ApiResult;
use crate::room::{MatchHandle, Note, Room};
use crate::state::Server;
use game_core::net::RoomInfo;

/// What this connection has already sent.
#[derive(Debug, Default)]
struct Cursor {
    match_id: i32,
    last_event: i32,
    last_seq: i32,
    last_room: String,
}

fn parse_last_event_id(headers: &HeaderMap, query: Option<&str>) -> (i32, i32) {
    let raw = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .or_else(|| {
            query?
                .split('&')
                .find_map(|kv| kv.strip_prefix("lastEventId="))
                .map(|v| v.replace("%3A", ":"))
        });
    raw.and_then(|v| {
        let (m, e) = v.split_once(':')?;
        Some((m.parse().ok()?, e.parse().ok()?))
    })
    .unwrap_or((0, 0))
}

/// Build the frames for one poll. `m` is the checked-out match, so this runs
/// with **no room lock held** -- the worker round-trips inside must not take it.
fn frames(info: &RoomInfo, m: Option<&MatchHandle>, member: i32, cur: &mut Cursor) -> Vec<Event> {
    let mut out = vec![];
    let room = serde_json::to_string(info).unwrap_or_default();
    if room != cur.last_room {
        out.push(Event::default().event("room").data(&room));
        cur.last_room = room;
    }
    if let Some(m) = m {
        // Two round-trips through the worker pool: this member's view, and the
        // events since the cursor.
        let Ok(view) = match_view(m, member) else {
            return out;
        };
        if view.state.match_id != cur.match_id {
            cur.match_id = view.state.match_id;
            cur.last_event = 0;
            cur.last_seq = -1;
        }
        for e in m.events(cur.last_event).unwrap_or_default() {
            let id = e.get("id").and_then(serde_json::Value::as_i64).unwrap_or(0) as i32;
            cur.last_event = id;
            let data = serde_json::to_string(&e).unwrap_or_default();
            out.push(
                Event::default()
                    .event("event")
                    .id(format!("{}:{}", cur.match_id, id))
                    .data(data),
            );
        }
        if view.state.seq != cur.last_seq {
            cur.last_seq = view.state.seq;
            out.push(
                Event::default()
                    .event("match")
                    .data(serde_json::to_string(&view).unwrap_or_default()),
            );
        }
    }
    out
}

pub async fn stream(
    State(s): State<Arc<Server>>,
    Auth(sess): Auth,
    Path(id): Path<String>,
    headers: HeaderMap,
    uri: axum::http::Uri,
) -> ApiResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let (room, member) = member_room(&s, &sess, &id)?;
    let (match_id, last_event) = parse_last_event_id(&headers, uri.query());
    let (rx, back) = {
        let mut r = room.lock().unwrap();
        let back = r.stream_opened(member);
        (r.tx.subscribe(), back)
    };
    if back {
        // Room lock dropped: `member_back` is a worker round-trip.
        if let Some(m) = room.lock().unwrap().match_handle() {
            if let Err(e) = m.member_back(member) {
                eprintln!("member_back failed: {e}");
            }
        }
        room.lock().unwrap().notify();
    }
    let (tx, out) = mpsc::channel::<Event>(256);
    tokio::spawn(pump(
        room,
        member,
        rx,
        tx,
        Cursor {
            match_id,
            last_event,
            last_seq: -1,
            last_room: String::new(),
        },
        id,
    ));
    Ok(Sse::new(ReceiverStream::new(out).map(Ok)).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    ))
}

async fn pump(
    room: Arc<Mutex<Room>>,
    member: i32,
    mut rx: broadcast::Receiver<Note>,
    tx: mpsc::Sender<Event>,
    mut cur: Cursor,
    id: String,
) {
    let hello = serde_json::json!({ "you": member, "room": id });
    if tx
        .send(Event::default().event("hello").data(hello.to_string()))
        .await
        .is_ok()
    {
        loop {
            // Check the match out and drop the room lock first: `frames` does
            // two worker round-trips and must hold neither the room lock nor a
            // runtime thread.
            let (info, m, dissolved) = {
                let r = room.lock().unwrap();
                (r.info.clone(), r.match_handle(), r.dissolved.clone())
            };
            let moved = std::mem::take(&mut cur);
            let (batch, back) = match tokio::task::spawn_blocking(move || {
                let mut c = moved;
                let b = frames(&info, m.as_deref(), member, &mut c);
                (b, c)
            })
            .await
            {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("sse frames failed: {e}");
                    break;
                }
            };
            cur = back;
            let mut gone = false;
            for f in batch {
                if tx.send(f).await.is_err() {
                    gone = true;
                    break;
                }
            }
            if gone {
                break;
            }
            if let Some(reason) = dissolved {
                let _ = tx
                    .send(
                        Event::default()
                            .event("dissolve")
                            .data(serde_json::json!({ "reason": reason }).to_string()),
                    )
                    .await;
                break;
            }
            tokio::select! {
                _ = tx.closed() => break,
                note = rx.recv() => match note {
                    Ok(Note::Changed) | Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Coalesce a burst of changes into one batch.
                        while rx.try_recv().is_ok() {}
                    }
                    Ok(Note::Dissolved(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => break,
                },
            }
        }
    }
    room.lock().unwrap().stream_closed(member);
}

use super::*;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};
use zbus::{
    blocking::{Connection, MessageIterator, Proxy},
    message::Type,
    zvariant::{OwnedFd, OwnedObjectPath, OwnedValue},
    MatchRule,
};

pub struct NativeComputerWatcher {
    connection: Connection,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<String>>,
}
fn platform(e: impl std::fmt::Display) -> WatcherError {
    WatcherError::Platform(format!("logind: {e}"))
}
fn inhibit(manager: &Proxy<'_>) -> Option<OwnedFd> {
    manager
        .call(
            "Inhibit",
            &(
                "sleep:shutdown",
                "Draugr Time",
                "Flush local activity before suspend/shutdown",
                "delay",
            ),
        )
        .ok()
}
impl ComputerWatcher for NativeComputerWatcher {
    fn start(callback: LifecycleCallback) -> Result<Self, WatcherError> {
        let connection = Connection::system().map_err(platform)?;
        let manager = Proxy::new(
            &connection,
            "org.freedesktop.login1",
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
        )
        .map_err(platform)?;
        // Subscribe before snapshot: PreparingFor* properties do not emit changes.
        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .sender("org.freedesktop.login1")
            .map_err(platform)?
            .path_namespace("/org/freedesktop/login1")
            .map_err(platform)?
            .build();
        let mut signals =
            MessageIterator::for_match_rule(rule, &connection, Some(128)).map_err(platform)?;
        let session: Option<OwnedObjectPath> = manager
            .call("GetSessionByPID", &std::process::id())
            .ok()
            .or_else(|| {
                let id = std::env::var("XDG_SESSION_ID").ok()?;
                let path: OwnedObjectPath = manager.call("GetSession", &id).ok()?;
                let proxy = Proxy::new(
                    &connection,
                    "org.freedesktop.login1",
                    path.as_str(),
                    "org.freedesktop.login1.Session",
                )
                .ok()?;
                let (owner, _): (u32, OwnedObjectPath) = proxy.get_property("User").ok()?;
                let status = std::fs::read_to_string("/proc/self/status").ok()?;
                let own: u32 = status
                    .lines()
                    .find(|line| line.starts_with("Uid:"))?
                    .split_whitespace()
                    .nth(2)?
                    .parse()
                    .ok()?;
                drop(proxy);
                (own == owner).then_some(path)
            });
        let session = session.and_then(|path| {
            let proxy = Proxy::new(
                &connection,
                "org.freedesktop.login1",
                path.as_str(),
                "org.freedesktop.login1.Session",
            )
            .ok()?;
            let kind: String = proxy.get_property("Type").ok()?;
            drop(proxy);
            ["x11", "wayland"].contains(&kind.as_str()).then_some(path)
        });
        let mut inhibitor = inhibit(&manager);
        let status=Arc::new(Mutex::new(format!("logind sleep/resume and shutdown intent; lock {}; pre-suspend flush {}; off/history reconstruction unavailable",if session.is_some(){"desktop LockedHint"}else{"unavailable (no own graphical session)"},if inhibitor.is_some(){"delay inhibitor"}else{"best effort: delay inhibitor denied"})));
        let emit = |kind| {
            callback(LifecycleEvent {
                kind,
                ts: timestamp(),
                source: "logind (notification receipt)".into(),
            })
        };
        if let Some(path) = &session {
            let proxy = Proxy::new(
                &connection,
                "org.freedesktop.login1",
                path.as_str(),
                "org.freedesktop.login1.Session",
            )
            .map_err(platform)?;
            if let Ok(locked) = proxy.get_property::<bool>("LockedHint") {
                emit(if locked {
                    LifecycleKind::Lock
                } else {
                    LifecycleKind::Unlock
                });
            }
        }
        if manager
            .get_property::<bool>("PreparingForSleep")
            .map_err(platform)?
        {
            emit(LifecycleKind::Sleep);
            inhibitor.take();
        }
        if manager
            .get_property::<bool>("PreparingForShutdown")
            .map_err(platform)?
        {
            emit(LifecycleKind::ShutdownRequested);
            inhibitor.take();
        }
        drop(manager);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let health = status.clone();
        let bus = connection.clone();
        std::thread::spawn(move || {
            let result = (|| -> Result<(), WatcherError> {
                let manager = Proxy::new(
                    &bus,
                    "org.freedesktop.login1",
                    "/org/freedesktop/login1",
                    "org.freedesktop.login1.Manager",
                )
                .map_err(platform)?;
                for message in &mut signals {
                    if stopping.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    let message = message.map_err(platform)?;
                    let header = message.header();
                    let member = header.member().map(|m| m.as_str()).unwrap_or("");
                    let path = header.path().map(|p| p.as_str()).unwrap_or("");
                    let kind = if path == "/org/freedesktop/login1"
                        && ["PrepareForSleep", "PrepareForShutdown"].contains(&member)
                    {
                        let (preparing,) =
                            message.body().deserialize::<(bool,)>().map_err(platform)?;
                        let kind = match (member, preparing) {
                            ("PrepareForSleep", true) => LifecycleKind::Sleep,
                            ("PrepareForSleep", false) => LifecycleKind::Wake,
                            (_, true) => LifecycleKind::ShutdownRequested,
                            (_, false) => LifecycleKind::ShutdownCancelled,
                        };
                        Some((kind, preparing))
                    } else if member == "PropertiesChanged"
                        && session.as_ref().is_some_and(|s| s.as_str() == path)
                    {
                        let (interface, changed, invalidated) = message
                            .body()
                            .deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
                            .map_err(platform)?;
                        if interface != "org.freedesktop.login1.Session" {
                            continue;
                        }
                        let locked = changed
                            .get("LockedHint")
                            .and_then(|v| bool::try_from(v).ok())
                            .or_else(|| {
                                if invalidated.iter().any(|s| s == "LockedHint") {
                                    Proxy::new(
                                        &bus,
                                        "org.freedesktop.login1",
                                        path,
                                        "org.freedesktop.login1.Session",
                                    )
                                    .ok()?
                                    .get_property::<bool>("LockedHint")
                                    .ok()
                                } else {
                                    None
                                }
                            });
                        locked.map(|v| {
                            (
                                if v {
                                    LifecycleKind::Lock
                                } else {
                                    LifecycleKind::Unlock
                                },
                                false,
                            )
                        })
                    } else {
                        None
                    };
                    if let Some((kind, preparing)) = kind {
                        callback(LifecycleEvent {
                            kind,
                            ts: timestamp(),
                            source: "logind (notification receipt)".into(),
                        });
                        if preparing {
                            inhibitor.take();
                        } else if matches!(
                            kind,
                            LifecycleKind::Wake | LifecycleKind::ShutdownCancelled
                        ) {
                            inhibitor = inhibit(&manager);
                            if inhibitor.is_none() {
                                *health.lock().unwrap()="logind connected; delay inhibitor denied, flush best effort; off/history unavailable".into();
                            }
                        }
                    }
                }
                Err(WatcherError::Platform("logind signal stream closed".into()))
            })();
            if !stopping.load(Ordering::Relaxed) {
                let reason = result
                    .err()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "logind disconnected".into());
                *health.lock().unwrap() = reason.clone();
                callback(LifecycleEvent {
                    kind: LifecycleKind::Unavailable,
                    ts: timestamp(),
                    source: reason,
                });
            }
        });
        Ok(Self {
            connection,
            stop,
            status,
        })
    }
    fn capabilities(&self) -> String {
        self.status.lock().unwrap().clone()
    }
}
impl Drop for NativeComputerWatcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.connection.clone().close();
    }
}

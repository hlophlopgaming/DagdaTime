use super::WatcherError;
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};
use wayland_client::{
    delegate_noop,
    globals::{registry_queue_init, GlobalListContents},
    protocol::{wl_registry, wl_seat},
    Connection, Dispatch, Proxy, QueueHandle,
};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1, ext_idle_notifier_v1,
};

type IdleState = Arc<Mutex<Result<Option<Instant>, String>>>;
pub struct IdleMonitor {
    state: IdleState,
}
struct State {
    idle: IdleState,
}
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
delegate_noop!(State: ignore wl_seat::WlSeat);
delegate_noop!(State: ignore ext_idle_notifier_v1::ExtIdleNotifierV1);
impl Dispatch<ext_idle_notification_v1::ExtIdleNotificationV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ext_idle_notification_v1::ExtIdleNotificationV1,
        event: ext_idle_notification_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_idle_notification_v1::Event::Idled => {
                *state.idle.lock().unwrap() = Ok(Some(Instant::now()))
            }
            ext_idle_notification_v1::Event::Resumed => *state.idle.lock().unwrap() = Ok(None),
            _ => {}
        }
    }
}
impl IdleMonitor {
    pub fn new() -> Result<Self, String> {
        let connection = Connection::connect_to_env().map_err(|error| error.to_string())?;
        let (globals, mut queue) =
            registry_queue_init::<State>(&connection).map_err(|error| error.to_string())?;
        let handle = queue.handle();
        let seat: wl_seat::WlSeat = globals
            .bind(&handle, 1..=1, ())
            .map_err(|error| error.to_string())?;
        let manager: ext_idle_notifier_v1::ExtIdleNotifierV1 = globals
            .bind(&handle, 1..=2, ())
            .map_err(|_| "compositor does not expose ext-idle-notify-v1".to_string())?;
        let notification = if manager.version() >= 2 {
            manager.get_input_idle_notification(1_000, &seat, &handle, ())
        } else {
            manager.get_idle_notification(1_000, &seat, &handle, ())
        };
        let state = Arc::new(Mutex::new(Ok(None)));
        let thread_state = state.clone();
        std::thread::spawn(move || {
            let _keep_alive = (seat, manager, notification);
            let mut state = State { idle: thread_state };
            loop {
                if let Err(error) = queue.blocking_dispatch(&mut state) {
                    *state.idle.lock().unwrap() = Err(error.to_string());
                    break;
                }
            }
        });
        Ok(Self { state })
    }
    pub fn seconds(&self) -> Result<u64, WatcherError> {
        match &*self.state.lock().unwrap() {
            Ok(Some(since)) => Ok(since.elapsed().as_secs() + 1),
            Ok(None) => Ok(0),
            Err(error) => Err(WatcherError::Platform(error.clone())),
        }
    }
}

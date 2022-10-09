use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::thread;

use mio::{Events, Interest, Poll, Token};
use parking_lot::RwLock;

const ID_DS4V2: &str = "000009CC";
const ID_SENSE: &str = "00000CE6";

#[derive(Debug, Clone, Copy)]
pub enum DSType {
    DS4BT,
    DS4USB,
    SenseBT,
    SenseUSB,
}

pub type Gamepads = Arc<RwLock<HashMap<String, DSGamepad>>>;

#[derive(Debug)]
pub struct DSGamepad {
    pub gamepad_type: DSType,
    pub hidraw_path: String,
    pub used_by: Option<SocketAddr>,
}

fn handle_event(event: udev::Event, gamepads: &Gamepads) {
    let sysname = String::from(event.sysname().to_str().unwrap());
    if event.event_type() == udev::EventType::Add {
        if let Some(gamepad) = filter_gamepads(event.device()) {
            gamepads.write().insert(sysname, gamepad);
            println!("Added {:?}", gamepads.read().keys());
        }
    } else if event.event_type() == udev::EventType::Remove {
        gamepads.write().remove(&sysname);
        println!("Removed {:?}", gamepads.read().keys());
    }
}

fn poll(gamepads: Gamepads) -> io::Result<()> {
    let builder = udev::MonitorBuilder::new()
        .unwrap()
        .match_subsystem("hidraw")
        .unwrap();
    let mut socket = builder.listen().unwrap();
    let mut poll = Poll::new()?;
    let mut events = Events::with_capacity(1024);

    poll.registry().register(
        &mut socket,
        Token(0),
        Interest::READABLE | Interest::WRITABLE,
    )?;

    loop {
        poll.poll(&mut events, None)?;
        for event in &events {
            if event.token() == Token(0) && event.is_writable() {
                socket.clone().for_each(|ev| handle_event(ev, &gamepads));
                println!("{:?}", gamepads);
            }
        }
    }
}

fn filter_gamepads(device: udev::Device) -> Option<DSGamepad> {
    let parent = device.parent_with_subsystem("hid").unwrap()?;
    let is_bt = device.parent_with_subsystem("bluetooth").unwrap().is_some();
    let hid_id = parent.property_value("HID_ID")?;
    let hid_id_str = hid_id.to_str()?;
    let ids: Vec<&str> = hid_id_str.split(':').collect();
    if ids[1] != "0000054C" {
        return None;
    }

    let devname = device.property_value("DEVNAME")?;
    let devname_str = devname.to_str()?;
    let hidraw_path = String::from(devname_str);
    let map = HashMap::from([
        (
            ID_DS4V2,
            HashMap::from([(true, DSType::DS4BT), (false, DSType::DS4USB)]),
        ),
        (
            ID_SENSE,
            HashMap::from([(true, DSType::SenseBT), (false, DSType::SenseUSB)]),
        ),
    ]);
    Some(DSGamepad {
        gamepad_type: map.get(ids[2])?[&is_bt],
        hidraw_path,
        used_by: None,
    })
}

pub fn start_monitor(gamepads: &Gamepads) {
    let gamepads = Arc::clone(gamepads);
    if let Err(err) = thread::Builder::new()
        .name(String::from("udev"))
        .spawn(move || poll(gamepads))
    {
        eprintln!("Error in creating thread for monitoring udev: {}", err);
        //stop.store(true, Ordering::SeqCst);
    }
}

pub mod identity {
    include!(concat!(env!("OUT_DIR"), "/nextim.identity.rs"));
}

pub mod message {
    include!(concat!(env!("OUT_DIR"), "/nextim.message.rs"));
}

pub mod group {
    include!(concat!(env!("OUT_DIR"), "/nextim.group.rs"));
}

pub mod transport {
    include!(concat!(env!("OUT_DIR"), "/nextim.transport.rs"));
}

pub mod identity {
    include!(concat!(env!("OUT_DIR"), "/lattice.identity.rs"));
}

pub mod message {
    include!(concat!(env!("OUT_DIR"), "/lattice.message.rs"));
}

pub mod group {
    include!(concat!(env!("OUT_DIR"), "/lattice.group.rs"));
}

pub mod transport {
    include!(concat!(env!("OUT_DIR"), "/lattice.transport.rs"));
}

pub mod discovery {
    include!(concat!(env!("OUT_DIR"), "/lattice.discovery.rs"));
}

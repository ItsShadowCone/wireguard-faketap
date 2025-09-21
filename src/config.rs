use std::env;

#[derive(Debug)]
pub struct Peer {
    pub public_key: String,
    pub endpoint: Option<String>,
}

#[derive(Debug)]
pub struct Config {
    pub interface_name: String,
    pub additional_setup: Option<String>,
    pub private_key: String,
    pub listen_addr: String,
    pub persistent_keepalive: Option<u16>,
    pub peer: Peer,
}

impl Config {
    pub fn from_env() -> Config {
        Config {
            interface_name: env::var("IFNAME").unwrap_or("vpn%d".to_string()),
            additional_setup: env::var("ADDITIONAL_SETUP").ok(),
            private_key: env::var("PRIVATE_KEY").expect("Error: PRIVATE_KEY not given"),
            listen_addr: env::var("LISTEN_ADDR").unwrap_or("0.0.0.0:51820".to_string()),
            persistent_keepalive: env::var("PERSISTENT_KEEPALIVE").ok()
                .map(|s| s.parse::<u16>().expect("Error: Couldn't parse PERSISTENT_KEEPALIVE"))
                .or(Some(25u16)),
            peer: Peer {
                public_key: env::var("PEER_PUBLIC").expect("Error: PEER_PUBLIC not given"),
                endpoint: env::var("PEER_ENDPOINT").ok(),
            },
        }
    }
}

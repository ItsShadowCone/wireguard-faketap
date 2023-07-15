mod config;
mod tap;

use tap::{Mode, Iface};
use std::{env, net::SocketAddr, sync::Arc, thread};
use std::error::Error;
use std::net::UdpSocket;
use std::ops::DerefMut;
use std::process::Command;
use std::sync::{Mutex};
use std::thread::sleep;
use std::time::Duration;
use base64::engine::general_purpose::STANDARD;
use base64::prelude::*;
use boringtun::noise::{
    errors::WireGuardError, Tunn, TunnResult,
};

use config::Config;

const BUFFER_SIZE: usize = 131072;


pub fn string_to_key<T>(data: String) -> Result<T, WireGuardError> where T: From<[u8; 32]>,
{
    STANDARD.decode(&data).ok()
        .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
        .map(T::from)
        .ok_or_else(|| WireGuardError::WrongKey)
}

fn cmd(cmd: &str, args: &[&str]) {
    let ecode = Command::new(cmd)
        .args(args)
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
    assert!(ecode.success(), "Failed to execute {}", cmd);
}

struct Tunnel {
    pub endpoint: Option<SocketAddr>,
    pub tun: Tunn,
}

impl Tunnel {
    fn new(endpoint: Option<SocketAddr>, tun: Tunn) -> Self {
        Self {
            endpoint,
            tun,
        }
    }
}


fn main() {
    let config = Config::from_env();

    println!("Using config: {:?}", config);

    let tap = Arc::from(Iface::without_packet_info(&config.interface_name, Mode::Tap).unwrap());
    cmd("ip", &["link", "set", "up", "dev", tap.name()]);
    if let Some(setup) = config.additional_setup {
        let mut it = setup.split_whitespace();
        let command = it.next().expect("Error: couldn't parse additional setup");
        let rest = it.collect::<Vec<&str>>();
        cmd(command, &rest);
    }

    println!(" ... interface: {}", tap.name());

    let socket = Arc::from(UdpSocket::bind(config.listen_addr).expect("Socket: bind failed"));
    let tunnel = Arc::new(Mutex::new(Tunnel::new(config.peer.endpoint, Tunn::new(string_to_key(config.private_key).unwrap(),
                                                                                 string_to_key(config.peer.public_key).unwrap(),
                                                                                 None, config.persistent_keepalive, 0, None).expect("Tunnel creation failed"))));

    let keepalive_socket = socket.clone();
    let keepalive_tunnel = tunnel.clone();
    thread::spawn(move || {
        loop {
            if let Err(e) = handle_keepalive(&keepalive_socket, &keepalive_tunnel) {
                println!("Error from keepalive: {:?}", e);
            }
        }
    });

    let thread_socket = socket.clone();
    let thread_tap = tap.clone();
    let thread_tunnel = tunnel.clone();
    thread::spawn(move || {
        loop {
            if let Err(e) = handle_tap(&thread_socket, &thread_tap, &thread_tunnel) {
                println!("Error from tap: {:?}", e);
            }
        }
    });

    loop {
        if let Err(e) = handle_socket(&socket, &tap, &tunnel) {
            println!("Error from socket: {:?}", e);
        }
    }
}

fn handle_keepalive(socket: &Arc<UdpSocket>, tunnel: &Arc<Mutex<Tunnel>>) -> Result<(), Box<dyn Error>> {
    let mut encapsulate_buffer = [0; BUFFER_SIZE - 14 + 32];
    loop {
        {
            let mut tunnel = tunnel.lock().unwrap();

            if let Some(addr) = tunnel.endpoint {
                match tunnel.tun.update_timers(&mut encapsulate_buffer) {
                    TunnResult::Done => (),
                    TunnResult::Err(error) => println!("Encapsulate error {:?}", error),
                    TunnResult::WriteToNetwork(send_buffer) => {
                        socket.send_to(send_buffer, addr.clone())?;
                    }
                    TunnResult::WriteToTunnelV4(_, _) => (),
                    TunnResult::WriteToTunnelV6(_, _) => (),
                }
            }
        }
        sleep(Duration::from_secs(1));
    }
}

fn handle_tap(socket: &Arc<UdpSocket>, tap: &Arc<Iface>, tunnel: &Arc<Mutex<Tunnel>>) -> Result<(), Box<dyn Error>> {
    let mut buf = [0; BUFFER_SIZE];
    let mut encapsulate_buffer = [0; BUFFER_SIZE - 14 + 32];
    loop {
        let len = tap.recv(&mut buf)?;
        //println!("Got from tap: {:02x?}", &buf[..len]);

        let mut tunnel = tunnel.lock().unwrap();
        if let Some(addr) = tunnel.endpoint {
            match tunnel.tun.encapsulate(&buf[14..len], &mut encapsulate_buffer) {
                TunnResult::Done => (),
                TunnResult::Err(error) => println!("Encapsulate error {:?}", error),
                TunnResult::WriteToNetwork(send_buffer) => {
                    socket.send_to(send_buffer, addr.clone())?;
                }
                TunnResult::WriteToTunnelV4(_, _) => (),
                TunnResult::WriteToTunnelV6(_, _) => (),
            }
        } else {
            println!("got no known endpoint, dropping packet");
        }
    }
}

fn handle_socket(socket: &Arc<UdpSocket>, tap: &Arc<Iface>, tunnel: &Arc<Mutex<Tunnel>>) -> Result<(), Box<dyn Error>> {
    let mut buf = [0; BUFFER_SIZE - 14 + 32];
    let mut decapsulate_buffer = [0; BUFFER_SIZE - 14];
    let mut send_buffer = [0; BUFFER_SIZE];
    loop {
        let (len, addr) = socket.recv_from(&mut buf)?;

        let mut tunnel = tunnel.lock().unwrap();
        if tunnel.endpoint.is_none() {
            tunnel.endpoint.replace(addr);
            println!("Endpoint not given at startup, using {:?} instead", addr);
        }

        let mut result = tunnel.tun.decapsulate(Some(addr.ip()), &buf[..len], &mut decapsulate_buffer);
        while let TunnResult::WriteToNetwork(b) = result {
            socket.send_to(b, tunnel.endpoint.unwrap())?;

            // check if there are more things to be handled
            result = tunnel.tun.decapsulate(Some(addr.ip()), &[0; 0], &mut decapsulate_buffer);
        }

        match result {
            TunnResult::Done => (),
            TunnResult::Err(error) => println!("Decapsulate error {:?}", error),
            TunnResult::WriteToNetwork(_) => unreachable!(),
            TunnResult::WriteToTunnelV4(buf, _) => {
                send_buffer[14..buf.len() + 14].copy_from_slice(buf);
                send_buffer[12] = 0x08;
                //println!("TAP Sending: {:02x?}", &send_buffer[ .. buf.len() + 14]);
                tap.send(&send_buffer[..buf.len() + 14])?;
            }
            TunnResult::WriteToTunnelV6(buf, _) => {
                send_buffer[14..buf.len() + 14].copy_from_slice(buf);
                send_buffer[12] = 0x86;
                send_buffer[13] = 0xDD;
                //println!("TAP Sending: {:02x?}", &send_buffer[ .. buf.len() + 14]);
                tap.send(&send_buffer[..buf.len() + 14])?;
            }
        }
    }
}

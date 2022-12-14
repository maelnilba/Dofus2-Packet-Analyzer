#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod lib;
use lib::{
    packet_capture::PacketCapture,
    packet_decoder::{DofusPacket, PacketDecoder},
};
use pcap::{Capture, Device};
use serde::Serialize;
use tauri::Manager;

#[derive(Clone, serde::Serialize)]
struct Payload {
    message: String,
}

#[derive(Serialize)]
struct ServerMessage {
    data: Vec<DofusPacket>,
}
impl ServerMessage {
    pub fn new(dp: Vec<DofusPacket>) -> ServerMessage {
        ServerMessage { data: dp }
    }
}

fn main() {
    env_logger::init();

    tauri::Builder::default()
        .setup(|app| {
            let id = app.listen_global("event-name", |event| {
                println!("got event-name with payload {:?}", event.payload());
            });

            app.unlisten(id);

            let device = Device::lookup()
                .expect("device lookup failed")
                .expect("no device available");

            let mut cap = Capture::from_device(device)
                .unwrap()
                .immediate_mode(true)
                .open()
                .unwrap();
            cap.filter("tcp port 5555", true).unwrap();
            let app_handle = app.handle();

            tauri::async_runtime::spawn(async move {
                loop {
                    let mut decoder = PacketDecoder::new();
                    let mut packet_capture = PacketCapture::new();
                    while let Ok(packet) = cap.next_packet() {
                        // parsed.remaining flush data so we lose the len value for the dofus decoder.
                        // still needed for know if this is client or server
                        let parsed = packet_capture.get_packet(&packet);
                        let (_src_addr, src_port, _dst_addr, _dst_port) =
                            packet_capture.get_packet_meta(&parsed);

                        // we remove the header from the data, slice at 54
                        let tcp_content = &packet.data[54..];

                        decoder.decode_packet(tcp_content, src_port.parse().unwrap_or_default());
                        let messages = decoder.get_messages();
                        let server_message = ServerMessage::new(messages);

                        rs2js(serde_json::to_string(&server_message).unwrap(), &app_handle);
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn rs2js<R: tauri::Runtime>(message: String, manager: &impl Manager<R>) {
    manager.emit_all("rs2js", message).unwrap();
}

use clap::Parser;
use std::{process::Command, time::Duration};

use arboard::Clipboard;
use device_query::{DeviceQuery, DeviceState, Keycode};
use enigo::{Enigo, Keyboard, Settings};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let send_addr = args.server.as_str();
    let recv_port = args.recv_port;

    // works for mac
    let command = Command::new("ipconfig")
        .arg("getifaddr")
        .arg("en0")
        .output()
        .unwrap()
        .stdout;

    let recv_addr = String::from_utf8(command).unwrap().replace('\n', "")
        + ":"
        + recv_port.to_string().as_str();
    println!("receive {:?}", recv_addr);
    println!("send {:?}", send_addr);

    let mut is_receiving = false;

    let mut enigo = Enigo::new(&Settings::default()).unwrap();
    let device_state = DeviceState::new();

    let mut clipboard = Clipboard::new().unwrap();
    let clip = clipboard.get_text();
    let mut prompt = if clip.is_ok() {
        clip.unwrap()
    } else {
        "".to_string()
    };
    println!("Clipboard text is: {:?}", prompt);

    loop {
        if is_receiving {
            let listener = TcpListener::bind(recv_addr.to_string()).await?;
            let (mut socket, _) = listener.accept().await?;
            let (mut rx, _) = socket.split();
            println!("receive");
            let mut empty = 0;
            'recv: loop {
                let mut buf = vec![];
                let r = rx.read_buf(&mut buf).await;

                let s = String::from_utf8(buf);
                // println!("{:?}", s);
                if s.clone().is_ok_and(|x| x == "") {
                    empty += 1;
                    if empty > 5 {
                        is_receiving = false;
                        break;
                    }
                }
                println!("{:?}", s);

                let _g = enigo.text(s.unwrap().as_str());
                // println!("{:?}", g);

                if r.is_ok_and(|x| x == 0) {
                    is_receiving = false;
                    break 'recv;
                }
            }
        } else {
            println!("waiting for hotkey");
            let mut pressed = false;
            while !pressed {
                tokio::time::sleep(Duration::from_millis(1)).await;
                let keys = device_state.get_keys();
                if keys.contains(&Keycode::LShift)
                    && keys.contains(&Keycode::LControl)
                    && (keys.contains(&Keycode::LOption) || keys.contains(&Keycode::LAlt))
                {
                    prompt = clipboard.get_text().unwrap();
                    println!("Clipboard text is: {:?}", prompt);
                    pressed = true;
                }
            }

            println!("send");
            if let Ok(mut stream) = TcpStream::connect(send_addr).await {
                let (_, mut tx) = stream.split();
                let r = tx
                    .write_buf(&mut prompt.clone().into_bytes().as_slice())
                    .await?;
                tx.flush().await?;
                println!("{:?}", r);

                is_receiving = true;
            } else {
                println!("failed to connect to server");
            }
        }
    }
}

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, short, default_value = "10.0.0.81:8080")]
    server: String,

    #[arg(long, short, default_value_t = 9191)]
    recv_port: usize,
}

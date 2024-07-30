use clap::Parser;
use std::{
    process::{Command, Stdio},
    sync::Arc,
};
use tokio::{
    io::AsyncReadExt,
    net::{TcpListener, TcpStream},
};

use log::info;

use either::Either;
use indexmap::IndexMap;
use tokio::sync::mpsc::channel;

use mistralrs::{
    Constraint, DefaultSchedulerMethod, Device, DeviceMapMetadata, GGUFLoaderBuilder,
    GGUFSpecificConfig, MistralRs, MistralRsBuilder, ModelDType, NormalRequest, Request,
    RequestMessage, Response, Result, SamplingParams, SchedulerConfig, TokenSource,
};

/// Gets the best device, cpu, cuda if compiled with CUDA
pub(crate) fn best_device() -> Result<Device> {
    #[cfg(not(feature = "metal"))]
    {
        Device::cuda_if_available(0)
    }
    #[cfg(feature = "metal")]
    {
        Device::new_metal(0)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let server_port = args.server_port;
    let client_port = args.client_port;

    let ipconfig = Command::new("ipconfig")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let grep = Command::new("grep")
        .arg("IPv4")
        .stdin(Stdio::from(ipconfig.stdout.unwrap()))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let ipv4 = grep.wait_with_output().unwrap().stdout;
    let ip = String::from_utf8(ipv4).unwrap().as_str()[39..]
        .replace('\n', "")
        .to_string();

    let mistralrs = setup("/Users/eric/Downloads/llama3-8b-instruct-q5_1.gguf")?;
    println!("setup complete");

    println!("listening on {:?}", ip);
    let recv_addr = ip + ":" + server_port.to_string().as_str();
    let mut send_addr: String;

    let mut prompt = "".to_string();
    loop {
        let listener = TcpListener::bind(recv_addr.clone()).await?;
        info!("listening on {:?} ", listener.local_addr());

        let (mut socket, _) = listener.accept().await?;
        let (mut rx, _) = socket.split();

        let mut buf = vec![];
        loop {
            let r = rx.read_buf(&mut buf).await;
            send_addr =
                rx.peer_addr().unwrap().ip().to_string() + ":" + client_port.to_string().as_str();
            println!("send back to {:?}", send_addr);
            println!("{:?}", buf);

            if r.is_ok_and(|x| x == 0) {
                break;
            }
        }
        if let Ok(s) = String::from_utf8(buf) {
            prompt = s;
        }
        let p = prompt.clone();
        println!("{:?}", prompt);

        let (tx, mut rx) = channel(256);

        let request = Request::Normal(NormalRequest {
            messages: RequestMessage::Chat(vec![IndexMap::from([
                ("role".to_string(), Either::Left("user".to_string())),
                ("content".to_string(), Either::Left(p.to_string())),
            ])]),
            sampling_params: SamplingParams {
                temperature: Some(0.3),
                frequency_penalty: Some(2.2),
                max_len: Some(500),
                ..Default::default()
            },
            response: tx,
            return_logprobs: false,
            is_streaming: true,
            id: 0,
            constraint: Constraint::None,
            suffix: None,
            adapters: None,
        });
        mistralrs.get_sender()?.send(request).await?;

        let mut stream = TcpStream::connect(send_addr).await.unwrap();
        let (_, clienttx) = stream.split();

        while let Some(response) = rx.recv().await {
            match response {
                Response::Chunk(t) => {
                    clienttx.try_write(t.choices[0].delta.content.as_bytes())?;
                }
                Response::Done(c) => println!(
                    "Text: {}, Prompt T/s: {}, Completion T/s: {}",
                    c.choices[0].message.content,
                    c.usage.avg_prompt_tok_per_sec,
                    c.usage.avg_compl_tok_per_sec
                ),
                Response::InternalError(e) => panic!("Internal error: {e}"),
                Response::ValidationError(e) => panic!("Validation error: {e}"),
                Response::ModelError(e, c) => panic!(
                    "Model error: {e}. Response: Text: {}, Prompt T/s: {}, Completion T/s: {}",
                    c.choices[0].message.content,
                    c.usage.avg_prompt_tok_per_sec,
                    c.usage.avg_compl_tok_per_sec
                ),
                _ => unreachable!(),
            }
        }
    }
}

fn setup(model_path: &str) -> anyhow::Result<Arc<MistralRs>> {
    // Select a Mistral model
    // We do not use any files from HF servers here, and instead load the
    // chat template from the specified file, and the tokenizer and model from a
    // local GGUF file at the path `.`
    let loader = GGUFLoaderBuilder::new(
        GGUFSpecificConfig { repeat_last_n: 64 },
        Some("./mistral.json".to_string()),
        None,
        ".".to_string(),
        model_path.to_string(),
    )
    .build();

    // Load, into a Pipeline
    let pipeline = loader.load_model_from_hf(
        None,
        TokenSource::CacheToken,
        &ModelDType::Auto,
        &best_device()?,
        false,
        DeviceMapMetadata::dummy(),
        None,
        None, // No PagedAttention.
    )?;
    // Create the MistralRs, which is a runner
    Ok(MistralRsBuilder::new(
        pipeline,
        SchedulerConfig::DefaultScheduler {
            method: DefaultSchedulerMethod::Fixed(5.try_into().unwrap()),
        },
    )
    .build())
}

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, short, default_value_t = 9191)]
    client_port: usize,

    #[arg(long, short, default_value_t = 8080)]
    server_port: usize,
}

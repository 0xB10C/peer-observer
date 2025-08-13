// This file includes code from rants (https://github.com/davidMcneil/rants/)
// Copyright (c) 2021 David McNeil and contributors
// Licensed under the MIT License: https://opensource.org/licenses/MIT
//
// Modifications to
// https://github.com/davidMcneil/rants/blob/beb187b49bd6ade187cb5b92023cb67ba0e15baa/tests/common.rs:
// - Renamed NATS_PATH_ENV to ENV_NATS_SERVER_BINARY
// - Renamed NatsServer to NatsServerForTesting
// - Changed the function new() to take a port as argument and hardcode all other nats-server args

use std::{env, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::oneshot::{self, Sender},
    time::timeout,
};

const ENV_NATS_SERVER_BINARY: &str = "NATS_SERVER_BINARY";
const NATS_READY_MESSAGE: &str = "Server is ready";

pub struct NatsServerForTesting {
    kill: Option<Sender<()>>,
}

impl NatsServerForTesting {
    pub async fn new(port: u16) -> Self {
        let nats_server_binary_path: String = match env::var(ENV_NATS_SERVER_BINARY) {
            Ok(b) => b,
            Err(e) => {
                panic!(
                "Set the {} environment variable to the location of your nats-server binary run the integration tests: {}",
                ENV_NATS_SERVER_BINARY, e
            );
            }
        };

        let args = [&format!("--port={}", port), "--addr=127.0.0.1"];

        println!(
            "Starting NATS server with: {} {}",
            nats_server_binary_path,
            args.join(" ")
        );

        let mut child = Command::new(nats_server_binary_path.clone())
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect(&format!(
                "Failed to start nats-server with binary='{}' and args='{}'",
                nats_server_binary_path,
                args.join(" ")
            ));

        // Spawn a task to handle stdout
        let stdout = child
            .stdout
            .take()
            .expect("child did not have a handle to stdout");
        tokio::spawn(async {
            let mut reader = BufReader::new(stdout).lines();
            while let Some(line) = reader.next_line().await.expect("valid stdout line") {
                println!("{}", line);
            }
        });

        // Spawn a task to handle stderr and check if nats is ready
        let (ready_tx, ready_rx) = oneshot::channel::<()>();
        let stderr = child
            .stderr
            .take()
            .expect("child did not have a handle to stdout");
        tokio::spawn(async {
            let mut ready_tx = Some(ready_tx);
            let mut reader = BufReader::new(stderr).lines();
            while let Some(line) = reader.next_line().await.expect("valid stdout line") {
                println!("{}", line);
                if line.contains(NATS_READY_MESSAGE) {
                    if let Some(ready_tx) = ready_tx.take() {
                        ready_tx.send(()).expect("to send nats ready oneshot");
                    }
                }
            }
        });

        // Spawn a task to run the child and wait for the kill oneshot
        let (kill_tx, kill_rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            tokio::select! {
                exit = child.wait()  => {
                    if let Err(_) = exit {
                        panic!("NATS produced Err while running");
                    } else {
                        panic!("NATS exited early");
                    }
                }
                rx = kill_rx => {
                    if let Err(_) = rx {
                        panic!("failed to receive ready oneshot");
                    } else {
                        ();
                    }
                }
            }
        });

        // Wait for nats to be ready or timeout
        if let Err(_) = timeout(Duration::from_secs(5), ready_rx).await {
            panic!("NATS server failed to reach ready state within timeout");
        }

        Self {
            kill: Some(kill_tx),
        }
    }
}

impl Drop for NatsServerForTesting {
    fn drop(&mut self) {
        if let Some(kill) = self.kill.take() {
            kill.send(()).expect("to send kill oneshot")
        }
    }
}

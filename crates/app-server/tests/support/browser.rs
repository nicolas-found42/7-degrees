#![allow(dead_code)]
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};
pub struct Browser {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}
impl Browser {
    pub fn start() -> Self {
        let home = std::env::var("HOME").unwrap();
        let node = std::env::var("BROWSER_NODE").unwrap_or_else(|_| {
            format!("{home}/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin/node")
        });
        let driver =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/browser_driver.cjs");
        let mut child = Command::new(node)
            .arg(driver)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("launch installed Node/Playwright");
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            input,
            output,
        }
    }
    pub fn call(&mut self, command: Value) -> Value {
        writeln!(self.input, "{command}").unwrap();
        self.input.flush().unwrap();
        let mut line = String::new();
        self.output.read_line(&mut line).unwrap();
        let result: Value = serde_json::from_str(&line).expect("browser RPC response");
        assert_eq!(result["ok"], true, "{command}: {result}");
        result["result"].clone()
    }
    pub fn attr(&mut self, name: &str) -> String {
        self.call(json!({"op":"attribute","selector":"#canvas-status","name":name}))
            .as_str()
            .unwrap()
            .into()
    }
    pub fn click(&mut self, name: &str) {
        self.call(json!({"op":"click","role":"button","name":name}));
    }
    pub fn text(&mut self, selector: &str) -> String {
        self.call(json!({"op":"text","selector":selector}))
            .as_str()
            .unwrap()
            .into()
    }
    pub fn ready(&mut self) {
        self.call(json!({"op":"ready","selector":"#canvas-status[data-ready=true]"}));
    }
}
impl Drop for Browser {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct Server {
    pub url: String,
    task: tokio::task::JoinHandle<()>,
}
impl Server {
    pub async fn start(app: axum::Router) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self { url, task }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

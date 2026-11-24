//! 验收专用限速 Range 服务；暂停传输由测试控制，缓冲状态必须来自播放器。
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub struct Server {
    pub url: String,
    pub blocked: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Server {
    pub fn new(bytes: Vec<u8>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/quality.mp4", listener.local_addr().unwrap());
        let blocked = Arc::new(AtomicBool::new(false));
        let stopped = Arc::new(AtomicBool::new(false));
        let block = Arc::clone(&blocked);
        let stop = Arc::clone(&stopped);
        let bytes = Arc::new(bytes);
        let worker = thread::spawn(move || {
            let mut clients = Vec::new();
            while !stop.load(Ordering::Acquire) {
                if let Ok((stream, _)) = listener.accept() {
                    let bytes = Arc::clone(&bytes);
                    let block = Arc::clone(&block);
                    let stop = Arc::clone(&stop);
                    clients.push(thread::spawn(move || {
                        let _ = serve(stream, &bytes, &block, &stop);
                    }));
                } else {
                    thread::sleep(Duration::from_millis(10));
                }
            }
            for client in clients {
                client.join().unwrap();
            }
        });
        Self {
            url,
            blocked,
            stopped,
            worker: Some(worker),
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        self.worker.take().unwrap().join().unwrap();
    }
}
fn serve(
    mut stream: TcpStream,
    bytes: &[u8],
    blocked: &AtomicBool,
    stopped: &AtomicBool,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut header = Vec::new();
    while header.len() < 16384 && !header.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        if stream.read(&mut byte)? == 0 {
            return Ok(());
        }
        header.push(byte[0]);
    }
    let text = String::from_utf8_lossy(&header);
    let range = text.lines().find_map(|line| {
        line.to_ascii_lowercase()
            .strip_prefix("range: bytes=")
            .map(str::to_owned)
    });
    let (start, end) =
        range
            .as_deref()
            .and_then(|s| s.split_once('-'))
            .map_or((0, bytes.len() - 1), |(a, b)| {
                (
                    a.parse::<usize>().unwrap(),
                    b.trim().parse::<usize>().unwrap_or(bytes.len() - 1),
                )
            });
    if start > end || end >= bytes.len() {
        return Ok(());
    }
    let status = if range.is_some() {
        "206 Partial Content"
    } else {
        "200 OK"
    };
    write!(
        stream,
        "HTTP/1.1 {status}\r\nAccept-Ranges: bytes\r\nContent-Type: video/mp4\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{end}/{}\r\nConnection: close\r\n\r\n",
        end - start + 1,
        bytes.len()
    )?;
    for chunk in bytes[start..=end].chunks(16384) {
        while blocked.load(Ordering::Acquire) && !stopped.load(Ordering::Acquire) {
            thread::sleep(Duration::from_millis(20));
        }
        if stopped.load(Ordering::Acquire) {
            break;
        }
        stream.write_all(chunk)?;
        thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

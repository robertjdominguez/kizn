//! Server-sent events, just enough for streamed completions: split the byte
//! stream into events and hand back each `data:` payload.

use futures_util::{Stream, StreamExt};

/// `data:` payloads from an SSE body, in order. Multi-line data is joined
/// with newlines; comments and other fields are skipped.
pub fn data_events(
    body: impl Stream<Item = reqwest::Result<bytes::Bytes>> + Unpin,
) -> impl Stream<Item = Result<String, reqwest::Error>> {
    futures_util::stream::unfold((body, String::new(), Vec::<String>::new()), |(mut body, mut buf, mut ready)| async move {
        loop {
            if let Some(ev) = ready.pop() {
                return Some((Ok(ev), (body, buf, ready)));
            }
            match body.next().await {
                Some(Ok(chunk)) => {
                    buf.push_str(&String::from_utf8_lossy(&chunk).replace("\r\n", "\n"));
                    let mut found = vec![];
                    while let Some(end) = buf.find("\n\n") {
                        let block: String = buf.drain(..end + 2).collect();
                        let data: Vec<&str> = block
                            .lines()
                            .filter_map(|l| l.strip_prefix("data:"))
                            .map(|d| d.strip_prefix(' ').unwrap_or(d))
                            .collect();
                        if !data.is_empty() {
                            found.push(data.join("\n"));
                        }
                    }
                    found.reverse();
                    ready = found;
                }
                Some(Err(e)) => return Some((Err(e), (body, buf, ready))),
                None => return None,
            }
        }
    })
}

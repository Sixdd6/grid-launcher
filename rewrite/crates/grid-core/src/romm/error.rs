/// Errors are user-presentable and MUST NEVER embed the request or its
/// headers. Body excerpts are capped and come from the server response only.
#[derive(Debug, thiserror::Error)]
pub enum RommError {
    #[error("invalid server URL")]
    InvalidUrl,
    #[error("could not reach the server: {0}")]
    Connection(String),
    #[error("the server rejected the credentials")]
    Unauthorized,
    #[error("server error {status}: {excerpt}")]
    Http { status: u16, excerpt: String },
    #[error("unexpected response from the server: {0}")]
    Decode(String),
}

pub(crate) fn excerpt(body: &str) -> String {
    const MAX: usize = 240;
    let collapsed: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.len() > MAX {
        // Walk back to a char boundary so we never split a multi-byte UTF-8
        // sequence — the body is server-controlled input.
        let mut end = MAX;
        while end > 0 && !collapsed.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &collapsed[..end])
    } else {
        collapsed
    }
}

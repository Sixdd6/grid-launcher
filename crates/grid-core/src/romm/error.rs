/// Errors are user-presentable and MUST NEVER embed the request or its
/// headers. Body excerpts are capped and come from the server response only.
#[derive(Debug, Clone, thiserror::Error)]
pub enum RommError {
    #[error("invalid server URL")]
    InvalidUrl,
    #[error("could not reach the server: {0}")]
    Connection(String),
    #[error("the server rejected the credentials")]
    Unauthorized,
    #[error("server error {status}{}", http_excerpt_suffix(.excerpt))]
    Http { status: u16, excerpt: String },
    #[error("unexpected response from the server: {0}")]
    Decode(String),
}

/// The `": <body>"` tail of an HTTP error line, or nothing at all when the
/// server sent no body worth quoting. Without this a 404 whose excerpt was
/// suppressed reads `server error 404: `, and the UI paints the dangling
/// colon (the video path builds `Http` with an empty excerpt on purpose).
fn http_excerpt_suffix(excerpt: &str) -> String {
    if excerpt.trim().is_empty() {
        String::new()
    } else {
        format!(": {excerpt}")
    }
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

#[cfg(test)]
mod tests {
    use super::RommError;

    #[test]
    fn http_error_with_a_body_quotes_it_after_a_colon() {
        let err = RommError::Http {
            status: 500,
            excerpt: "upstream unavailable".into(),
        };
        assert_eq!(err.to_string(), "server error 500: upstream unavailable");
    }

    #[test]
    fn http_error_without_a_body_ends_at_the_status() {
        // `get_bytes_with_type` (the video path) always builds this shape, so a
        // missing `path_video` must read `server error 404`, with no trailing
        // colon and no trailing space for the UI to paint.
        let err = RommError::Http {
            status: 404,
            excerpt: String::new(),
        };
        assert_eq!(err.to_string(), "server error 404");
    }

    #[test]
    fn http_error_with_a_whitespace_only_body_ends_at_the_status() {
        let err = RommError::Http {
            status: 502,
            excerpt: "   ".into(),
        };
        assert_eq!(err.to_string(), "server error 502");
    }
}

//! Core log line stream (`F-MONITOR-001`, upstream `ProcessService.displayLog`).
//!
//! The pipeline is `AsyncRead -> LineReader -> LogFilter -> LogBuffer`, where
//! `LogBuffer` is a bounded ring for bulk lines plus a small, separate queue
//! for control events. The separation is the backpressure guarantee: when a
//! slow consumer lets the ring overflow, only log lines are evicted (and
//! counted); control events (process start/exit, stream errors) live in their
//! own queue and cannot be displaced by log volume.

use std::collections::VecDeque;

use tokio::io::{AsyncRead, AsyncReadExt};

/// Severity of a parsed log line, ordered least to most severe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
    /// No level token was found; filtering never hides these.
    Unknown,
}

impl LogLevel {
    /// Map a single token (`info`, `warning`, `[Info]`, `level=debug`).
    pub fn from_token(token: &str) -> Option<Self> {
        let token = token
            .trim()
            .trim_matches(|c: char| !c.is_ascii_alphanumeric())
            .to_ascii_lowercase();
        Some(match token.as_str() {
            "trace" | "verbose" => LogLevel::Trace,
            "debug" => LogLevel::Debug,
            "info" | "information" => LogLevel::Info,
            "warn" | "warning" => LogLevel::Warn,
            "error" | "err" => LogLevel::Error,
            "fatal" | "critical" | "panic" => LogLevel::Fatal,
            _ => return None,
        })
    }
}

/// One decoded log line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub text: String,
    /// True when the reader had to cut the line to honour `max_line_bytes`.
    pub truncated: bool,
}

impl LogLine {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            truncated: false,
        }
    }

    /// Best-effort severity detection for filtering.
    pub fn level(&self) -> LogLevel {
        detect_level(&self.text)
    }
}

/// Heuristic level detection: bracketed tag first (`[Info]`), then structured
/// `level=` / `"level":"..."`, then the first whitespace-separated token.
pub fn detect_level(text: &str) -> LogLevel {
    if let Some(start) = text.find('[') {
        if let Some(end) = text[start + 1..].find(']') {
            if let Some(level) = LogLevel::from_token(&text[start + 1..start + 1 + end]) {
                return level;
            }
        }
    }
    let lowered = text.to_ascii_lowercase();
    for needle in ["level=", "\"level\":", "level\":"] {
        if let Some(pos) = lowered.find(needle) {
            let rest = &text[pos + needle.len()..];
            let token: String = rest
                .trim_start_matches(['"', ' ', '\''])
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            if let Some(level) = LogLevel::from_token(&token) {
                return level;
            }
        }
    }
    if let Some(first) = text.split_whitespace().next() {
        if let Some(level) = LogLevel::from_token(first) {
            return level;
        }
    }
    LogLevel::Unknown
}

fn decode(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Async line reader with a hard per-line byte cap.
pub struct LineReader<R> {
    reader: R,
    buf: [u8; 8 * 1024],
    pending: Vec<u8>,
    max_line_bytes: usize,
    overlong: bool,
    truncated_lines: u64,
}

impl<R: AsyncRead + Unpin> LineReader<R> {
    /// `max_line_bytes` caps memory for a single line; anything longer is
    /// emitted as a truncated marker line and the remainder is discarded.
    pub fn new(reader: R, max_line_bytes: usize) -> Self {
        Self {
            reader,
            buf: [0u8; 8 * 1024],
            pending: Vec::new(),
            max_line_bytes: max_line_bytes.max(1),
            overlong: false,
            truncated_lines: 0,
        }
    }

    /// Number of lines that had to be truncated so far.
    pub fn truncated_lines(&self) -> u64 {
        self.truncated_lines
    }

    /// Read the next line, `None` at EOF.
    pub async fn next_line(&mut self) -> std::io::Result<Option<LogLine>> {
        loop {
            if let Some(pos) = self.pending.iter().position(|byte| *byte == b'\n') {
                let mut line: Vec<u8> = self.pending.drain(..=pos).collect();
                line.pop();
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                if self.overlong {
                    // Tail of an already-reported over-long line: drop it.
                    self.overlong = false;
                    continue;
                }
                if line.len() > self.max_line_bytes {
                    line.truncate(self.max_line_bytes);
                    self.truncated_lines += 1;
                    return Ok(Some(LogLine {
                        text: decode(&line),
                        truncated: true,
                    }));
                }
                return Ok(Some(LogLine {
                    text: decode(&line),
                    truncated: false,
                }));
            }

            if self.pending.len() > self.max_line_bytes {
                let chunk: Vec<u8> = self.pending.drain(..self.max_line_bytes).collect();
                self.overlong = true;
                self.truncated_lines += 1;
                return Ok(Some(LogLine {
                    text: decode(&chunk),
                    truncated: true,
                }));
            }

            let read = self.reader.read(&mut self.buf).await?;
            if read == 0 {
                if self.pending.is_empty() {
                    return Ok(None);
                }
                let mut line = std::mem::take(&mut self.pending);
                if self.overlong {
                    self.overlong = false;
                    continue;
                }
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                if line.len() > self.max_line_bytes {
                    line.truncate(self.max_line_bytes);
                    self.truncated_lines += 1;
                    return Ok(Some(LogLine {
                        text: decode(&line),
                        truncated: true,
                    }));
                }
                return Ok(Some(LogLine {
                    text: decode(&line),
                    truncated: false,
                }));
            }
            self.pending.extend_from_slice(&self.buf[..read]);
        }
    }
}

/// Bounded ring of log lines with visible overflow accounting.
pub struct RingBuffer {
    lines: VecDeque<LogLine>,
    max_lines: usize,
    max_bytes: usize,
    bytes: usize,
    dropped_lines: u64,
    dropped_bytes: u64,
}

impl RingBuffer {
    /// `max_lines == 0` or `max_bytes == 0` means "no limit" for that axis.
    pub fn new(max_lines: usize, max_bytes: usize) -> Self {
        Self {
            lines: VecDeque::new(),
            max_lines,
            max_bytes,
            bytes: 0,
            dropped_lines: 0,
            dropped_bytes: 0,
        }
    }

    pub fn push(&mut self, line: LogLine) {
        let len = line.text.len();
        if self.max_bytes != 0 && len > self.max_bytes {
            // A single line larger than the whole budget can never fit.
            self.dropped_lines += 1;
            self.dropped_bytes += len as u64;
            return;
        }
        self.bytes += len;
        self.lines.push_back(line);
        let max_lines = self.max_lines;
        let max_bytes = self.max_bytes;
        self.enforce(max_lines, max_bytes);
    }

    fn enforce(&mut self, max_lines: usize, max_bytes: usize) {
        while max_lines != 0 && self.lines.len() > max_lines {
            self.evict_one();
        }
        while max_bytes != 0 && self.bytes > max_bytes {
            self.evict_one();
        }
    }

    fn evict_one(&mut self) {
        if let Some(line) = self.lines.pop_front() {
            self.bytes -= line.text.len();
            self.dropped_lines += 1;
            self.dropped_bytes += line.text.len() as u64;
        }
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn dropped_lines(&self) -> u64 {
        self.dropped_lines
    }

    pub fn dropped_bytes(&self) -> u64 {
        self.dropped_bytes
    }

    pub fn iter(&self) -> impl Iterator<Item = &LogLine> {
        self.lines.iter()
    }

    pub fn snapshot(&self) -> Vec<LogLine> {
        self.lines.iter().cloned().collect()
    }
}

/// Level/keyword filter applied before a line enters the ring.
#[derive(Debug, Clone, Default)]
pub struct LogFilter {
    pub min_level: Option<LogLevel>,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

impl LogFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_min_level(mut self, level: LogLevel) -> Self {
        self.min_level = Some(level);
        self
    }

    pub fn include(mut self, keyword: impl Into<String>) -> Self {
        self.include.push(keyword.into());
        self
    }

    pub fn exclude(mut self, keyword: impl Into<String>) -> Self {
        self.exclude.push(keyword.into());
        self
    }

    /// `Unknown` levels are always kept so unfiltered output is not silently
    /// swallowed when a format is unrecognised.
    pub fn accept(&self, line: &LogLine) -> bool {
        if self
            .exclude
            .iter()
            .any(|keyword| line.text.contains(keyword.as_str()))
        {
            return false;
        }
        if !self.include.is_empty()
            && !self
                .include
                .iter()
                .any(|keyword| line.text.contains(keyword.as_str()))
        {
            return false;
        }
        if let Some(min) = self.min_level {
            let level = line.level();
            if level != LogLevel::Unknown && level < min {
                return false;
            }
        }
        true
    }
}

/// Process lifecycle markers that must survive log overflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlEvent {
    ProcessStarted,
    ProcessExit(i32),
    StreamError(String),
}

/// A log line or a control event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogEvent {
    Line(LogLine),
    Control(ControlEvent),
}

/// Ring of lines plus a protected control queue.
///
/// Backpressure note: `push_line` may evict old lines (counted by
/// [`RingBuffer::dropped_lines`]); `push_control` uses a separate queue that is
/// only bounded by a generous cap, so a log flood cannot drop a
/// `ProcessExit`. If the control cap itself is exceeded the event is counted
/// in `control_dropped` rather than silently lost.
pub struct LogBuffer {
    pub lines: RingBuffer,
    control: VecDeque<ControlEvent>,
    control_cap: usize,
    control_dropped: u64,
}

impl LogBuffer {
    pub fn new(max_lines: usize, max_bytes: usize) -> Self {
        Self {
            lines: RingBuffer::new(max_lines, max_bytes),
            control: VecDeque::new(),
            control_cap: 256,
            control_dropped: 0,
        }
    }

    pub fn with_control_cap(mut self, cap: usize) -> Self {
        self.control_cap = cap;
        self
    }

    pub fn push_line(&mut self, line: LogLine) {
        self.lines.push(line);
    }

    pub fn push_control(&mut self, event: ControlEvent) {
        if self.control.len() >= self.control_cap {
            self.control.pop_front();
            self.control_dropped += 1;
        }
        self.control.push_back(event);
    }

    pub fn push_event(&mut self, event: LogEvent) {
        match event {
            LogEvent::Line(line) => self.push_line(line),
            LogEvent::Control(event) => self.push_control(event),
        }
    }

    pub fn control(&self) -> Vec<ControlEvent> {
        self.control.iter().cloned().collect()
    }

    pub fn control_len(&self) -> usize {
        self.control.len()
    }

    pub fn control_dropped(&self) -> u64 {
        self.control_dropped
    }
}

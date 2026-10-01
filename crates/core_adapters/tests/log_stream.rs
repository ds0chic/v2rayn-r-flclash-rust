mod common;

use core_adapters::log_stream::{
    detect_level, ControlEvent, LineReader, LogBuffer, LogFilter, LogLevel, LogLine, RingBuffer,
};

async fn read_all(data: &[u8], max: usize) -> Vec<LogLine> {
    let mut reader = LineReader::new(data, max);
    let mut lines = Vec::new();
    while let Some(line) = reader.next_line().await.unwrap() {
        lines.push(line);
    }
    lines
}

#[tokio::test]
async fn splits_on_lf_and_crlf() {
    let lines = read_all(b"one\ntwo\r\nthree", 1024).await;
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].text, "one");
    assert_eq!(lines[1].text, "two");
    assert_eq!(lines[2].text, "three");
}

#[tokio::test]
async fn empty_stream_yields_none() {
    let lines = read_all(b"", 1024).await;
    assert!(lines.is_empty());
}

#[tokio::test]
async fn trailing_crlf_does_not_emit_blank_line() {
    let lines = read_all(b"a\r\n", 1024).await;
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "a");
}

#[tokio::test]
async fn truncates_over_long_line_and_recovers() {
    let mut reader = LineReader::new(&b"abcdefgh\nXY\n"[..], 4);
    let first = reader.next_line().await.unwrap().unwrap();
    assert_eq!(first.text, "abcd");
    assert!(first.truncated);
    let second = reader.next_line().await.unwrap().unwrap();
    assert_eq!(second.text, "XY");
    assert!(!second.truncated);
    assert_eq!(reader.truncated_lines(), 1);
    assert!(reader.next_line().await.unwrap().is_none());
}

#[tokio::test]
async fn exact_max_length_is_not_truncated() {
    let lines = read_all(b"abc\n", 3).await;
    assert_eq!(lines[0].text, "abc");
    assert!(!lines[0].truncated);
}

#[test]
fn detects_bracketed_level() {
    assert_eq!(detect_level("[Info] listening"), LogLevel::Info);
    assert_eq!(detect_level("2026/01/01 [Warning] disk"), LogLevel::Warn);
    assert_eq!(detect_level("[ERROR] boom"), LogLevel::Error);
}

#[test]
fn detects_structured_level() {
    assert_eq!(detect_level("level=debug something"), LogLevel::Debug);
    assert_eq!(
        detect_level(r#"{"level":"fatal","msg":"x"}"#),
        LogLevel::Fatal
    );
}

#[test]
fn unknown_level_when_no_token() {
    assert_eq!(detect_level("just a plain sentence"), LogLevel::Unknown);
}

#[test]
fn ring_enforces_line_limit_and_counts_drops() {
    let mut ring = RingBuffer::new(2, 0);
    for index in 0..5 {
        ring.push(LogLine::new(format!("line-{index}")));
    }
    assert_eq!(ring.len(), 2);
    assert_eq!(ring.iter().next().unwrap().text, "line-3");
    assert_eq!(ring.dropped_lines(), 3);
    assert!(ring.dropped_bytes() > 0);
}

#[test]
fn ring_enforces_byte_limit() {
    let mut ring = RingBuffer::new(0, 10);
    ring.push(LogLine::new("12345"));
    ring.push(LogLine::new("67890"));
    ring.push(LogLine::new("abcde"));
    assert!(ring.bytes() <= 10);
    assert_eq!(ring.len(), 2);
    assert_eq!(ring.dropped_lines(), 1);
}

#[test]
fn ring_drops_single_line_larger_than_budget() {
    let mut ring = RingBuffer::new(0, 4);
    ring.push(LogLine::new("too long to fit"));
    assert!(ring.is_empty());
    assert_eq!(ring.dropped_lines(), 1);
}

#[test]
fn filter_min_level_drops_lower_severity() {
    let filter = LogFilter::new().with_min_level(LogLevel::Warn);
    assert!(!filter.accept(&LogLine::new("[Info] quiet")));
    assert!(filter.accept(&LogLine::new("[Warn] louder")));
    assert!(filter.accept(&LogLine::new("[Error] loudest")));
}

#[test]
fn filter_keeps_unknown_levels() {
    let filter = LogFilter::new().with_min_level(LogLevel::Error);
    assert!(filter.accept(&LogLine::new("unparseable output")));
}

#[test]
fn filter_include_keywords() {
    let filter = LogFilter::new().include("proxy");
    assert!(filter.accept(&LogLine::new("dial proxy tcp")));
    assert!(!filter.accept(&LogLine::new("dial direct tcp")));
}

#[test]
fn filter_exclude_keywords_take_priority() {
    let filter = LogFilter::new().include("proxy").exclude("noise");
    assert!(!filter.accept(&LogLine::new("proxy noise")));
    assert!(filter.accept(&LogLine::new("proxy clean")));
}

#[test]
fn control_events_survive_log_flood() {
    let mut buffer = LogBuffer::new(1, 0);
    buffer.push_control(ControlEvent::ProcessStarted);
    for index in 0..50 {
        buffer.push_line(LogLine::new(format!("flood-{index}")));
    }
    buffer.push_control(ControlEvent::ProcessExit(0));
    assert_eq!(buffer.lines.dropped_lines(), 49);
    let control = buffer.control();
    assert_eq!(
        control,
        vec![ControlEvent::ProcessStarted, ControlEvent::ProcessExit(0)]
    );
    assert_eq!(buffer.control_dropped(), 0);
}

#[test]
fn control_queue_overflow_is_counted() {
    let mut buffer = LogBuffer::new(0, 0).with_control_cap(2);
    buffer.push_control(ControlEvent::ProcessStarted);
    buffer.push_control(ControlEvent::StreamError("a".into()));
    buffer.push_control(ControlEvent::StreamError("b".into()));
    assert_eq!(buffer.control_len(), 2);
    assert_eq!(buffer.control_dropped(), 1);
}

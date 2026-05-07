use crate::ast::FrameType;
use crate::event::{EventSink, EventSinkError, InMemoryEventSink, JsonlFileSink, PipelineEvent};

fn sample_events() -> Vec<PipelineEvent> {
    vec![
        PipelineEvent::PipelineStarted {
            pipeline_id: "pipe-1".to_string(),
            input: serde_json::json!({"key": "value"}),
            timestamp_ms: 1,
        },
        PipelineEvent::StepStarted {
            step_id: "s1".to_string(),
            command_id: "wiki.search".to_string(),
            timestamp_ms: 2,
        },
        PipelineEvent::frame(
            "s1",
            0,
            &FrameType::ParagraphFrame,
            &serde_json::json!({"text": "hello"}),
        ),
        PipelineEvent::CheckpointSaved {
            step_id: "s1".to_string(),
            timestamp_ms: 4,
        },
        PipelineEvent::StepCompleted {
            step_id: "s1".to_string(),
            output_frame_count: 1,
            timestamp_ms: 5,
        },
        PipelineEvent::StepStarted {
            step_id: "s2".to_string(),
            command_id: "citation.resolve".to_string(),
            timestamp_ms: 6,
        },
        PipelineEvent::StepFailed {
            step_id: "s2".to_string(),
            error_kind: "QuotaExceeded".to_string(),
            retryable: true,
            timestamp_ms: 7,
        },
        PipelineEvent::PipelineCompleted {
            pipeline_id: "pipe-1".to_string(),
            final_state: "failed".to_string(),
            timestamp_ms: 8,
        },
    ]
}

#[test]
fn event_serialize_roundtrip() {
    let events = sample_events();
    for original in &events {
        let json = serde_json::to_string(original).expect("serialize");
        let deserialized: PipelineEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(*original, deserialized);
    }
}

#[test]
fn in_memory_sink_collects_events() {
    let sink = InMemoryEventSink::new();
    let events = sample_events();
    for event in &events {
        sink.emit(event.clone());
    }
    assert_eq!(sink.events(), events);
}

#[test]
fn in_memory_sink_filters_by_step() {
    let sink = InMemoryEventSink::new();
    let events = sample_events();
    for event in &events {
        sink.emit(event.clone());
    }

    let s1_events = sink.events_for_step("s1");
    assert_eq!(s1_events.len(), 4);
    assert!(s1_events.iter().all(|e| matches!(e,
        PipelineEvent::StepStarted { step_id, .. } |
        PipelineEvent::Frame { step_id, .. } |
        PipelineEvent::CheckpointSaved { step_id, .. } |
        PipelineEvent::StepCompleted { step_id, .. }
        if step_id == "s1"
    )));

    let s2_events = sink.events_for_step("s2");
    assert_eq!(s2_events.len(), 2);
    assert!(s2_events.iter().all(|e| matches!(e,
        PipelineEvent::StepStarted { step_id, .. } |
        PipelineEvent::StepFailed { step_id, .. }
        if step_id == "s2"
    )));

    let empty = sink.events_for_step("nonexistent");
    assert!(empty.is_empty());
}

#[test]
fn in_memory_sink_to_jsonl() {
    let sink = InMemoryEventSink::new();
    let events = sample_events();
    for event in &events {
        sink.emit(event.clone());
    }

    let jsonl = sink.to_jsonl();
    let lines: Vec<&str> = jsonl.trim_end().split('\n').collect();
    assert_eq!(lines.len(), events.len());

    for (line, original) in lines.iter().zip(&events) {
        let deserialized: PipelineEvent = serde_json::from_str(line).expect("valid JSON line");
        assert_eq!(&deserialized, original);
    }
}

#[test]
fn event_replay_order_preserved() {
    let sink = InMemoryEventSink::new();
    let events = sample_events();
    for event in &events {
        sink.emit(event.clone());
    }
    assert_eq!(sink.replay(), events);
}

#[test]
fn jsonl_file_sink_appends() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.jsonl");

    let sink = JsonlFileSink::open(&path).expect("open sink");
    let events = sample_events();
    for event in &events {
        sink.emit(event.clone());
    }
    sink.flush().expect("flush");

    let content = std::fs::read_to_string(&path).expect("read file");
    let lines: Vec<&str> = content.trim_end().split('\n').collect();
    assert_eq!(lines.len(), events.len());

    for (line, original) in lines.iter().zip(&events) {
        let deserialized: PipelineEvent = serde_json::from_str(line).expect("valid JSON line");
        assert_eq!(&deserialized, original);
    }

    // Append a second batch and verify total count.
    let sink2 = JsonlFileSink::open(&path).expect("re-open sink");
    sink2.emit(PipelineEvent::PipelineStarted {
        pipeline_id: "pipe-2".to_string(),
        input: serde_json::json!({}),
        timestamp_ms: 100,
    });
    sink2.flush().expect("flush");

    let content2 = std::fs::read_to_string(&path).expect("read file after append");
    let lines2: Vec<&str> = content2.trim_end().split('\n').collect();
    assert_eq!(lines2.len(), events.len() + 1);
}

#[test]
fn payload_summary_truncation() {
    let long_payload = serde_json::json!({"text": "a".repeat(300) });
    let event = PipelineEvent::frame("s1", 0, &FrameType::JsonValue, &long_payload);
    match &event {
        PipelineEvent::Frame {
            payload_summary, ..
        } => {
            assert_eq!(payload_summary.len(), 203); // 200 chars + "..."
            assert!(payload_summary.ends_with("..."));
        }
        _ => panic!("expected Frame event"),
    }
}

#[test]
fn event_sink_error_display() {
    let io_err = EventSinkError::IoError("disk full".to_string());
    assert_eq!(io_err.to_string(), "event sink I/O error: disk full");

    let ser_err = EventSinkError::SerializeError("bad json".to_string());
    assert_eq!(
        ser_err.to_string(),
        "event sink serialization error: bad json"
    );
}

#[test]
fn event_sink_error_implements_std_error() {
    let err: Box<dyn std::error::Error> = Box::new(EventSinkError::IoError("test".to_string()));
    assert!(err.source().is_none());
}

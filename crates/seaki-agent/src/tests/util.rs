use crate::safe_truncate;

#[test]
fn safe_truncate_ascii() {
    let s = "Hello, world!";
    assert_eq!(safe_truncate(s, 5), "Hello");
    assert_eq!(safe_truncate(s, 100), s);
}

#[test]
fn safe_truncate_chinese() {
    let s = "这是一个中文字符串测试";
    assert_eq!(safe_truncate(s, 5), "这是一个中");
    assert_eq!(safe_truncate(s, 100), s);
}

#[test]
fn safe_truncate_emoji() {
    let s = "Hello 👋 World 🌍";
    assert_eq!(safe_truncate(s, 7), "Hello 👋");
    assert_eq!(safe_truncate(s, 100), s);
}

#[test]
fn safe_truncate_mixed_boundary() {
    let s = "中a文b👋c🌍d";
    assert_eq!(safe_truncate(s, 0), "");
    assert_eq!(safe_truncate(s, 1), "中");
    assert_eq!(safe_truncate(s, 3), "中a文");
    assert_eq!(safe_truncate(s, 5), "中a文b👋");
    assert_eq!(safe_truncate(s, 100), s);
}

#[test]
fn safe_truncate_empty() {
    assert_eq!(safe_truncate("", 10), "");
}

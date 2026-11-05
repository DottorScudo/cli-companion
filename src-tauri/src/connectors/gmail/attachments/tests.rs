use super::*;
use crate::connectors::gmail::mime::AttachmentSource;

fn attachment(name: &str, size: u64) -> AttachmentRef {
    AttachmentRef { filename: name.to_string(), size, source: AttachmentSource::Remote("att".to_string()) }
}

#[test]
fn small_attachments_are_all_wanted() {
    let list = [attachment("a.pdf", 10), attachment("b.png", 20)];
    let selection = select(&list);
    assert_eq!(selection.wanted, vec![0, 1]);
    assert!(selection.not_imported.is_empty());
    assert_eq!(wanted_bytes(&list, &selection), 30);
}

#[test]
fn an_attachment_over_the_limit_is_left_out() {
    let list = [attachment("big.zip", ATTACHMENT_SIZE_MAX + 1), attachment("ok.txt", ATTACHMENT_SIZE_MAX)];
    let selection = select(&list);
    assert_eq!(selection.wanted, vec![1]);
    assert_eq!(selection.not_imported, vec![("big.zip".to_string(), ATTACHMENT_SIZE_MAX + 1)]);
}

#[test]
fn only_the_first_ones_up_to_the_hub_limit_are_wanted() {
    let list: Vec<_> = (0..ATTACHMENTS_MAX + 2).map(|i| attachment(&format!("f{i}"), 1)).collect();
    let selection = select(&list);
    assert_eq!(selection.wanted.len(), ATTACHMENTS_MAX);
    assert_eq!(selection.not_imported.len(), 2);
    assert_eq!(selection.not_imported[0].0, format!("f{ATTACHMENTS_MAX}"));
}

#[test]
fn an_unnamed_attachment_gets_a_name() {
    assert_eq!(display_name(&attachment("  ", 1)), "allegato");
    assert_eq!(left_out(&attachment("", 7)), ("allegato".to_string(), 7));
}

#[test]
fn files_count_their_bytes() {
    let files = Files {
        files: vec![IncomingFile { name: "a".to_string(), bytes: vec![0; 3] }],
        not_imported: vec![("b".to_string(), 99)],
    };
    assert_eq!(files.bytes(), 3);
}

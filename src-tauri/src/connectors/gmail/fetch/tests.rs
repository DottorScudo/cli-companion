use super::*;
use crate::connectors::gmail::attachments::select;
use crate::connectors::gmail::fixtures::parsed;
use crate::connectors::testing::test_context;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::json;

fn data(bytes: &[u8]) -> Value {
    json!({"size": bytes.len(), "data": URL_SAFE_NO_PAD.encode(bytes)})
}

#[test]
fn the_attachment_data_is_decoded() {
    assert_eq!(decoded(&data(b"ciao")), Some(b"ciao".to_vec()));
    assert_eq!(decoded(&json!({"data": "Y2lhbw=="})), Some(b"ciao".to_vec()));
}

#[test]
fn bad_or_oversized_data_is_refused() {
    assert_eq!(decoded(&json!({})), None);
    assert_eq!(decoded(&json!({"data": 5})), None);
    assert_eq!(decoded(&json!({"data": "!!!"})), None);
    let limit = usize::try_from(ATTACHMENT_SIZE_MAX).unwrap();
    assert_eq!(decoded(&data(&vec![0u8; limit + 1])), None);
    assert_eq!(decoded(&data(&vec![0u8; limit])).map(|b| b.len()), Some(limit));
}

#[test]
fn only_service_errors_stop_the_download_of_an_attachment() {
    for skippable in
        [ApiFailure::NotFound, ApiFailure::BadRequest, ApiFailure::Error(ConnectorError::Transient("x".to_string()))]
    {
        assert_eq!(attachment_failure(skippable), Ok(None));
    }
    assert!(attachment_failure(ApiFailure::Error(ConnectorError::Auth("x".to_string()))).is_err());
    assert!(attachment_failure(ApiFailure::Error(ConnectorError::RateLimited { retry_after: None })).is_err());
    assert!(attachment_failure(ApiFailure::Unauthorized).is_err());
}

#[test]
fn inline_attachments_need_no_network_and_oversized_ones_are_left_out() {
    let t = test_context(&crate::connectors::gmail::descriptor::DESCRIPTOR, json!({}));
    let cache = Mutex::new(TokenCache::new());
    let inline = |name: &str, size: u64, bytes: Vec<u8>| AttachmentRef {
        filename: name.to_string(),
        size,
        source: AttachmentSource::Inline(bytes),
    };
    let mut message = parsed("m1", 1);
    message.attachments = vec![
        inline("a.txt", 3, vec![1, 2, 3]),
        inline("", 2, vec![9, 9]),
        inline("enorme.bin", ATTACHMENT_SIZE_MAX + 1, vec![]),
    ];
    let selection = select(&message.attachments);
    let files = tauri::async_runtime::block_on(download(&cache, &t.ctx, &message, selection)).unwrap();
    assert_eq!(files.files.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), vec!["a.txt", "allegato"]);
    assert_eq!(files.not_imported, vec![("enorme.bin".to_string(), ATTACHMENT_SIZE_MAX + 1)]);
}

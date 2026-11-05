use super::*;
use crate::connectors::model::{ConnectorInstance, ConnectorNotices, ConnectorsFile};
use crate::connectors::testing::{FAKE_CONTACT, FAKE_SOURCE, INSTANCE};
use crate::hub::{Channel, HubNotice, NoticeCategory};
use serde_json::json;

fn instance(notices_all: bool, enabled: bool) -> ConnectorInstance {
    ConnectorInstance {
        id: INSTANCE.to_string(),
        connector: "fake-contact".to_string(),
        label: "x".to_string(),
        enabled,
        config: json!({}),
        config_version: 1,
        notices: ConnectorNotices { all: notices_all, ..ConnectorNotices::default() },
        created_at: 1,
        updated_at: 1,
    }
}

fn router_with(inst: ConnectorInstance) -> NoticeRouter {
    let store = Arc::new(ConnectorsStore::default());
    store.replace(ConnectorsFile { instances: vec![inst], ..ConnectorsFile::default() });
    NoticeRouter::new(store)
}

fn notice(n: usize) -> ContactNotice {
    let notice = HubNotice {
        channel: Channel::General,
        thread_id: "thr-1".to_string(),
        message_id: format!("msg-{n}"),
        title: "t".to_string(),
        body: "b".to_string(),
        sound: crate::hub::HubNoticeSound::Ding,
    };
    ContactNotice {
        notice,
        category: NoticeCategory::Reply,
        channel_name: "Generale".to_string(),
        thread_title: "Titolo".to_string(),
        text: "testo".to_string(),
    }
}

#[test]
fn a_notice_reaches_a_registered_instance_with_the_switch_on() {
    let router = router_with(instance(true, true));
    let mut rx = router.register(INSTANCE, &FAKE_CONTACT).unwrap();
    router.deliver(&notice(1));
    assert_eq!(rx.try_recv().unwrap().notice.message_id, "msg-1");
}

#[test]
fn without_the_receive_notices_capability_nothing_is_registered() {
    let router = router_with(instance(true, true));
    assert!(router.register(INSTANCE, &FAKE_SOURCE).is_none());
    assert_eq!(router.registered(), 0);
    router.deliver(&notice(1));
}

#[test]
fn the_instance_switch_is_read_live_and_a_disabled_instance_gets_nothing() {
    for inst in [instance(false, true), instance(true, false)] {
        let router = router_with(inst);
        let mut rx = router.register(INSTANCE, &FAKE_CONTACT).unwrap();
        router.deliver(&notice(1));
        assert!(rx.try_recv().is_err());
    }
}

#[test]
fn a_full_queue_drops_the_notice_without_blocking() {
    let router = router_with(instance(true, true));
    let mut rx = router.register(INSTANCE, &FAKE_CONTACT).unwrap();
    for n in 0..NOTICE_QUEUE + 10 {
        router.deliver(&notice(n));
    }
    let mut received = 0;
    while rx.try_recv().is_ok() {
        received += 1;
    }
    assert_eq!(received, NOTICE_QUEUE);
}

#[test]
fn unregistering_or_clearing_stops_the_flow() {
    let router = router_with(instance(true, true));
    let mut rx = router.register(INSTANCE, &FAKE_CONTACT).unwrap();
    router.deliver(&notice(1));
    router.clear();
    router.deliver(&notice(2));
    assert_eq!(rx.try_recv().unwrap().notice.message_id, "msg-1");
    assert!(rx.try_recv().is_err());
    router.register(INSTANCE, &FAKE_CONTACT).unwrap();
    router.unregister(INSTANCE);
    assert_eq!(router.registered(), 0);
}

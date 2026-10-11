use super::*;
use crate::errors::ClientError;
use serde_json::{json, Value};
use std::{cell::RefCell, collections::HashMap, error::Error, net::IpAddr, rc::Rc};

type Events = Rc<RefCell<Vec<String>>>;

struct FakeIp {
    events: Events,
    v4: Option<IpAddr>,
    v6: Option<IpAddr>,
}
impl IPAddr for FakeIp {
    fn get_global_ip_addr(
        &self,
        interface: &str,
        family: AddressFamily,
    ) -> Result<IpAddr, Box<dyn Error>> {
        self.events.borrow_mut().push(format!("ip:{interface}"));
        let address = match family {
            AddressFamily::V4 => self.v4,
            AddressFamily::V6 => self.v6,
        };
        address.ok_or_else(|| Box::new(std::io::Error::other("no ip")) as Box<dyn Error>)
    }
}
struct FakeDns {
    events: Events,
    records: Value,
    fail_get: bool,
    fail_patch: Option<&'static str>,
}
impl DnsClient for FakeDns {
    fn get_dns_records(&self) -> Result<HashMap<String, Value>, ClientError> {
        self.events.borrow_mut().push("get".into());
        if self.fail_get {
            Err(ClientError::RecordNotFound)
        } else {
            Ok(serde_json::from_value(self.records.clone()).unwrap())
        }
    }
    fn update_record(
        &self,
        id: &Value,
        record: &Value,
        name: &str,
        record_type: &str,
    ) -> Result<HashMap<String, Value>, ClientError> {
        self.events.borrow_mut().push(format!(
            "patch:{record_type}:{}:{}:{name}",
            id.as_str().unwrap(),
            record.as_str().unwrap()
        ));
        if self.fail_patch == Some(record_type) {
            Err(ClientError::RecordNotFound)
        } else {
            Ok(HashMap::new())
        }
    }
}
struct FakeLogger(Events);
impl Logger for FakeLogger {
    fn log_info(&mut self, message: &str) {
        self.0.borrow_mut().push(format!("info:{message}"));
    }
    fn log_error(&mut self, message: &str) {
        self.0.borrow_mut().push(format!("error:{message}"));
    }
}
struct Scenario {
    args: Args,
    events: Events,
    v4: Option<IpAddr>,
    v6: Option<IpAddr>,
    records: Value,
    fail_get: bool,
    fail_patch: Option<&'static str>,
}
impl Scenario {
    fn new() -> Self {
        Self {
            args: Args {
                api_token: "token".into(),
                zone_id: "zone".into(),
                record_name: "name".into(),
                v4_int: "v4".into(),
                v6_int: "v6".into(),
                dry_run: false,
            },
            events: Rc::new(RefCell::new(vec![])),
            v4: Some("8.8.8.8".parse().unwrap()),
            v6: Some("2606:4700:4700::1111".parse().unwrap()),
            records: json!({"result":[{"id":"a","name":"name","type":"A"},{"id":"aaaa","name":"name","type":"AAAA"}]}),
            fail_get: false,
            fail_patch: None,
        }
    }
    fn run(&self) -> Result<(), AppError> {
        let ip = FakeIp {
            events: self.events.clone(),
            v4: self.v4,
            v6: self.v6,
        };
        run(
            &self.args,
            &ip,
            || {
                self.events.borrow_mut().push("dns_factory".into());
                FakeDns {
                    events: self.events.clone(),
                    records: self.records.clone(),
                    fail_get: self.fail_get,
                    fail_patch: self.fail_patch,
                }
            },
            || {
                self.events.borrow_mut().push("logger_factory".into());
                Box::new(FakeLogger(self.events.clone()))
            },
        )
    }
    fn patches(&self) -> Vec<String> {
        self.events
            .borrow()
            .iter()
            .filter(|event| event.starts_with("patch:"))
            .cloned()
            .collect()
    }
    fn errors(&self) -> Vec<String> {
        self.events
            .borrow()
            .iter()
            .filter(|event| event.starts_with("error:"))
            .cloned()
            .collect()
    }
}

#[test]
fn missing_interfaces_do_not_create_dependencies() {
    let mut s = Scenario::new();
    s.args.v4_int.clear();
    s.args.v6_int.clear();
    assert_eq!(
        s.run(),
        Err(AppError::Stderr(
            "At least one of v4_int or v6_int must be set".into()
        ))
    );
    assert!(s.events.borrow().is_empty());
}

#[test]
fn updates_both_records_in_order_after_ip_and_logger_creation() {
    let s = Scenario::new();
    assert_eq!(s.run(), Ok(()));
    assert_eq!(
        *s.events.borrow(),
        vec![
            "ip:v4",
            "ip:v6",
            "logger_factory",
            "info:going to update A record to 8.8.8.8 based on interface: v4",
            "info:going to update AAAA record to 2606:4700:4700::1111 based on interface: v6",
            "dns_factory",
            "get",
            "patch:A:a:8.8.8.8:name",
            "info:updated A record to 8.8.8.8",
            "patch:AAAA:aaaa:2606:4700:4700::1111:name",
            "info:updated AAAA record to 2606:4700:4700::1111",
        ]
    );
}

#[test]
fn single_interface_only_requests_and_updates_that_side() {
    for v4_only in [true, false] {
        let mut s = Scenario::new();
        if v4_only {
            s.args.v6_int.clear();
        } else {
            s.args.v4_int.clear();
        }
        assert_eq!(s.run(), Ok(()));
        assert_eq!(
            s.patches(),
            if v4_only {
                vec!["patch:A:a:8.8.8.8:name"]
            } else {
                vec!["patch:AAAA:aaaa:2606:4700:4700::1111:name"]
            }
        );
        assert!(!s
            .events
            .borrow()
            .contains(&format!("ip:{}", if v4_only { "v6" } else { "v4" })));
    }
}

#[test]
fn dry_run_logs_plans_without_constructing_dns_client() {
    let mut s = Scenario::new();
    s.args.dry_run = true;
    assert_eq!(s.run(), Ok(()));
    let events = s.events.borrow();
    assert_eq!(events.len(), 5);
    assert_eq!(&events[..3], &["ip:v4", "ip:v6", "logger_factory"]);
    assert!(events[3].starts_with("info:going to update A"));
    assert!(events[4].starts_with("info:going to update AAAA"));
}

#[test]
fn ip_failure_is_reported_after_updating_other_side() {
    for fail_v4 in [true, false] {
        let mut s = Scenario::new();
        if fail_v4 {
            s.v4 = None;
        } else {
            s.v6 = None;
        }
        assert_eq!(s.run(), Err(AppError::Logged));
        assert_eq!(
            s.patches(),
            if fail_v4 {
                vec!["patch:AAAA:aaaa:2606:4700:4700::1111:name"]
            } else {
                vec!["patch:A:a:8.8.8.8:name"]
            }
        );
        assert_eq!(
            s.errors(),
            if fail_v4 {
                vec!["error:Failed to discover A address on interface v4: no ip"]
            } else {
                vec!["error:Failed to discover AAAA address on interface v6: no ip"]
            }
        );
    }
}

#[test]
fn no_addresses_fail_without_constructing_dns_client() {
    for dry_run in [false, true] {
        let mut s = Scenario::new();
        s.v4 = None;
        s.v6 = None;
        s.args.dry_run = dry_run;
        assert_eq!(s.run(), Err(AppError::Logged));
        assert_eq!(
            *s.events.borrow(),
            vec![
                "ip:v4",
                "ip:v6",
                "logger_factory",
                "error:Failed to discover A address on interface v4: no ip",
                "error:Failed to discover AAAA address on interface v6: no ip",
            ]
        );
        assert!(s.patches().is_empty());
    }
}

#[test]
fn dry_run_with_one_discovery_failure_logs_other_plan_and_fails() {
    let mut s = Scenario::new();
    s.v4 = None;
    s.args.dry_run = true;
    assert_eq!(s.run(), Err(AppError::Logged));
    let events = s.events.borrow();
    assert!(events
        .iter()
        .any(|event| event.starts_with("info:going to update AAAA")));
    assert!(!events.iter().any(|event| event == "dns_factory"));
    assert_eq!(s.errors().len(), 1);
    assert!(s.patches().is_empty());
}

#[test]
fn get_failure_is_logged_once_and_stops_updates() {
    let mut s = Scenario::new();
    s.fail_get = true;
    assert_eq!(s.run(), Err(AppError::Logged));
    assert_eq!(
        s.errors(),
        vec!["error:Failed to find record from response"]
    );
    assert!(s.patches().is_empty());
}

#[test]
fn missing_or_duplicate_a_record_stops_before_any_patch() {
    for records in [
        json!({"result":[{"id":"aaaa","name":"name","type":"AAAA"}]}),
        json!({"result":[{"id":"a","name":"name","type":"A"},{"id":"duplicate","name":"name","type":"A"}]}),
    ] {
        let mut s = Scenario::new();
        s.records = records;
        assert_eq!(s.run(), Err(AppError::Logged));
        assert!(s.patches().is_empty());
        assert_eq!(s.errors().len(), 1);
        assert!(s.errors()[0].starts_with("error:Failed to find A record:"));
    }
}

#[test]
fn missing_id_is_logged_without_patch() {
    let mut s = Scenario::new();
    s.records = json!({"result":[{"name":"name","type":"A"}]});
    assert_eq!(s.run(), Err(AppError::Logged));
    assert_eq!(s.errors(), vec!["error:Failed to find content"]);
    assert!(s.patches().is_empty());
}

#[test]
fn patch_failure_goes_to_stderr_and_a_failure_prevents_aaaa_update() {
    let mut s = Scenario::new();
    s.fail_patch = Some("A");
    assert_eq!(
        s.run(),
        Err(AppError::Stderr(
            "Failed to find record from response".into()
        ))
    );
    assert_eq!(s.patches(), vec!["patch:A:a:8.8.8.8:name"]);
    assert!(s.errors().is_empty());
    assert!(!s
        .events
        .borrow()
        .iter()
        .any(|e| e.starts_with("info:updated")));
}

#[test]
fn aaaa_patch_failure_keeps_completed_a_update() {
    let mut s = Scenario::new();
    s.fail_patch = Some("AAAA");
    assert_eq!(
        s.run(),
        Err(AppError::Stderr(
            "Failed to find record from response".into()
        ))
    );
    assert_eq!(
        s.patches(),
        vec![
            "patch:A:a:8.8.8.8:name",
            "patch:AAAA:aaaa:2606:4700:4700::1111:name"
        ]
    );
    assert!(s
        .events
        .borrow()
        .contains(&"info:updated A record to 8.8.8.8".into()));
    assert!(!s
        .events
        .borrow()
        .iter()
        .any(|e| e.starts_with("info:updated AAAA")));
}

#[test]
fn missing_aaaa_keeps_a_update_and_original_error_wording() {
    let mut s = Scenario::new();
    s.records = json!({"result":[{"id":"a","name":"name","type":"A"}]});
    assert_eq!(s.run(), Err(AppError::Logged));
    assert_eq!(s.patches(), vec!["patch:A:a:8.8.8.8:name"]);
    assert_eq!(
        s.errors(),
        vec!["error:Failed to find A record: Failed to find record from response"]
    );
}

#[test]
fn same_interface_updates_both_record_families() {
    let mut s = Scenario::new();
    s.args.v4_int = "eth0".into();
    s.args.v6_int = "eth0".into();
    assert_eq!(s.run(), Ok(()));
    assert_eq!(
        s.patches(),
        vec![
            "patch:A:a:8.8.8.8:name",
            "patch:AAAA:aaaa:2606:4700:4700::1111:name",
        ]
    );
}

#[test]
fn mixed_case_configured_name_finds_and_updates_both_records() {
    let mut s = Scenario::new();
    s.args.record_name = "NAME".into();
    assert_eq!(s.run(), Ok(()));
    assert_eq!(
        s.patches(),
        vec![
            "patch:A:a:8.8.8.8:NAME",
            "patch:AAAA:aaaa:2606:4700:4700::1111:NAME",
        ]
    );
}

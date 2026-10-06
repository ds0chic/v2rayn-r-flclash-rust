#[cfg(test)]
#[path = "../../../../../../services/net_host/src/events.rs"]
mod actual_event_bus;

#[cfg(test)]
#[allow(dead_code)]
#[path = "../../../../../../services/net_host/src/helper_client.rs"]
mod helper_client;

#[cfg(test)]
#[allow(dead_code)]
#[path = "../../../../../../services/net_host/src/journal.rs"]
mod journal;

#[cfg(test)]
#[allow(dead_code)]
#[path = "../../../../../../services/net_host/src/tun_lease.rs"]
mod tun_lease;

#[cfg(test)]
mod tests {
    use application::{runtime_client::NullRuntimeClient, AppEngine};
    use domain::runtime_plan::{ConfigSource, ContentHash, RuntimeTarget};
    use domain::{AppliedRevision, CoreType, DesiredRevision, RuntimePlan};
    use ipc_contract::{
        CidrAddress, HelperOp, HelperRequest, HelperResponse, HelperResult, SessionIdentity,
        TunAddressConfig, HELPER_PROTOCOL_VERSION,
    };
    use privileged_helper::backend::FakeBackend;
    use privileged_helper::server::{
        serve_connection, ConnectionLease, HelperServer, HelperServerConfig,
    };
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    struct GatedRuntime {
        inner: NullRuntimeClient,
        entered: std::sync::mpsc::Sender<()>,
        release: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
    }
    impl application::runtime_client::RuntimeClient for GatedRuntime {
        fn snapshot(
            &self,
        ) -> Result<application::runtime_client::RuntimeSnapshot, domain::DomainError> {
            self.inner.snapshot()
        }
        fn apply(
            &self,
            plan: &RuntimePlan,
        ) -> Result<application::runtime_client::ApplyOutcome, domain::DomainError> {
            self.entered.send(()).unwrap();
            self.release
                .lock()
                .unwrap()
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap();
            self.inner.apply(plan)
        }
        fn stop(&self) -> Result<(), domain::DomainError> {
            self.inner.stop()
        }
        fn cancel(&self, id: &domain::JobId) -> Result<domain::CancelOutcome, domain::DomainError> {
            self.inner.cancel(id)
        }
    }

    #[test]
    fn applying_a_while_desired_moves_to_b_must_still_publish_a() {
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let runtime = Arc::new(GatedRuntime {
            inner: NullRuntimeClient::new(),
            entered: entered_tx,
            release: std::sync::Mutex::new(release_rx),
        });
        let engine = Arc::new(AppEngine::with_runtime(runtime.clone()));
        engine.seed(vec![
            domain::Profile {
                index_id: "synthetic-a".into(),
                config_type: domain::ConfigType::Socks,
                core_type: Some(CoreType::Xray),
                address: "127.0.0.1".into(),
                port: 12977,
                ..Default::default()
            },
            domain::Profile {
                index_id: "synthetic-b".into(),
                config_type: domain::ConfigType::Socks,
                core_type: Some(CoreType::Xray),
                address: "127.0.0.1".into(),
                port: 12978,
                ..Default::default()
            },
        ]);
        let mut settings = engine.load_settings().unwrap().settings;
        settings.inbound[0].local_port = 11977;
        engine.save_settings(settings, 0).unwrap();
        engine.set_active(Some("synthetic-a".into())).unwrap();
        let revision = engine.desired_revision();
        let plan = engine
            .build_runtime_plan_with_hints(
                "synthetic-a",
                revision,
                &application::tun_plan::TunPlanHints::default(),
            )
            .unwrap();
        let frozen_plan_id = plan.plan_id.clone();
        let applying = engine.clone();
        let task = std::thread::spawn(move || {
            applying
                .apply_runtime(plan, DesiredRevision::new(revision))
                .unwrap()
        });
        entered_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        engine.set_active(Some("synthetic-b".into())).unwrap();
        release_tx.send(()).unwrap();
        task.join().unwrap();
        runtime.inner.mark_running_with(
            "synthetic-session-a",
            vec![11977],
            AppliedRevision::new(revision),
        );
        engine.snapshot().unwrap();
        let actual_target = engine.applied_session().unwrap().active_index_id;
        println!("OBSERVED applied_plan={frozen_plan_id} generated_from=synthetic-a applied_target={actual_target:?} desired_target={:?}; AppEngine production + gated synthetic RuntimeClient; no process/IPC/OS", engine.active_profile());
        assert_eq!(
            actual_target.as_deref(),
            Some("synthetic-a"),
            "a newer desired selection is not the accepted plan's applied target"
        );
    }

    struct CleanupFailure;
    impl crate::helper_client::HelperLink for CleanupFailure {
        fn session_id(&self) -> String {
            "synthetic-cleanup".into()
        }
        fn dry_run(&self) -> bool {
            false
        }
        fn available(&mut self) -> Result<(), domain::DomainError> {
            Ok(())
        }
        fn apply(
            &mut self,
            spec: &runtime::tun::TunSpec,
        ) -> Result<crate::helper_client::TunLease, domain::DomainError> {
            Ok(crate::helper_client::TunLease::new(
                self.session_id(),
                spec.clone(),
                false,
            ))
        }
        fn cleanup(
            &mut self,
            _: &crate::helper_client::TunLease,
        ) -> Result<(), domain::DomainError> {
            Err(domain::DomainError::new(
                domain::codes::UNAVAILABLE,
                "error.synthetic_cleanup_failed",
            ))
        }
    }

    #[test]
    fn cleanup_failure_should_keep_recovery_record() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(format!("cleanup-fault-{}", std::process::id()));
        let spec = runtime::tun::TunSpec {
            kind: runtime::tun::TUN_CONFIG_KIND.into(),
            adapter_name: "synthetic-cleanup-tun".into(),
            interface_index: 7,
            addresses: vec![runtime::tun::TunAddress {
                address: "198.18.0.1".into(),
                prefix_len: 30,
            }],
            mtu: Some(1280),
            routes: vec![],
            route_exclude: vec![],
        };
        let lease = crate::helper_client::TunLease::new("synthetic-cleanup", spec, false);
        crate::tun_lease::write_tun_journal(&root, "synthetic-cleanup", &lease).unwrap();
        let result = crate::tun_lease::cleanup_tun_lease(
            &root,
            "synthetic-cleanup",
            &lease,
            &mut CleanupFailure,
        );
        let retained = crate::tun_lease::read_tun_journal(&root, "synthetic-cleanup").is_some();
        println!("OBSERVED cleanup_error_injected=true result_ok={} recovery_journal_retained={retained}; only synthetic file + actual production cleanup_tun_lease called; no OS/helper/pipe", result.is_ok());
        assert!(
            result.is_err(),
            "an unconfirmed cleanup cannot report success"
        );
        assert!(
            retained,
            "the owner needs a recovery record to retry failed cleanup"
        );
    }

    #[test]
    fn helper_failed_cleanup_should_retain_owned_resources_for_retry() {
        use privileged_helper::backend::FakeOp;
        let fake = Arc::new(FakeBackend::new().elevated(true).fail_on(
            FakeOp::ResetTunAddress,
            ipc_contract::HelperError::Backend {
                detail: "synthetic-reset-failure".into(),
            },
        ));
        let server = HelperServer::new(
            fake.clone(),
            HelperServerConfig {
                session_token: "synthetic-only".into(),
                ..Default::default()
            },
        );
        let mut lease = ConnectionLease::new("synthetic-retry");
        let request = HelperRequest {
            session: SessionIdentity {
                protocol_version: HELPER_PROTOCOL_VERSION,
                session_token: "synthetic-only".into(),
                peer_pid: 1,
                peer_created_at_ms: 1,
            },
            request_id: "synthetic-set".into(),
            operation: HelperOp::SetTunAdapterAddress {
                config: TunAddressConfig {
                    adapter_name: "synthetic-retry-tun".into(),
                    interface_index: 7,
                    addresses: vec![CidrAddress {
                        address: "198.18.0.1".into(),
                        prefix_len: 30,
                    }],
                    mtu: Some(1280),
                },
            },
        };
        assert!(matches!(
            server.handle(&mut lease, &request).result,
            HelperResult::TunAddressSet { .. }
        ));
        let failures = server.on_disconnect(&mut lease);
        let second = server.on_disconnect(&mut lease);
        println!("OBSERVED reset_error_count={} fake_interfaces={:?} owned_tun_count={} closed={} retry_error_count={}; FakeBackend only", failures.len(), fake.tun_interfaces(), lease.owned_tun_count(), lease.is_closed(), second.len());
        assert_eq!(failures.len(), 1);
        assert_eq!(fake.tun_interfaces(), [7]);
        assert_eq!(
            lease.owned_tun_count(),
            1,
            "failed resources must remain owned or be copied into durable recovery"
        );
    }

    #[tokio::test]
    async fn slow_control_subscriber_hits_the_unhandled_lagged_branch() {
        let bus = crate::actual_event_bus::EventBus::new();
        let mut receiver = bus.subscribe();
        for index in 0..2049 {
            bus.emit_named("synthetic-log-burst", serde_json::json!({"index": index}));
        }
        let result = receiver.recv().await;
        assert!(matches!(
            result,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_))
        ));
        // server.rs:147 uses while let Ok(event)=receiver.recv().await; a
        // Lagged result ends that forwarder without closing the connection.
        assert!(
            receiver.recv().await.is_ok(),
            "the stream could continue after explicit lag reconciliation"
        );
        println!("OBSERVED: actual EventBus emits Lagged after a synthetic 2049-event burst; production forwarder treats this recoverable error as end-of-stream; no pipe or OS used");
    }

    #[tokio::test(start_paused = true)]
    async fn active_helper_lease_is_still_cleaned_at_24h_without_a_heartbeat() {
        let fake = Arc::new(FakeBackend::new().elevated(true));
        let config = HelperServerConfig {
            session_token: "synthetic-only".into(),
            ..Default::default()
        };
        assert_eq!(config.idle_timeout.as_secs(), 24 * 60 * 60);
        let server = Arc::new(HelperServer::new(fake.clone(), config));
        let (mut client, stream) = tokio::io::duplex(65536);
        let task = tokio::spawn(async move {
            serve_connection(
                stream,
                server,
                ConnectionLease::new("synthetic-lease"),
                None,
            )
            .await
            .unwrap();
        });
        let request = HelperRequest {
            session: SessionIdentity {
                protocol_version: HELPER_PROTOCOL_VERSION,
                session_token: "synthetic-only".into(),
                peer_pid: 1,
                peer_created_at_ms: 1,
            },
            request_id: "synthetic-request".into(),
            operation: HelperOp::SetTunAdapterAddress {
                config: TunAddressConfig {
                    adapter_name: "synthetic-audit-tun".into(),
                    interface_index: 7,
                    addresses: vec![CidrAddress {
                        address: "198.18.0.1".into(),
                        prefix_len: 30,
                    }],
                    mtu: Some(1280),
                },
            },
        };
        let body = serde_json::to_vec(&request).unwrap();
        client
            .write_all(&(body.len() as u32).to_le_bytes())
            .await
            .unwrap();
        client.write_all(&body).await.unwrap();
        let mut prefix = [0u8; 4];
        client.read_exact(&mut prefix).await.unwrap();
        let mut payload = vec![0; u32::from_le_bytes(prefix) as usize];
        client.read_exact(&mut payload).await.unwrap();
        let result: HelperResponse = serde_json::from_slice(&payload).unwrap();
        assert!(matches!(result.result, HelperResult::TunAddressSet { .. }));
        tokio::time::advance(std::time::Duration::from_secs(6)).await;
        tokio::task::yield_now().await;
        assert_eq!(
            fake.tun_interfaces(),
            [7],
            "the old five-second defect is fixed"
        );
        tokio::time::advance(std::time::Duration::from_secs(24 * 60 * 60)).await;
        task.await.unwrap();
        assert!(
            fake.tun_interfaces().is_empty(),
            "current 24h idle expiry still cleans an active lease"
        );
        println!("OBSERVED: helper active lease survives virtual 6s but expires after virtual 24h; client remained open; no socket/OS backend used");
    }

    #[test]
    fn completed_apply_leaves_a_running_job_and_compound_operation_id() {
        let runtime = Arc::new(NullRuntimeClient::new());
        let engine = AppEngine::with_runtime(runtime.clone());
        let plan = RuntimePlan {
            plan_id: "synthetic-accepted".into(),
            desired_revision: 0,
            target: RuntimeTarget {
                core_type: CoreType::Xray,
                version: None,
                config: ConfigSource::Inline { body: "{}".into() },
                config_sha256: ContentHash::new(runtime::sha256_hex(b"{}")),
            },
            process_graph: Default::default(),
            outbound_graph: Default::default(),
            ports: vec![],
            privileges: vec![],
            network_policy: Default::default(),
            resources: vec![],
        };
        let operation = engine.apply_runtime(plan, DesiredRevision::new(0)).unwrap();
        runtime.mark_running_with("synthetic-session", vec![11977], AppliedRevision::new(0));
        let snapshot = engine.snapshot().unwrap();
        let jobs = snapshot
            .active_jobs
            .iter()
            .filter(|job| job.kind == "apply_runtime")
            .count();
        assert_eq!(jobs, 1);
        engine.stop_runtime().unwrap();
        let stopped = engine.snapshot().unwrap();
        assert_eq!(
            stopped
                .active_jobs
                .iter()
                .filter(|job| job.kind == "apply_runtime")
                .count(),
            1
        );
        assert!(operation.contains(":job-"));
        println!("OBSERVED: returned operation={operation}, apply jobs remain active after running + stop; this records current defect, not intended acceptance");
    }
}

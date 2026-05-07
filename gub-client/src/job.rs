use anyhow::{Context, Result, anyhow, bail};
use futures::StreamExt;
use log::{debug, warn};

use tokio::sync::Semaphore;
use zbus_systemd::{systemd1 as sd, zbus::{self, AsyncDrop}, zvariant::{OwnedValue, Value}};


/// pre-emptively factoring out platform-dependent structures needed to pass to [`run_job`]
pub struct SystemInterface {
    /// semaphore with one permit for subscibing to `JobRemoved()` signals
    job_removed_sem: Semaphore,
    conn: &'static zbus::Connection,
    mgr: sd::ManagerProxy<'static>,
}

impl SystemInterface {
    pub async fn new() -> Result<Self> {
        static CONN: tokio::sync::OnceCell<Option<zbus::Connection>> = tokio::sync::OnceCell::const_new();

        let conn = match CONN.get_or_try_init(|| async { zbus::Connection::session().await.map(Some) }).await {
            Ok(Some(x)) => x,
            Ok(None) => {
                bail!("dbus connection failed previously")
            },
            Err(e) => {
                CONN.get_or_init(|| async { None }).await;
                return Err(e).context("failed to create dbus connection")
            },
        };

        Ok(SystemInterface {
            job_removed_sem: Semaphore::new(1),
            conn,
            mgr: sd::ManagerProxy::new(conn).await.context("failed to create Manager proxy")?
        })
    }
}


/// I'll want to improve this behavior in the future to something more actionable - maybe we
/// want to reschedule on another machine if we timed out or a dependency failed?
fn sd_job_result(result: &str) -> Result<()> {
    match result {
        "done" => Ok(()),
        "canceled" => Err(anyhow!("canceled via systemd before starting")),
        "timeout" => Err(anyhow!("timed out via systemd before starting")),
        "failed" => Err(anyhow!("failed to start")),
        "dependency" => Err(anyhow!("failed to start because a dependency failed")),
        "skipped" => Err(anyhow!("skipped by systemd before starting")),
        _ => Err(anyhow!("unexpected result from systemd: {result:?}"))
    }
}

/// Incomplete - lumps similar states together
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveState {
    Active,
    Done(DoneState),
}

/// Incomplete - lumps similar states together
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DoneState {
    Inactive,
    Failed,
}

/// returns `Err` if it's in an unexpected state
fn sd_unit_active_state(state: &str) -> Result<ActiveState> {
    match state {
        "active" | "activating" | "deactivating" => Ok(ActiveState::Active),
        "inactive" => Ok(ActiveState::Done(DoneState::Inactive)),
        "failed" => Ok(ActiveState::Done(DoneState::Failed)),
        "maintenance" | "reloading" | "refreshing" => Err(anyhow!("unhandled unit active state from systemd: {state} (I wasn't expecting to see it)")),
        _ => Err(anyhow!("unexpected unit active state from systemd: {state:?}"))
    }
}


pub async fn run_job(sif: &SystemInterface, id: u64, script: String) -> Result<(bool, Option<u8>)> {
    debug!("Starting job {id}: {}", gub_wire::util::truncate_str_debug(&script, 40));

    let permit = sif.job_removed_sem.acquire().await?;
    let mut job_stream = sif.mgr.receive_job_removed().await.context("failed to subscribe to JobRemoved")?;

    let unit_name = format!("gubernator_job-{id}.service");

    // enclose all of this in an async block so we can be sure to call async drop on the job
    // stream.
    let res = async {
        let execstart: OwnedValue = Value::try_from([ 
            (
                "/usr/bin/env",
                ["/usr/bin/env", "bash", "-c", &script].as_slice(),
                false, // whether "unclean exit" is failure. It appears to be reversed?
            )
        ].as_slice())?.try_into_owned()?;
        let spawn_job = sif.mgr.start_transient_unit(
            unit_name.clone(),
            String::from("fail"),
            vec![ 
                (String::from("Type"), Value::from("exec").try_into_owned()?),
                (String::from("ExecStart"), execstart),
                // this appears to be effectively undocumented, but it seems to be what allows us
                // to make sure it doesn't dissappear
                (String::from("AddRef"), true.into()),
            ],
            vec![],
        ).await.context("unit failed to start")?;

        while let Some(sig) = job_stream.next().await {
            let args = match sig.args() {
                Ok(x) => x,
                Err(e) => {
                    warn!("bad JobRemoved() signal: {e}");
                    continue
                },
            };

            if args.job != spawn_job {
                continue
            }

            sd_job_result(&args.result)?;

            break
        }
        anyhow::Ok(())
    }.await;

    job_stream.async_drop().await;
    drop(permit);

    res?;


    debug!("Successfully started job {id} as {unit_name}");



    // we've successfully started the job, we'll just wait until it's done
    // I have to be careful because we ref'ed the unit, I expect it can live for as long as the
    // connection if I don't explicitly unref it.

    let unit_path = match sif.mgr.get_unit(unit_name).await {
        Ok(x) => x,
        Err(e) => {
            warn!("failed getting unit path of Ref'd unit - potentially created resource leak / zombie process: {e}");
            return Err(e).context("couldn't get systemd unit of job")?;
        },
    };
    let unit = match sd::UnitProxy::new(sif.conn, unit_path.clone()).await {
        Ok(x) => x,
        Err(e) => {
            warn!("failed creating UnitProx of Ref'd unit - potentially created resource leak / zombie process: {e}");
            return Err(e).context("failed to create UnitProxy")?;
        },
    };

    struct UnRefUnit(Option<sd::UnitProxy<'static>>);
    impl Drop for UnRefUnit {
        fn drop(&mut self) {
            let Some(px) = self.0.take() else {
                return
            };
            // we still get dropped normally even when we call async_drop, but we take the
            // contained proxy so this won't run normally
            debug!("abnormal return: trying to unref unit");
            tokio::spawn(async move {
                if let Err(e) = px.unref().await {
                    warn!("failed UnRef-ing unit - potentially created resource leak / zombie process: {e}");
                }
            });
        }
    }
    impl AsyncDrop for UnRefUnit {
        #[allow(mismatched_lifetime_syntaxes,clippy::type_complexity,clippy::type_repetition_in_bounds)]
        fn async_drop<'async_trait>(mut self) ->  ::core::pin::Pin<Box<dyn ::core::future::Future<Output = ()> + ::core::marker::Send+'async_trait> >where Self:'async_trait {
            let px = self.0.take();
            Box::pin( async move {
                let Some(px) = px else {
                    return
                };
                if let Err(e) = px.unref().await {
                    warn!("failed UnRef-ing unit - potentially created resource leak / zombie process: {e}");
                }
            })
        }
    }

    let unit_root = UnRefUnit(Some(unit.clone()));

    let service = sd::ServiceProxy::new(sif.conn, unit_path).await.context("failed to create ServiceProxy")?;




    // sd::ServiceProxy::new(sif.conn, unit)

    let mut state_stream = unit.receive_active_state_changed().await;

    let state = unit.active_state().await.context("failed to get active state")?;

    let done_state = 'exited: {
        let active_state = sd_unit_active_state(&state)?;
        drop(state);
        if let ActiveState::Done(ds) = active_state {
                break 'exited ds
        }

        while let Some(state) = state_stream.next().await {
            let state = state.get().await?;
            let active_state = sd_unit_active_state(&state)?;

            if let ActiveState::Done(ds) = active_state {
                break 'exited ds
            }
        }

        warn!("Did not get done message from active state change stream for job {id} - assuming failure");

        DoneState::Failed
    };

    let status = service.exec_main_status().await.context("failed to get exit code from status")?;

    let status = (status >= 0).then_some(status as u8);

    unit_root.async_drop().await;

    debug!("Job complete {id}: {done_state:?}");
    Ok((done_state != DoneState::Failed, status))
}



use std::{
    collections::BTreeMap, ffi::OsStr, net::SocketAddr, path::PathBuf, sync::{Arc, atomic::*}, time::{Duration, SystemTime}
};


use anyhow::{Context, Result};
use gub_wire::{ServerMsg, job_req::{Exec, JobDescription, JobDispatch, WorkingDir}, machine::{MachineDesc, MachineStatus}, protocol::ClientMsg, sel_expr::MachineSel};
use log::debug;
use tokio::{io::split, net::TcpListener, sync::oneshot, time::Instant};
use tokio_rustls::{
    TlsAcceptor,
    rustls::{
        self, RootCertStore, ServerConfig,
        crypto::{self, CryptoProvider},
        pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
        server::WebPkiClientVerifier
    },
};

mod logging;

type StdMutex<T> = std::sync::Mutex<T>;

fn options() -> getopts::Options {
    let mut o = getopts::Options::new();
    o.reqopt("a", "addr", "address to bind to", "ADDRESS");
    o.reqopt("c", "cert", "path to cert file", "CERTFILE");
    o.reqopt("k", "key", "path to key file", "KEYFILE");
    o.optflagmulti("v", "verbose", "increase verbosity");
    o.optflagmulti("q", "quiet", "decrease verbosity (by one -v flag)");
    o.optflag("h", "help", "print this message");
    o
}

struct Opts {
    addr: String,
    cert: PathBuf,
    key: PathBuf,
    verbose: log::LevelFilter,
}

fn get_opts<I: IntoIterator>(i: I) -> Result<Option<Opts>>
where
    I::Item: AsRef<OsStr>,
{
    let opts = options().parse(i)?;
    if opts.opt_present("help") {
        return Ok(None);
    }
    let cert = opts.opt_get("cert")?.unwrap();
    let key = opts.opt_get("key")?.unwrap();
    let addr = opts.opt_get("addr")?.unwrap();

    // LevelFilter doesn't expose it's `from_usize` but it does let us easily do so through its
    // iterator
    let verbose = (1 + opts.opt_count("verbose")).saturating_sub(opts.opt_count("quiet"));
    let verbose = log::LevelFilter::iter().find(|&l| l as usize == verbose).unwrap_or(log::LevelFilter::Trace);

    Ok(Some(Opts { addr, cert, key, verbose }))
}

#[cold]
fn print_help(short: bool) {
    let name_buf;
    let name = if let Some(n) = std::env::args_os()
        .next()
        .and_then(|a| a.into_string().ok())
    {
        name_buf = n;
        &*name_buf
    } else {
        env!("CARGO_PKG_NAME")
    };
    let opt = options();
    let s = if short {
        opt.short_usage(name)
    } else {
        opt.usage(&opt.short_usage(name))
    };
    eprintln!("{s}");
}

fn make_config(opts: &Opts) -> Result<Arc<ServerConfig>> {
    let provider = CryptoProvider::get_default().expect("no provider set");

    let certs = CertificateDer::pem_file_iter(&opts.cert)
        .with_context(|| {
            format!(
                "could not read cert file {cert}",
                cert = opts.cert.display()
            )
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut roots = RootCertStore::empty();
    roots.add_parsable_certificates(certs.iter().cloned());
    let roots = Arc::new(roots);

    let verifier = WebPkiClientVerifier::builder_with_provider(roots, Arc::clone(&provider))
        .allow_unauthenticated()
        .build()
        .context("failed to build client verifier")?;

    let key = PrivateKeyDer::from_pem_file(&opts.key)
        .with_context(|| format!("could not read key file {key}", key = opts.key.display()))?;

    let config = ServerConfig::builder_with_provider(Arc::clone(provider))
        .with_safe_default_protocol_versions()?
        .with_client_cert_verifier(verifier)
        .with_single_cert(certs, key)?;

    Ok(Arc::new(config))
}

#[derive(Debug)]
struct JobDone {
    nodename: String,
    success: bool,
    code: Option<u8>,
}

struct JobDispatchMsg {
    id: u64,
    dispatch: JobDispatch,
}

struct JobWaitMsg {
    id: u64,
    response: oneshot::Sender<JobDone>,
}

struct ClientState {
    addr: SocketAddr,
    desc: MachineDesc,
    jobs: tokio::sync::mpsc::Sender<JobDispatchMsg>,
    mstate: StdMutex<MutableClientState>,
}

impl ClientState {
    fn get_status(&self) -> MachineStatus {
        self.mstate.lock().unwrap().status.clone()
    }

    fn is_shutdown(&self) -> bool {
        self.mstate.lock().unwrap().shutdown.is_some()
    }
}

struct MutableClientState {
    shutdown: Option<std::time::SystemTime>,
    status: MachineStatus,
    last_heard_from: tokio::time::Instant,
}

#[derive(Default)]
struct State {
    id_cnt: AtomicU64,
    jobs: StdMutex<BTreeMap<u64, oneshot::Sender<JobDone>>>,
    machines: StdMutex<Arc<Vec<Arc<ClientState>>>>,
}

impl State {
    fn add_machine(&self, state: ClientState) -> Arc<ClientState> {
        let mut lock = self.machines.lock().unwrap();
        let a = Arc::new(state);
        Arc::make_mut(&mut lock).push(Arc::clone(&a));
        a
    }

    fn get_machines(&self) -> Arc<Vec<Arc<ClientState>>> {
        Arc::clone(&self.machines.lock().unwrap())
    }

    fn send_done(&self, id: u64, done: JobDone) {
        let ch = {
            self.jobs.lock().unwrap().remove(&id)
        };
        let Some(ch) = ch else {
            // TODO: should I warn here?
            return
        };
        let _ = ch.send(done);
    }

    async fn run_job(&self, req: &JobDescription) -> Result<Vec<JobDone>> {
        let machs = self.get_machines();

        let mut js = tokio::task::JoinSet::new();
        let mut filter = req.machine_sel.validate_machines_filter();
        for m in machs.iter().filter(|m| !m.is_shutdown()).filter(|m| filter(&m.desc)) {
            let (snd, recv) = oneshot::channel();
            let id = self.id_cnt.fetch_add(1, Ordering::Relaxed);
            {
                self.jobs.lock().unwrap().insert(id, snd);
            }
            m.jobs.send(JobDispatchMsg { id, dispatch: JobDispatch { working_dir: req.working_dir.clone(), exec: req.exec.clone() } }).await?;
            js.spawn(recv);
        }

        let mut ret = Vec::with_capacity(js.len());
        while let Some(done) = js.join_next().await {
            ret.push(done??);
        }
        Ok(ret)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let Some(
        ref opts @ Opts {
            ref addr,
            cert: _,
            key: _,
            verbose,
        },
    ) = get_opts(std::env::args_os().skip(1)).inspect_err(|_| print_help(true))?
    else {
        // if we get none, print help
        print_help(false);
        return Ok(());
    };

    logging::enable_logging(verbose);



    let _ = crypto::aws_lc_rs::default_provider().install_default();

    let config = make_config(opts)?;
    let acceptor = TlsAcceptor::from(config);
    let listener = TcpListener::bind(&addr)
        .await
        .with_context(|| format!("failed to bind to {}", addr))?;
    let state = Arc::new(State::default());

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let acceptor = acceptor.clone();
        let state = Arc::clone(&state);

        tokio::spawn(run_test_jobs(Arc::clone(&state)));

        let fut = async move {
            let mut buf = Vec::with_capacity(4080);
            let stream = acceptor.accept(stream).await.map_err(|e| 
                match e.downcast::<rustls::Error>() {
                    Ok(e) => anyhow::Error::from(e),
                    Err(e) => e.into(),
                }
            ).context("failed to establish tls connection")?;
            let (mut reader, mut writer) = split(stream);
            let desc = gub_wire::recieve_msg::<_, MachineDesc>(&mut reader, &mut buf).await?;
            let (j_snd, mut j_rcv) = tokio::sync::mpsc::channel(32);
            log::info!("new machine {peer_addr}: {desc:.2?}");
            let client = ClientState {
                addr: peer_addr,
                desc,
                jobs: j_snd,
                mstate: StdMutex::new(MutableClientState {
                    shutdown: None,
                    status: MachineStatus::default(),
                    last_heard_from: Instant::now(),
                }),
            };
            let client = state.add_machine(client);
            let rclient = Arc::clone(&client);

            let sstate = Arc::clone(&state);
            tokio::spawn(async move {
                let mut buf = Vec::new();
                while let Some(JobDispatchMsg { id, dispatch }) = j_rcv.recv().await {
                    let msg = ServerMsg::Job { 
                        id,
                        dispatch
                    };
                    if let Err(e) = gub_wire::send_msg(&mut writer, &mut buf, &msg).await {
                        eprintln!("{e}");

                        // try to record that we're not getting anything more here
                        // TODO: this isn't quite right...
                        sstate.send_done(id, JobDone { success: false, code: None, nodename: client.desc.uname.nodename.clone() });

                        // hard to recover here
                        break
                    }
                    client.mstate.lock().unwrap().last_heard_from = Instant::now();
                }
            });

            tokio::spawn(async move {
                let mut buf = Vec::new();
                loop {
                    let msg = gub_wire::recieve_msg(&mut reader, &mut buf).await;
                    let msg: ClientMsg = match msg {
                        Ok(msg) => msg,
                        Err(e) => {
                            eprintln!("{e}");

                            rclient.mstate.lock().unwrap().shutdown = Some(SystemTime::now());

                            // hard to recover here
                            return
                        },
                    };

                    let nodename = &*rclient.desc.uname.nodename;
                    let name = format_args!("{nodename} ({})", rclient.addr);
                    match msg {
                        ClientMsg::JobDone { id, success, code } => {
                            debug!("job {id} finished on {name}: success={success} code={code:?}");
                            state.send_done(id, JobDone { success, code, nodename: nodename.into() });
                            rclient.mstate.lock().unwrap().last_heard_from = Instant::now();
                        },
                        ClientMsg::Status(machine_status) => {
                            debug!("status update from {name}: {machine_status:.3?}");
                            let mut lock = rclient.mstate.lock().unwrap();
                            lock.status = machine_status;
                            lock.last_heard_from = Instant::now();
                        },
                        ClientMsg::Shutdown => {
                            let now = SystemTime::now();
                            debug!("status update from {name}: shutting down at {now:?}");
                            let mut lock = rclient.mstate.lock().unwrap();
                            lock.last_heard_from = Instant::now();
                            lock.shutdown = Some(now);
                        },
                    }
                }
            });

            Ok(()) as anyhow::Result<()>
        };

        tokio::spawn(async move {
            if let Err(e) = fut.await {
                eprintln!("{e:?}");
            }
        });
    }
}


async fn run_test_jobs(state: Arc<State>) {
    tokio::time::sleep(Duration::from_secs(1)).await;

    let job = JobDescription { 
        machine_sel: MachineSel::single_node(["marisa", "reimu"]),
        working_dir: WorkingDir::Home,
        exec: Exec::bash_script("sleep 3")
    };
    let res = state.run_job(&job).await;

    let _ = eprintln!("{res:?}");
}

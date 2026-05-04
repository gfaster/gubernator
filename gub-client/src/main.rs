use std::{ffi::OsStr, path::PathBuf, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use gub_wire::{ClientMsg, ServerMsg, machine::MachineDescState};
use log::debug;
use tokio::{io::{AsyncWriteExt, split}, net::TcpStream, sync::mpsc, time::Instant};
use tokio_rustls::{TlsConnector, rustls::{self, RootCertStore, pki_types::{CertificateDer, PrivateKeyDer, ServerName, pem::PemObject}}};

mod logging;

fn options() -> getopts::Options {
    let mut o = getopts::Options::new();
    o.reqopt("a", "host", "host to connect to", "ADDRESS");
    o.optopt("d", "domain", "doman to use for cert", "DOMAIN");
    o.reqopt("c", "cafile", "path to ca file", "CAFILE");
    o.reqopt("p", "clientauth", "path to client auth certs file", "CERTFILE");
    o.reqopt("k", "key", "path to client auth key file", "KEYFILE");
    o.optflagmulti("v", "verbose", "increase verbosity");
    o.optflagmulti("q", "quiet", "decrease verbosity (by one -v flag)");
    o.optflag("h", "help", "print this message");
    o
}

struct Opts {
    addr: String,
    domain: Option<String>,
    cert: PathBuf,
    key: PathBuf,
    ccert: PathBuf,
    verbose: log::LevelFilter,
}

fn get_opts<I: IntoIterator>(i: I) -> Result<Option<Opts>> 
where I::Item: AsRef<OsStr>
{
    let i = Vec::from_iter(i);
    if i.iter().map(AsRef::as_ref).any(|a| a == OsStr::new("-h") || a == OsStr::new("--help")) {
        return Ok(None)
    }
    let opts = options().parse(i)?;
    if opts.opt_present("help") {
        return Ok(None)
    }
    let addr = opts.opt_get("host")?.unwrap();
    let cert = opts.opt_get("cafile")?.unwrap();
    let ccert = opts.opt_get("clientauth")?.unwrap();
    let key = opts.opt_get("key")?.unwrap();
    let domain = opts.opt_str("domain");

    // LevelFilter doesn't expose it's `from_usize` but it does let us easily do so through its
    // iterator
    let verbose = (1 + opts.opt_count("verbose")).saturating_sub(opts.opt_count("quiet"));
    let verbose = log::LevelFilter::iter().find(|&l| l as usize == verbose).unwrap_or(log::LevelFilter::Trace);

    Ok(Some(Opts {
        addr,
        domain,
        cert,
        key,
        ccert,
        verbose
    }))
}

#[cold]
fn print_help(short: bool) {
    let name_buf;
    let name = if let Some(n) = std::env::args_os().next().and_then(|a| a.into_string().ok()).or_else(|| std::env::current_exe().ok().and_then(|e| e.into_os_string().into_string().ok())) {
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

#[tokio::main]
async fn main() -> Result<()> {
    let Some(Opts { addr, cert, key, ccert, domain, verbose }) = get_opts(std::env::args_os().skip(1))? else {
        // if we get none, print help
        print_help(false);
        return Ok(())
    };

    logging::enable_logging(verbose);

    let root_certs = tokio::task::spawn_blocking(move || {
        let mut root_certs = RootCertStore::empty();
        CertificateDer::pem_file_iter(&cert)
            .with_context(|| format!("could not read ca file {cert}", cert = cert.display()))?
            .try_for_each(|der| anyhow::Ok(root_certs.add(der?)?))
            .context("invalid ca cert")?;
        anyhow::Ok(root_certs)
    });

    let cert_chain = tokio::task::spawn_blocking(move ||
        CertificateDer::pem_file_iter(&ccert)
            .with_context(|| format!("could not read client auth cert file {cert}", cert = ccert.display()))?
            .collect::<Result<Vec<_>, _>>()
            .context("invalid cert")
    );

    let key = tokio::task::spawn_blocking(move ||
        PrivateKeyDer::from_pem_file(&key)
            .with_context(|| format!("could not read key file {key}", key = key.display()))
    );

    let machine = tokio::task::spawn_blocking(|| gub_wire::machine::MachineDescState::new());

    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_certs.await??)
        // .with_client_auth_cert(cert_chain.await??, key.await??).context("failed to configure client")?;
        .with_no_client_auth();

    let connector = TlsConnector::from(Arc::new(config));

    let domain = {
        let domain = domain.unwrap_or_else(|| addr.to_string());
        ServerName::try_from(domain.as_str()).with_context(|| format!("invalid domain `{domain}`"))?.to_owned()
    };

    let machine = machine.await?;
    const BASE_RETRY: Duration = Duration::from_millis(3000);
    let mut current_wait = BASE_RETRY;
    loop {
        let last_attempt = Instant::now();
        if let Err(e) = connect_to_gubernator(connector.clone(), &addr, &domain, machine.clone()).await {
            eprintln!("{e:?}")
        }
        let elapsed = last_attempt.elapsed();
        if elapsed > current_wait {
            current_wait = BASE_RETRY
        } else {
            current_wait *= 2;
        }
        eprintln!("retrying in {}s", current_wait.as_secs());
        tokio::time::sleep(current_wait).await
    }
}

async fn connect_to_gubernator(connector: TlsConnector, addr: &str, domain: &ServerName<'static>, machine: MachineDescState) -> Result<()> {
    let stream = TcpStream::connect(addr).await.with_context(|| format!("failed to connect to {addr}"))?;
    let mut stream = connector.connect(domain.clone(), stream).await.map_err(|e|
        // this error is wrapped in io::Error, which makes printout worse
        match e.downcast::<rustls::Error>() {
            Ok(e) => anyhow::Error::from(e),
            Err(e) => e.into(),
        }
    )?;

    let mut rbuf = Vec::new();
    {
        let desc = machine.get_desc().await;
        debug!("sending machine description: {desc:#?}");
        gub_wire::send_msg(&mut stream, &mut rbuf, &desc).await?;
    }
    stream.flush().await?;

    let (mut reader, mut writer) = split(stream);

    // keep client buffering small so we can discard status messages
    let (wsnd, mut wrcv) = mpsc::channel::<ClientMsg>(2);
    let (rsnd, rrcv) = mpsc::channel::<ServerMsg>(10);

    // heartbeat automatically shuts down on close
    tokio::spawn(heartbeat(machine.clone(), wsnd.clone()));

    // have separate read and write halves to make selecting async safe
    let readhalf = async move {
        loop {
            let msg = gub_wire::recieve_msg::<_, ServerMsg>(&mut reader, &mut rbuf).await.context("failed to receive message")?;
            if rsnd.send(msg).await.is_err() {
                return anyhow::Ok(())
            }
        }
    };

    let writehalf = async move {
        let mut buf = Vec::new();
        while let Some(msg) = wrcv.recv().await {
            gub_wire::send_msg(&mut writer, &mut buf, &msg).await.context("failed to send message")?;
        }
        anyhow::Ok(())
    };


    let h1 = tokio::spawn(async move {
        if let Err(e) = readhalf.await {
            eprintln!("{e:?}")
        }
    });
    let h2 = tokio::spawn(async move {
        if let Err(e) = writehalf.await {
            eprintln!("{e:?}")
        }
    });
    let h3 = tokio::spawn(async move {
        if let Err(e) = scheduler(rrcv, wsnd).await {
            eprintln!("{e:?}")
        }
    });


    let (r1, r2, r3) = tokio::join!(h1, h2, h3);

    r3.or(r2).or(r1).context("error joining")
}

async fn heartbeat(machine: MachineDescState, outgoing: mpsc::Sender<ClientMsg>) {
    let mut status = machine.get_status().await;
    loop {
        debug!("sending status: {status:?}");
        if outgoing.send(ClientMsg::Status(status)).await.is_err() {
            return
        }
        status = machine.wait_for_next_status().await;
    }
}

async fn scheduler(mut incoming: mpsc::Receiver<ServerMsg>, outgoing: mpsc::Sender<ClientMsg>) -> Result<()> {
    while let Some(msg) = incoming.recv().await {
        match msg {
            ServerMsg::Spawn { id, script } => {
                let snd = outgoing.clone();

                tokio::spawn(async move {
                    let (success, code) = match run_job(id, script).await {
                        Ok(x) => x,
                        Err(e) => {
                            eprintln!("failed to run job {id}: {e}");
                            (false, None)
                        },
                    };

                    snd.send(ClientMsg::JobDone { id, success, code }).await
                });
            },
        }
    }
    Ok(())
}

async fn run_job(id: u64, script: String) -> Result<(bool, Option<u8>)> {
    // TODO: actually run the script
    debug!("Starting job {id}: {}", gub_wire::util::truncate_str_debug(&script, 40));
    let status = tokio::process::Command::new("/usr/bin/env").arg("sleep").arg("40").status().await?;
    debug!("Job complete {id}");
    Ok((status.success(), status.code().map(|c| c as u8)))
}

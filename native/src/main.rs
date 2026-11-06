use cirnet_contracts_native::validation::Registry;
use cirnet_hardware_native::{
    pb::ErrorCode,
    rpc::{HardwareRpc, PreviewRpc, canonical, internal, legacy},
    simulator::Simulator,
    station::StationHub,
};
use std::{env, error::Error, io, net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

const DEFAULT_LISTEN: &str = "127.0.0.1:50051";

fn setting(name: &str) -> Option<String> {
    env::var(format!("CIRNET_{name}"))
        .ok()
        .or_else(|| env::var(format!("NETCIRCUIT_{name}")).ok())
}

fn required(name: &str) -> Result<String, Box<dyn Error>> {
    setting(name)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("CIRNET_{name} must be supplied").into())
}

fn loopback(address: &str) -> Result<SocketAddr, Box<dyn Error>> {
    let local = if let Some(port) = address.strip_prefix("localhost:") {
        format!("127.0.0.1:{port}").parse::<SocketAddr>()?
    } else {
        address.parse::<SocketAddr>()?
    };
    if !local.ip().is_loopback() {
        return Err("Hardware RPC must bind to a local-only address".into());
    }
    Ok(local)
}

#[cfg(unix)]
async fn signal() {
    use tokio::signal::unix::{SignalKind, signal};
    match signal(SignalKind::terminate()) {
        Ok(mut terminate) => {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = terminate.recv() => {},
            }
        }
        Err(_) => {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
}

async fn shutdown_trigger(supervised: bool) {
    if !supervised {
        signal().await;
        return;
    }
    let (closed, eof) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let _ = io::copy(&mut io::stdin().lock(), &mut io::sink());
        let _ = closed.send(());
    });
    tokio::select! {
        _ = signal() => {},
        _ = eof => {},
    }
}

#[cfg(not(unix))]
async fn signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(unix)]
struct OwnedSocket {
    path: PathBuf,
    device: u64,
    inode: u64,
}

#[cfg(unix)]
impl Drop for OwnedSocket {
    fn drop(&mut self) {
        use std::os::unix::fs::MetadataExt;
        if let Ok(current) = std::fs::symlink_metadata(&self.path)
            && current.dev() == self.device
            && current.ino() == self.inode
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

async fn verify_shutdown(
    completion: oneshot::Receiver<Result<(), ErrorCode>>,
    simulator: Arc<Simulator>,
) -> Result<(), Box<dyn Error>> {
    let stations = completion.await?;
    let workers = tokio::task::spawn_blocking(move || simulator.shutdown()).await?;
    stations.map_err(|error| format!("Hardware cleanup failed: {}", error.as_str_name()))?;
    workers?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut args: Vec<_> = env::args().skip(1).collect();
    let supervised = args.last().is_some_and(|arg| arg == "--exit-on-stdin-eof");
    if supervised {
        args.pop();
    }
    let listen = match args.as_slice() {
        [] => setting("HARDWARE_RPC_BIND").unwrap_or_else(|| DEFAULT_LISTEN.into()),
        [flag, address] if flag == "--listen" => address.clone(),
        _ => return Err("usage: cirnet-hardware [--listen ADDRESS] [--exit-on-stdin-eof]".into()),
    };
    #[cfg(unix)]
    let local = if let Some(path) = listen.strip_prefix("unix:") {
        if !std::path::Path::new(path).is_absolute() {
            return Err("Hardware Unix socket requires an absolute path".into());
        }
        None
    } else {
        Some(loopback(&listen)?)
    };
    #[cfg(not(unix))]
    let local = loopback(&listen)?;
    if setting("SIMULATION_BACKEND").is_some_and(|value| value != "cpp") {
        return Err("Native Hardware requires the cpp simulation backend".into());
    }
    let slots = setting("SIMULATION_WORKERS")
        .unwrap_or_else(|| "4".into())
        .parse::<usize>()?;
    if slots == 0 || slots > 64 {
        return Err("CIRNET_SIMULATION_WORKERS must be between 1 and 64".into());
    }

    let root = PathBuf::from(required("CONTRACTS_ROOT")?);
    let worker = PathBuf::from(required("SIM_CPP_WORKER")?);
    let digest = required("SIM_CPP_WORKER_SHA256")?;
    let registry = Arc::new(Registry::load(&root)?);
    let simulator = Arc::new(
        Simulator::new(worker, &digest, slots, &root, registry.clone())
            .map_err(|error| format!("Simulator startup failed: {}", error.code()))?,
    );
    let hub = Arc::new(
        StationHub::new(simulator.clone(), registry, slots)
            .map_err(|error| format!("Hardware startup failed: {}", error.as_str_name()))?,
    );
    let rpc = HardwareRpc::new(hub.clone());
    let router = Server::builder()
        .max_concurrent_streams(128)
        .add_service(canonical::hardware_service_server::HardwareServiceServer::new(rpc.clone()))
        .add_service(legacy::hardware_service_server::HardwareServiceServer::new(
            rpc,
        ))
        .add_service(
            internal::simulator_preview_server::SimulatorPreviewServer::new(PreviewRpc::new(
                simulator.clone(),
            )),
        );
    let (report, completion) = oneshot::channel();
    let shutdown = async move {
        shutdown_trigger(supervised).await;
        let _ = report.send(hub.shutdown().await);
    };

    #[cfg(unix)]
    if let Some(path) = listen.strip_prefix("unix:") {
        use std::os::unix::fs::MetadataExt;
        let path = PathBuf::from(path);
        let listener = tokio::net::UnixListener::bind(&path)?;
        let meta = std::fs::symlink_metadata(&path)?;
        let _socket = OwnedSocket {
            path,
            device: meta.dev(),
            inode: meta.ino(),
        };
        println!("CIRNET_HARDWARE_READY={listen}");
        router
            .serve_with_incoming_shutdown(
                tokio_stream::wrappers::UnixListenerStream::new(listener),
                shutdown,
            )
            .await?;
        return verify_shutdown(completion, simulator).await;
    }

    #[cfg(unix)]
    let local = local.expect("local TCP listener");
    let listener = TcpListener::bind(local).await?;
    println!("CIRNET_HARDWARE_READY={}", listener.local_addr()?);
    router
        .serve_with_incoming_shutdown(TcpListenerStream::new(listener), shutdown)
        .await?;
    verify_shutdown(completion, simulator).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_all_non_loopback_addresses() {
        for address in ["0.0.0.0:50051", "[::]:0", "192.0.2.1:1", "example.com:1"] {
            assert!(loopback(address).is_err());
        }
        assert!(loopback("localhost:0").is_ok());
        assert!(loopback("[::1]:50051").is_ok());
        assert!(loopback("127.1.0.3:50051").is_ok());
    }
}

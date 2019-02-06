use capnp::capability::Promise;
use capnp_rpc::{pry, rpc_twoparty_capnp, twoparty, RpcSystem};
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

// Include capnp generated code
pub mod echo_capnp {
    include!(concat!(env!("OUT_DIR"), "/echo_capnp.rs"));
}

struct Echo;

impl echo_capnp::echo::Server for Echo {
    fn echo(
        &mut self,
        params: echo_capnp::echo::EchoParams,
        mut results: echo_capnp::echo::EchoResults,
    ) -> Promise<(), capnp::Error> {
        let message = pry!(pry!(params.get()).get_message());
        println!("{:?}", message);
        results.get().set_response(message);
        Promise::ok(())
    }
}

fn server() {
    let addr = ([127, 0, 0, 1], 1100).into();

    let listener = TcpListener::bind(&addr).expect("bind");

    let echo = echo_capnp::echo::ToClient::new(Echo)
        .into_client::<capnp_rpc::Server>();

    let server = listener
        .incoming()
        .map_err(|e| eprintln!("accept failed = {:?}", e))
        .for_each(|sock| {
            let (reader, writer) = sock.split();

            let network = twoparty::VatNetwork::new(
                reader,
                writer,
                rpc_twoparty_capnp::Side::Server,
                Default::default(),
            );

            let rpc_system =
                RpcSystem::new(Box::new(network), Some(echo.clone().client));

            tokio::runtime::current_thread::spawn(
                rpc_system.map_err(|e| eprintln!("rpc failed = {:?}", e)),
            );

            Ok(())
        });

    tokio::runtime::current_thread::block_on_all(server).expect("echo server");
}

fn client() {
    let mut runtime =
        tokio::runtime::current_thread::Runtime::new().expect("runtime");

    let addr = ([127, 0, 0, 1], 1100).into();
    let stream = runtime.block_on(TcpStream::connect(&addr)).expect("stream");

    // Create the RPC client system
    let (reader, writer) = stream.split();

    let rpc_network = Box::new(twoparty::VatNetwork::new(
        reader,
        writer,
        rpc_twoparty_capnp::Side::Client,
        Default::default(),
    ));

    let mut rpc_system = RpcSystem::new(rpc_network, None);

    let echo: echo_capnp::echo::Client =
        rpc_system.bootstrap(rpc_twoparty_capnp::Side::Server);

    runtime.spawn(rpc_system.map_err(|e| eprintln!("rpc failed = {:?}", e)));

    // Send echo request
    let mut request = echo.echo_request();
    request.get().set_message("hello");

    let response = runtime.block_on(request.send().promise).unwrap();
    let message = response.get().unwrap().get_response().unwrap();
    println!("{:?}", message);
}

fn main() {
    match std::env::args().nth(1) {
        Some(ref arg) if arg == "server" => server(),
        Some(ref arg) if arg == "client" => client(),
        _ => eprintln!("usage: echo [client|server]"),
    }
}

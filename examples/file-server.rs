#![feature(async_await)]
#![feature(await_macro)]
#![feature(futures_api)]

#[macro_use]
extern crate log;

use asyncio::executor::{block_on, spawn, TcpListener, TcpStream};
use asyncio::fs_future::{read_to_string};
use failure::Error;

fn main() -> Result<(), Error> {
    env_logger::init();
    block_on(new_server())?;
    Ok(())
}


const CRLF: &[char] = &['\r', '\n'];

async fn new_server() -> Result<(), Error> {
    let mut listener = TcpListener::bind(&"127.0.0.1:7878".parse()?)?;
    info!("Listening on 127.0.0.1:7878");
    while let Ok((mut stream, addr)) = await!(listener.accept()) {
        info!("connection from {}", addr);
        spawn(handle_stream(stream))?;
    }
    Ok(())
}

async fn handle_stream(mut stream: TcpStream) -> Result<(), Error> {
    await!(stream.write_str("Please enter filename: "))?;
    let file_name_vec = await!(stream.read())?;
    let file_name = String::from_utf8(file_name_vec)?.trim_matches(CRLF).to_owned();
    let file_contents = await!(read_to_string(file_name))?;
    await!(stream.write_str(&file_contents))?;
    stream.close();
    Ok(())
}

mod default;
mod image;
mod nmp_hdr;
mod test_serial_port;
mod transfer;

pub use crate::default::reset;
pub use crate::image::{erase, list, test, upload};
pub use crate::transfer::SerialSpecs;

// use reqwest::header::USER_AGENT;
// let client = reqwest::Client::new();
// let res = client
//     .get("https://www.rust-lang.org")
//     .header(USER_AGENT, "My Rust Program 1.0")
//     .send()
//     .await?;
// let mut body = String::new();
// res.read_to_string(&mut body)?;
// println!("Status: {}", res.status());
// println!("Headers:\n{:#?}", res.headers());
// println!("Body:\n{}", body);

use reqwest::{Client, Proxy, Result};

pub fn client() -> Result<Client> {
    let is_used = false;
    let url = "";
    let port = 0;

    Ok(if is_used {
        let proxy = Proxy::all(format!("socks5://{}:{}", url, port))?;
        Client::builder().proxy(proxy).build()?
    } else {
        Client::new()
    })
}

use reqwest::{Client, Proxy, Result};

pub fn client(proxy_info: Option<(&str, u16)>) -> Result<Client> {
    Ok(if proxy_info.is_some() {
        let proxy = Proxy::all(format!(
            "socks5://{}:{}",
            proxy_info.unwrap().0,
            proxy_info.unwrap().1
        ))?;
        Client::builder().proxy(proxy).build()?
    } else {
        Client::new()
    })
}

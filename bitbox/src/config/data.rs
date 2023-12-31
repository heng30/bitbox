#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    #[serde(skip)]
    pub config_path: String,

    #[serde(skip)]
    pub db_path: String,

    #[serde(skip)]
    pub cache_dir: String,

    pub ui: UI,

    pub account: Account,

    pub socks5: Socks5,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_path: String::default(),
            db_path: "".to_string(),
            cache_dir: "".to_string(),
            ui: UI::default(),
            account: Account::default(),
            socks5: Socks5::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UI {
    pub font_size: u32,
    pub font_family: String,
    pub win_width: u32,
    pub win_height: u32,
    pub language: String,
}

impl Default for UI {
    fn default() -> Self {
        Self {
            font_size: 18,
            font_family: "SourceHanSerifCN".to_string(),
            win_width: 600,
            win_height: 800,
            language: "cn".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Account {
    pub max_feerate: u32,
    pub max_fee_amount: u32,
    pub max_send_amount: f64,
    pub skip_utxo_amount: u32,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            max_feerate: 100_u32,
            max_fee_amount: 10_000_u32,
            max_send_amount: 1_f64,
            skip_utxo_amount: 1_000_u32,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Socks5 {
    pub enabled: bool,
    pub url: String,
    pub port: u16,
}

impl Default for Socks5 {
    fn default() -> Self {
        Self {
            enabled: false,
            url: "127.0.0.1".to_string(),
            port: 1080,
        }
    }
}

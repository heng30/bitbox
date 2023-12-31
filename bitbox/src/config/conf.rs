use super::data::{self, Config};
use anyhow::Result;
use log::debug;
use platform_dirs::AppDirs;
use std::cell::RefCell;
use std::fs;
use std::sync::Mutex;

lazy_static! {
    pub static ref CONFIG: Mutex<RefCell<Config>> = Mutex::new(RefCell::new(Config::default()));
}

pub fn init() {
    if let Err(e) = CONFIG.lock().unwrap().borrow_mut().init() {
        panic!("{:?}", e);
    }
}

pub fn ui() -> data::UI {
    CONFIG.lock().unwrap().borrow().ui.clone()
}

pub fn account() -> data::Account {
    CONFIG.lock().unwrap().borrow().account.clone()
}

pub fn socks5() -> data::Socks5 {
    CONFIG.lock().unwrap().borrow().socks5.clone()
}

#[allow(dead_code)]
pub fn conf_path() -> String {
    let conf = CONFIG.lock().unwrap();
    let conf = conf.borrow();

    conf.config_path.clone()
}

#[allow(dead_code)]
pub fn db_path() -> String {
    let conf = CONFIG.lock().unwrap();
    let conf = conf.borrow();

    conf.db_path.clone()
}

pub fn cache_dir() -> String {
    let conf = CONFIG.lock().unwrap();
    let conf = conf.borrow();

    conf.cache_dir.clone()
}

pub fn config() -> data::Config {
    CONFIG.lock().unwrap().borrow().clone()
}

pub fn save(conf: data::Config) -> Result<()> {
    let config = CONFIG.lock().unwrap();
    let mut config = config.borrow_mut();
    *config = conf;
    config.save()
}

impl Config {
    pub fn init(&mut self) -> Result<()> {
        let app_dirs = AppDirs::new(Some("bitbox"), true).unwrap();
        Self::init_app_dir(&app_dirs)?;
        self.init_config(&app_dirs)?;
        self.init_path()?;
        self.load()?;
        debug!("{:?}", self);
        Ok(())
    }

    fn init_app_dir(app_dirs: &AppDirs) -> Result<()> {
        fs::create_dir_all(&app_dirs.data_dir)?;
        fs::create_dir_all(&app_dirs.config_dir)?;
        Ok(())
    }

    fn init_path(&self) -> Result<()> {
        fs::create_dir_all(&self.cache_dir)?;
        Ok(())
    }

    fn init_config(&mut self, app_dirs: &AppDirs) -> Result<()> {
        self.config_path = app_dirs
            .config_dir
            .join("bitbox.conf")
            .to_str()
            .unwrap()
            .to_string();

        self.db_path = app_dirs
            .data_dir
            .join("bitbox.db")
            .to_str()
            .unwrap()
            .to_string();

        self.cache_dir = app_dirs
            .data_dir
            .join("cache")
            .to_str()
            .unwrap()
            .to_string();

        Ok(())
    }

    fn load(&mut self) -> Result<()> {
        match fs::read_to_string(&self.config_path) {
            Ok(text) => match serde_json::from_str::<Config>(&text) {
                Ok(c) => {
                    self.ui = c.ui;
                    self.account = c.account;
                    self.socks5 = c.socks5;
                    Ok(())
                }
                Err(e) => Err(e.into()),
            },

            Err(_) => match serde_json::to_string_pretty(self) {
                Ok(text) => Ok(fs::write(&self.config_path, text)?),
                Err(e) => Err(e.into()),
            },
        }
    }

    pub fn save(&self) -> Result<()> {
        match serde_json::to_string_pretty(self) {
            Ok(text) => Ok(fs::write(&self.config_path, text)?),
            Err(e) => Err(e.into()),
        }
    }
}

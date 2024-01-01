use crate::slint_generatedAppWindow::{AppWindow, Logic, Store};
use crate::wallet::transaction::blockstream;
use crate::{btc, db, util};
use serde_json::Value;
use slint::{ComponentHandle, Weak};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::task::spawn;
use tokio::time::{sleep, Duration};

static IS_FLUSH_NOW: AtomicBool = AtomicBool::new(false);

pub fn init(ui: &AppWindow) {
    update_timer(ui.as_weak());

    ui.global::<Logic>().on_pretty_btc_price(move |price| {
        let price: f64 = price.parse().unwrap_or(0_f64);

        if price >= 100_f64 {
            slint::format!("{:.2}", price)
        } else if price >= 0.001_f64 {
            slint::format!("{:.3}", price)
        } else if price >= 0.000_01_f64 {
            slint::format!("{:.5}", price)
        } else {
            slint::format!("{:.8}", price)
        }
    });

    ui.global::<Logic>().on_flush_account(move || {
        IS_FLUSH_NOW.store(true, Ordering::SeqCst);
    });
}

fn update_timer(ui: Weak<AppWindow>) {
    spawn(async move {
        const FLUSH_INTERVAL: u64 = 60_u64;
        let mut inc_index = 0_u64;

        loop {
            if inc_index % FLUSH_INTERVAL == 0 || IS_FLUSH_NOW.load(Ordering::SeqCst) {
                let (network, address) = match db::account::select_all().await {
                    Ok(items) => {
                        if items.is_empty() {
                            (None, None)
                        } else {
                            match serde_json::from_str::<Value>(&items[0].data) {
                                Err(e) => {
                                    log::warn!("Error: {e:?}");
                                    (None, None)
                                }
                                Ok(value) => {
                                    let network = value["network"].as_str().unwrap().to_string();
                                    let address = if network == "main" {
                                        value["main-address"].as_str().unwrap().to_string()
                                    } else {
                                        value["test-address"].as_str().unwrap().to_string()
                                    };

                                    (Some(network), Some(address))
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("Error: {}", e);
                        (None, None)
                    }
                };

                if let (Some(network), Some(address)) = (network, address) {
                    let balance = match blockstream::fetch_balance(&network, &address).await {
                        Err(e) => {
                            log::warn!("{:?}", e);
                            None
                        }
                        Ok(value) => Some(value as f64 / 1e8),
                    };

                    let price = match btc::price().await {
                        Err(e) => {
                            log::warn!("{:?}", e);
                            0_u64
                        }
                        Ok(value) => value as u64,
                    };

                    let (slow, normal, fast) = match btc::feerate().await {
                        Err(e) => {
                            log::warn!("{:?}", e);
                            (0_u64, 0_u64, 0_u64)
                        }
                        Ok(item) => item,
                    };

                    let ui = ui.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        let ui = ui.unwrap();
                        let mut info = ui.global::<Store>().get_btc_info();

                        if price > 0 {
                            info.price = slint::format!("{}", price);
                        }

                        if slow > 0 {
                            info.byte_fee_slow = slint::format!("{}", slow);
                            info.byte_fee_normal = slint::format!("{}", normal);
                            info.byte_fee_fast = slint::format!("{}", fast);
                        }
                        info.update_time = util::time::local_now("%H:%M:%S").into();
                        ui.global::<Store>().set_btc_info(info);

                        let mut account = ui.global::<Store>().get_account();

                        if balance.is_some() {
                            account.balance_btc = slint::format!("{}", balance.unwrap());
                        }

                        if price > 0 {
                            let btc_amount: f64 = account.balance_btc.parse().unwrap_or(0_f64);
                            account.balance_usd =
                                slint::format!("{:.2}", (btc_amount * price as f64));
                        }

                        if balance.is_some() || price > 0 {
                            ui.global::<Store>().set_account(account);
                        }
                    });
                }
            }

            inc_index += 1;
            IS_FLUSH_NOW.store(false, Ordering::SeqCst);
            sleep(Duration::from_secs(1)).await;
        }
    });
}

#[allow(dead_code)]
async fn unconfirmed_send_balance() -> u64 {
    match db::activity::select_all().await {
        Ok(items) => {
            let mut balance = 0;
            for item in items.iter() {
                match serde_json::from_str::<Value>(&item.data) {
                    Err(e) => log::warn!("{e:?}"),
                    Ok(value) => {
                        if value["status"].as_str().unwrap() == "unconfirmed" {
                            let fee: f64 =
                                value["fee"].as_str().unwrap().parse().unwrap_or(0_f64) * 1e8;
                            let amount: f64 =
                                value["amount"].as_str().unwrap().parse().unwrap_or(0_f64) * 1e8;
                            balance += (fee + amount) as u64;
                        }
                    }
                }
            }

            balance
        }
        _ => 0_u64,
    }
}

use crate::btc;
use crate::slint_generatedAppWindow::{AppWindow, Logic, Store};
use crate::util;
use crate::wallet::transaction::blockstream;
use slint::{ComponentHandle, Weak};
use tokio::task::spawn;
use tokio::time::{sleep, Duration};

pub fn init(ui: &AppWindow) {
    update_timer(ui.as_weak());

    ui.global::<Logic>().on_pretty_btc_price(move |price| {
        let price: f64 = price.parse().unwrap_or(0_f64);

        if price >= 100_f64 {
            slint::format!("{:.2}", price)
        } else if price >= 0_f64 {
            slint::format!("{:.3}", price)
        } else if price >= 0.00_001_f64 {
            slint::format!("{:.5}", price)
        } else {
            slint::format!("{:.8}", price)
        }
    });
}

fn update_timer(ui_handle: Weak<AppWindow>) {
    spawn(async move {
        loop {
            // TODO
            let address = "tb1qq8jfvz4fzc83jrysyrtj83sn4ps0zenwjcetrl";
            let network = "test";
            let balance = match blockstream::fetch_balance(network, address).await {
                Err(e) => {
                    log::warn!("{:?}", e);
                    None
                }
                Ok(value) => Some(value as f64 / 10e8),
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

            let ui = ui_handle.clone();
            let _ = slint::invoke_from_event_loop(move || {
                let mut info = ui.clone().unwrap().global::<Store>().get_btc_info();
                if price > 0 {
                    info.price = slint::format!("{}", price);
                }

                if slow > 0 {
                    info.byte_fee_slow = slint::format!("{}", slow);
                    info.byte_fee_normal = slint::format!("{}", normal);
                    info.byte_fee_fast = slint::format!("{}", fast);
                }
                info.update_time = util::time::local_now("%H:%M:%S").into();
                ui.clone().unwrap().global::<Store>().set_btc_info(info);

                let mut account = ui.clone().unwrap().global::<Store>().get_account();

                if balance.is_some() {
                    account.balance_btc = slint::format!("{}", balance.unwrap());
                }

                if price > 0 {
                    let btc_amount: f64 = account.balance_btc.parse().unwrap_or(0_f64);
                    account.balance_usd = slint::format!("{:.2}", (btc_amount * price as f64));
                }

                if balance.is_some() || price > 0 {
                    ui.clone().unwrap().global::<Store>().set_account(account);
                }
            });

            sleep(Duration::from_secs(60)).await;
        }
    });
}

use crate::btc;
use crate::slint_generatedAppWindow::{AppWindow, Store};
use crate::util;
use slint::ComponentHandle;
use tokio::task::spawn;
use tokio::time::{sleep, Duration};

pub fn init(ui: &AppWindow) {
    let ui_handle = ui.as_weak();

    spawn(async move {
        loop {
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
            });

            sleep(Duration::from_secs(60)).await;
        }
    });
}

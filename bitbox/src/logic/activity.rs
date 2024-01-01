use crate::message::{async_message_success, async_message_warn};
use crate::slint_generatedAppWindow::{ActivityItem, AppWindow, Logic, Store};
use crate::util::translator::tr;
use crate::wallet::transaction::blockstream;
use crate::{db, util};
use serde_json::{json, Value};
use slint::{ComponentHandle, Model, VecModel, Weak};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::task::spawn;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

static IS_FLUSH_NOW: AtomicBool = AtomicBool::new(false);

pub fn init(ui: &AppWindow) {
    check_confirm_timer(ui.as_weak());

    let ui_handle = ui.as_weak();
    ui.global::<Logic>().on_activity_delete_item(move |uuid| {
        let ui = ui_handle.unwrap();

        for (index, item) in ui.global::<Store>().get_activity_datas().iter().enumerate() {
            if item.uuid == uuid {
                ui.global::<Store>()
                    .get_activity_datas()
                    .as_any()
                    .downcast_ref::<VecModel<ActivityItem>>()
                    .expect("We know we set a VecModel earlier")
                    .remove(index);

                let (ui, uuid) = (ui.as_weak(), uuid.to_string());
                spawn(async move {
                    match db::activity::delete(&uuid).await {
                        Ok(_) => async_message_success(ui.clone(), tr("删除成功")),
                        Err(e) => async_message_warn(
                            ui.clone(),
                            format!("{}. {}: {}", tr("删除失败"), tr("原因"), e),
                        ),
                    }
                });
                return;
            }
        }
    });

    ui.global::<Logic>().on_flush_activity(move || {
        IS_FLUSH_NOW.store(true, Ordering::SeqCst);
    });
}

fn check_confirm_timer(ui: Weak<AppWindow>) {
    spawn(async move {
        const FLUSH_INTERVAL: u64 = 60_u64;
        let mut inc_index = 0_u64;

        loop {
            if inc_index % FLUSH_INTERVAL == 0 || IS_FLUSH_NOW.load(Ordering::SeqCst) {
                match db::activity::select_all().await {
                    Ok(items) => {
                        let mut update_items = vec![];

                        for item in items.into_iter() {
                            match serde_json::from_str::<Value>(&item.data) {
                                Err(e) => log::warn!("Error: {e:?}"),
                                Ok(mut value) => {
                                    if value["status"].as_str().unwrap() == "unconfirmed" {
                                        let txid = value["txid"].as_str().unwrap();
                                        if let Ok(true) =
                                            blockstream::is_tx_confirmed(&item.network, txid).await
                                        {
                                            value["status"] =
                                                Value::String("confirmed".to_string());
                                            let _ = db::activity::update(
                                                &item.uuid,
                                                &value.to_string(),
                                            )
                                            .await;
                                            update_items.push((item.uuid, item.network));
                                        }
                                    }
                                }
                            }
                        }

                        let ui = ui.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            let ui = ui.unwrap();
                            let current_network =
                                ui.global::<Store>().get_account().network.to_string();

                            for (uuid, network) in update_items.into_iter() {
                                if current_network != network {
                                    continue;
                                }

                                for (index, mut item) in
                                    ui.global::<Store>().get_activity_datas().iter().enumerate()
                                {
                                    if uuid.as_str() != item.uuid.as_str() {
                                        continue;
                                    }

                                    item.status = "confirmed".into();
                                    ui.global::<Store>()
                                        .get_activity_datas()
                                        .set_row_data(index, item);
                                }
                            }
                        });
                    }
                    Err(e) => log::warn!("Error: {}", e),
                };
            }

            inc_index += 1;
            IS_FLUSH_NOW.store(false, Ordering::SeqCst);
            sleep(Duration::from_secs(1)).await;
        }
    });
}

pub fn load_items(ui: Weak<AppWindow>, network: String) {
    spawn(async move {
        match db::activity::select_all_network(&network).await {
            Ok(items) => {
                let mut activity_items = vec![];
                for item in items.iter().rev() {
                    match serde_json::from_str::<Value>(&item.data) {
                        Err(e) => log::warn!("Error: {e:?}"),
                        Ok(value) => {
                            activity_items.push(ActivityItem {
                                uuid: value["uuid"].as_str().unwrap().into(),
                                time: value["time"].as_str().unwrap().into(),
                                txid: value["txid"].as_str().unwrap().into(),
                                operate: value["operate"].as_str().unwrap().into(),
                                amount: value["amount"].as_str().unwrap().into(),
                                fee: value["fee"].as_str().unwrap().into(),
                                status: value["status"].as_str().unwrap().into(),
                            });
                        }
                    }
                }

                let _ = slint::invoke_from_event_loop(move || {
                    ui.clone()
                        .unwrap()
                        .global::<Store>()
                        .get_activity_datas()
                        .as_any()
                        .downcast_ref::<VecModel<ActivityItem>>()
                        .expect("We know we set a VecModel earlier")
                        .set_vec(activity_items);
                });
            }
            Err(e) => async_message_warn(
                ui.clone(),
                format!("{}. {}: {}", tr("加载失败"), tr("原因"), e),
            ),
        }
    });
}

pub fn activity_add_item(
    ui: &AppWindow,
    network: &str,
    txid: &str,
    operate: &str,
    amount: &str,
    fee: &str,
    status: &str,
) {
    let uuid = Uuid::new_v4().to_string();
    let network = network.to_string();
    let time = util::time::local_now("%m-%d %H:%M:%S");

    let item = ActivityItem {
        uuid: uuid.clone().into(),
        time: time.into(),
        txid: txid.into(),
        operate: operate.into(),
        amount: amount.into(),
        fee: fee.into(),
        status: status.into(),
    };

    let json_item = json!({
        "uuid": uuid.clone(),
        "time": item.time.to_string(),
        "txid": item.txid.to_string(),
        "operate": item.operate.to_string(),
        "amount": item.amount.to_string(),
        "fee": item.fee.to_string(),
        "status": item.status.to_string()
    });
    let json = serde_json::to_string(&json_item).unwrap();

    ui.global::<Store>()
        .get_activity_datas()
        .as_any()
        .downcast_ref::<VecModel<ActivityItem>>()
        .expect("We know we set a VecModel earlier")
        .insert(0_usize, item);

    spawn(async move {
        if let Err(e) = db::activity::insert(&uuid, &network, &json).await {
            log::warn!("Error: {:?}", e);
        }
    });
}

#[allow(unused)]
fn test_add(ui: &AppWindow) {
    for i in 0..5 {
        let network = if i % 2 == 0 { "main" } else { "test" };

        let i = format!("{}", i);
        activity_add_item(ui, network, &i, &i, &i, &i, &i);
    }
}

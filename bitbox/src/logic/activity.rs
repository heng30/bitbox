use crate::db;
use crate::message::{async_message_success, async_message_warn};
use crate::slint_generatedAppWindow::{ActivityItem, AppWindow, Logic, Store};
use crate::util;
use crate::util::translator::tr;
use serde_json::{json, Value};
use slint::{ComponentHandle, Model, VecModel, Weak};
use tokio::task::spawn;
use uuid::Uuid;

pub fn init(ui: &AppWindow) {
    let account = ui.global::<Store>().get_account();
    load_items(ui.as_weak(), account.network.to_string());

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
        status: status.into(),
    };

    let json_item = json!({
        "uuid": uuid.clone(),
        "time": item.time.to_string(),
        "txid": item.txid.to_string(),
        "operate": item.operate.to_string(),
        "amount": item.amount.to_string(),
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
        match db::activity::insert(&uuid, &network, &json).await {
            Err(e) => log::warn!("Error: {:?}", e),
            _ => (),
        }
    });
}

#[allow(unused)]
fn test_add(ui: &AppWindow) {
    for i in 0..5 {
        let network = if i % 2 == 0 {
            "main"
        } else {
            "test"
        };

        let i = format!("{}", i);
        activity_add_item(&ui, network, &i, &i, &i, &i);
    }
}

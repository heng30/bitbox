use crate::message_warn;
use crate::slint_generatedAppWindow::{AppWindow, Logic, Position};
use crate::util::translator::tr;
use slint::{ComponentHandle, PhysicalPosition};

pub fn init(ui: &AppWindow) {
    let ui_handle = ui.as_weak();
    ui.global::<Logic>().on_hide_window(move || {
        let ui = ui_handle.unwrap();
        if let Err(e) = ui.window().hide() {
            message_warn!(ui, format!("{}. {}: {e:?}", tr("隐藏程序失败"), tr("原因")));
        }
    });

    let ui_handle = ui.as_weak();
    ui.global::<Logic>().on_show_window(move || {
        let ui = ui_handle.unwrap();
        if let Err(e) = ui.window().show() {
            message_warn!(ui, format!("{}. {}: {e:?}", tr("显示程序失败"), tr("原因")));
        }
    });

    let ui_handle = ui.as_weak();
    ui.global::<Logic>().on_window_position(move || {
        let ui = ui_handle.unwrap();
        let pos = ui.window().position();

        Position {
            x: pos.x as f32,
            y: pos.y as f32,
        }
    });

    let ui_handle = ui.as_weak();
    ui.global::<Logic>().on_update_position(move |x, y| {
        let ui = ui_handle.unwrap();
        ui.window().set_position(PhysicalPosition { x, y });
    });
}

use crate::message_warn;
use crate::slint_generatedAppWindow::{AppWindow, Logic, Util};
use crate::util::translator::tr;
use image::Rgb;
use qrcode::QrCode;
use slint::{ComponentHandle, Image, Rgb8Pixel, SharedPixelBuffer};
use webbrowser;

pub fn init(ui: &AppWindow) {
    ui.global::<Util>().on_string_fixed2(move |n| {
        let n = n.to_string().parse::<f32>().unwrap_or(0.0f32);
        slint::format!("{:2}", (n * 100.0).round() / 100.0)
    });

    ui.global::<Util>()
        .on_float_fixed2(move |n| slint::format!("{:2}", (n * 100.0).round() / 100.0));

    let ui_handle = ui.as_weak();
    ui.global::<Util>().on_open_url(move |url| {
        let ui = ui_handle.unwrap();
        if let Err(e) = webbrowser::open(url.as_str()) {
            message_warn!(
                ui,
                format!("{}{}: {:?}", tr("打开链接失败！"), tr("原因"), e)
            );
        }
    });

    let ui_handle = ui.as_weak();
    ui.global::<Util>().on_generate_qrcode(move |msg| {
        let ui = ui_handle.unwrap();
        match QrCode::new(msg) {
            Ok(code) => {
                let qrc = code.render::<Rgb<u8>>().build();

                let buffer = SharedPixelBuffer::<Rgb8Pixel>::clone_from_slice(
                    qrc.as_raw(),
                    qrc.width(),
                    qrc.height(),
                );
                Image::from_rgb8(buffer)
            }
            Err(e) => {
                log::warn!("gen qrcode image error: {:?}", e);
                ui.global::<Util>().get_no_image()
            }
        }
    });
}

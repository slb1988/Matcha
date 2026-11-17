//! Reusable visual chrome components.
pub mod controls;
pub mod dialog;
pub mod drop;
pub mod font_catalog;
pub mod fonts;
pub mod icons;
pub mod menubar;
pub mod modal;
pub mod native_dialog;
pub mod pathbar;
pub mod popup;
pub mod scrollbar;
pub mod sheet_tabs;
pub mod splitter;
pub mod statusbar;
pub mod tabstrip;
pub mod theme;
pub mod toolbar;
pub mod wheel;
pub mod widgets;
pub mod window;

/// 无渲染器输入注入测试必须显式消费纹理增量（egui 0.37）。
#[cfg(test)]
pub(crate) fn test_frame(
    ctx: &eframe::egui::Context,
    input: eframe::egui::RawInput,
    run: impl FnMut(&mut eframe::egui::Ui),
) -> eframe::egui::FullOutput {
    let mut output = ctx.run_ui(input, run);
    output.textures_delta.clear();
    output
}

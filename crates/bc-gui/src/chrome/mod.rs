//! Reusable visual chrome components.
pub mod fonts;
pub mod icons;
pub mod menubar;
pub mod pathbar;
pub mod scrollbar;
pub mod splitter;
pub mod statusbar;
pub mod theme;
pub mod toolbar;

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

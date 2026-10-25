mod api;
mod app;
mod backups;
mod chapter_list;
mod cli;
mod credentials;
mod domain;
mod engine;
mod manage;
mod model_catalog;
mod reader;
mod reader_wheel;
mod source_options;
mod storage;
mod tracking;
mod ui;
mod upscale;
mod views;

fn main() -> iced::Result {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        if let Err(e) = cli::run(&args) {
            eprintln!("{e:#}");
            std::process::exit(1);
        }
        return Ok(());
    }
    iced::application(app::App::boot, app::App::update, app::App::view)
        .title("Suwa")
        .settings(iced::Settings {
            id: Some("app.suwa.Suwa".into()),
            default_text_size: iced::Pixels(15.0),
            ..Default::default()
        })
        .font(include_bytes!("../assets/fonts/MapleMono-Regular.otf").as_slice())
        .font(include_bytes!("../assets/fonts/MapleMono-Bold.otf").as_slice())
        .default_font(ui::BODY)
        .theme(app::App::theme)
        .subscription(app::App::subscription)
        .window_size((1180.0, 820.0))
        .window(iced::window::Settings {
            platform_specific: iced::window::settings::PlatformSpecific {
                application_id: "app.suwa.Suwa".into(),
                ..Default::default()
            },
            min_size: Some(iced::Size::new(820.0, 600.0)),
            size: iced::Size::new(1180.0, 820.0),
            ..Default::default()
        })
        .run()
}
